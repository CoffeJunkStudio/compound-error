extern crate proc_macro;

use std::collections::HashMap;
use std::hash::Hash;

use syn::parse_macro_input;
use syn::DeriveInput;
use syn::Data;
use syn::Meta;
use syn::MetaList;
use syn::NestedMeta;
use syn::MetaNameValue;
use syn::Fields;
use syn::Type;
use syn::Path;
use syn::Ident;
use quote::quote;
use quote::quote_spanned;
use proc_macro::TokenStream;

fn error(spanned: &impl syn::spanned::Spanned, message: &str) -> TokenStream {
	let span = spanned.span();
	let output = quote_spanned! {
		span => compile_error!(#message);
	};

	output.into()
}

enum AttrArgsError<'attr, 'ident, I: ?Sized> {
	KeyMismatch {
		specified: &'attr syn::Path,
		required: &'ident I
	},
	InvalidArg(syn::Path),
	LitArg(syn::Lit),
	DupliateArg(syn::Path),
	NoMeta(&'attr syn::Attribute),
	NoNestedMeta(MetaNameValue)
}

impl<'attr, 'ident, I: ?Sized> AttrArgsError<'attr, 'ident, I> {
	fn explain(self) -> TokenStream where I: std::fmt::Display {
		match self {
			Self::KeyMismatch{ specified, required } => error(specified, &format!("Expected '{}'.", required)),
			Self::InvalidArg(key) => error(&key, "Invalid argument."),
			Self::LitArg(lit) => error(&lit, "Invalid first-level literal."),
			Self::DupliateArg(key) => error(&key, "Duplicate argument."),
			Self::NoMeta(attr) => error(attr, "Must be of 'meta' type."),
			Self::NoNestedMeta(mnv) => error(&mnv, "Invalid name-value pair. Expected path or list."),
		}
	}
}

fn attr_args<'attr, 'ident, I: ?Sized>(
	attrs: &'attr [syn::Attribute],
	required_key: &'ident I,
	known_arg_keys: &[&'ident I]
) -> Result<HashMap<&'ident I, Vec<NestedMeta>>, AttrArgsError<'attr, 'ident, I>> where
	syn::Ident: PartialEq<I>,
	I: Hash + Eq
{
	let mut args: HashMap<&'ident I, Vec<NestedMeta>> = HashMap::new();

	for attr in attrs {
		if let Some(specified_key) = attr.path.get_ident() {
			if specified_key != required_key {
				return Err(AttrArgsError::KeyMismatch { 
					specified: &attr.path,
					required: required_key
				})
			}
		} else {
			return Err(AttrArgsError::KeyMismatch { 
				specified: &attr.path,
				required: required_key
			})
		}
		
		match attr.parse_meta() {
			Ok(meta) => {
				match meta {
					Meta::NameValue(mnv) => {
						return Err(AttrArgsError::NoNestedMeta(mnv))
					}
					Meta::Path(_) => {
						// no arguments
					},
					Meta::List(MetaList {
						nested,
						..
					}) => {

						for nested_arg in nested {
							
							match nested_arg {
								NestedMeta::Lit(lit) => {
									return Err(AttrArgsError::LitArg(lit))
								},
								NestedMeta::Meta(meta) => {
									let path = {
										match &meta {
											Meta::Path(path) => path,
											Meta::List(list) => &list.path,
											Meta::NameValue(name_value) => &name_value.path
										}.clone()
									};

									let known_arg_key = {
										if let Some(path_ident) = path.get_ident() {
											if let Some(known_arg_key) = known_arg_keys.iter().find(|arg| &&path_ident == arg) {
												known_arg_key
											} else {
												return Err(AttrArgsError::InvalidArg(path))
											}
										} else {
											return Err(AttrArgsError::InvalidArg(path))
										}
									};
			
									if args.contains_key(known_arg_key) {
										return Err(AttrArgsError::DupliateArg(path))
									}

									match meta {
										Meta::Path(_) => {
											args.insert(known_arg_key, vec![ ]);
										},
										Meta::List(list) => {
											let nesteds = list.nested.into_iter().collect();
											args.insert(known_arg_key, nesteds);
										},
										Meta::NameValue(name_value) => {
											args.insert(known_arg_key, vec![
												NestedMeta::Lit(name_value.lit)
											]);
										}
									}
								}
							}
						}
					}
				}
			},
			_ => {
				return Err(AttrArgsError::NoMeta(attr))
			}
		}
	}

	Ok(args)
}


#[proc_macro_derive(CompositeError, attributes(compound_error))]
pub fn derive_composite_error(input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as DeriveInput);
	let original_input = input.clone();
	let ident = input.ident.clone();
	let generics = input.generics;
	let (generics_impl, generics_type, generics_where) =
		generics.split_for_impl();
	
	let mut from_enums: HashMap<Path, Vec<Ident>> = HashMap::new();
	let mut from_structs: Vec<Path> = Vec::new();
	
	match input.data {
		Data::Enum(data) => {
			for variant in data.variants {
				let variant_ident = variant.ident;
				let field = {
					match variant.fields {
						Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
							fields.unnamed[0].clone()
						},
						_ => {
							return error(&original_input, &format!("Variant '{}' must specify exactly one unnamed field!", variant_ident))
						}
					}
				};
				
				let primitive_type_path = {
					if let Type::Path(ty) = field.ty {
						ty.path
					} else {
						return error(&original_input, &format!("Variant '{}' must specify exactly one unnamed field referencing a type!", variant_ident))
					}
				};

				let mut args = {
					match attr_args(&variant.attrs, "compound_error", &["inline_from"]) {
						Err(AttrArgsError::KeyMismatch { .. }) => continue,
						Err(err) => return err.explain(),
						Ok(ok) => ok
					}
				};
				
				if let Some(from_attr) = args.remove(&"inline_from") {
					for nested in from_attr {
						match nested {
							NestedMeta::Meta(Meta::Path(path)) => {
								from_enums.entry(path).or_default().push(variant_ident.clone());
							},
							_ => return error(&original_input, "'inline_from' attribute must be a list of types!")
						}
					}
				}
				
				from_structs.push(primitive_type_path);
			}
		},
		_ => {
			return error(&original_input, "Can only be used on enums!");
		}
	}
	
	let mut generated = proc_macro2::TokenStream::new();
	
	for from_struct in from_structs {
		let variant_ident = from_struct.segments.last();
		
		let stream = quote! {
			impl #generics_impl From< #from_struct > for #ident #generics_type #generics_where {
				fn from(primitive: #from_struct) -> Self {
					Self::#variant_ident( primitive )
				}
			}
		};
		
		generated.extend(stream);
	}
	
	for (from_enum, variant_idents) in from_enums {
		let mut cases = proc_macro2::TokenStream::new();
		
		for variant_ident in variant_idents {
			cases.extend(quote!{
				#from_enum::#variant_ident( p ) => Self::#variant_ident(p),
			});
		}
		
		let stream = quote! {
			impl #generics_impl From< #from_enum > for #ident #generics_type #generics_where {
				fn from(composite: #from_enum) -> Self {
					match composite {
						#cases
					}
				}
			}
		};
		
		generated.extend(stream);
	}
	
	generated.into()
}
