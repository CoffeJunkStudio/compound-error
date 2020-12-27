extern crate proc_macro;

mod util;

use std::collections::HashMap;

use syn::parse_macro_input;
use syn::DeriveInput;
use syn::Data;
use syn::Meta;
use syn::NestedMeta;
use syn::Fields;
use syn::Type;
use syn::Path;
use syn::Ident;
use quote::quote;
use proc_macro::TokenStream;

use util::error;
use util::attr_args;
use util::flag;

macro_rules! try_compile {
	( $what:expr, | $err:ident | $ret:expr ) => {
		{
			match $what {
				Err($err) => return $ret,
				Ok(ok) => ok
			}
		}
	}
}

macro_rules! flag {
	( $args:expr, $arg:expr ) => {
		try_compile!(
			flag($args, $arg),
			|path| error(path, &format!("'{}' attribute takes no arguments!", $arg))
		);
	}
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
enum PathOrLit {
	Path(syn::Path),
	Lit(syn::TypePath)
}

impl PathOrLit {
	fn path(&self) -> syn::Path {
		let mut path = {
			match self {
				Self::Path(path) => path,
				Self::Lit(type_path) => &type_path.path
			}.clone()
		};

		path.segments.last_mut().unwrap().arguments = syn::PathArguments::None;
		path
	}
}

impl quote::ToTokens for PathOrLit {
	fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
		match self {
			Self::Path(path) => path.to_tokens(tokens),
			Self::Lit(ty) => ty.to_tokens(tokens)
		}
	}
}

#[proc_macro_derive(CompoundError, attributes(compound_error))]
pub fn derive_compound_error(input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as DeriveInput);
	let original_input = input.clone();
	let ident = input.ident.clone();
	let generics = input.generics;
	let (generics_impl, generics_type, generics_where) =
		generics.split_for_impl();
	
	let mut toplevel_args = try_compile!(
		attr_args(&input.attrs, "compound_error", &["title", "description", "skip_display", "skip_error"]),
		|err| err.explain()
	);

	let title_attr = toplevel_args.remove(&"title");
	let title = {
		if let Some(attr) = title_attr {
			if attr.values.len() != 1 {
				return error(&attr.path, "'title' takes exactly one string argument!")
			}
			match &attr.values[0] {
				NestedMeta::Lit(syn::Lit::Str(lit)) => {
					lit.value()
				},
				_ => return error(&attr.path, "'title' argument must be a string!")
			}
		} else {
			ident.to_string()
		}
	};

	let description_attr = toplevel_args.remove(&"description");
	let description = {
		if let Some(attr) = description_attr {
			if attr.values.len() != 1 {
				return error(&attr.path, "'description' takes exactly one string argument!")
			}
			match &attr.values[0] {
				NestedMeta::Lit(syn::Lit::Str(lit)) => {
					format!(" ({})", lit.value())
				},
				_ => return error(&attr.path, "'description' argument must be a string!")
			}
		} else {
			"".into()
		}
	};

	let skip_display = flag!(&toplevel_args, &"skip_display");
	let skip_error = flag!(&toplevel_args, &"skip_error");
	
	let mut err_source = proc_macro2::TokenStream::new();
	let mut from_enums: HashMap<PathOrLit, Vec<Ident>> = HashMap::new();
	let mut from_structs: Vec<(Path, Ident)> = Vec::new();
	
	match input.data {
		Data::Enum(data) => {
			let mut err_sources = proc_macro2::TokenStream::new();

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
					match attr_args(&variant.attrs, "compound_error", &["inline_from", "skip_single_from", "no_source"]) {
						Err(err) => return err.explain(),
						Ok(ok) => ok
					}
				};
				
				if let Some(from_attr) = args.remove(&"inline_from") {
					for nested in from_attr.values {
						match nested {
							NestedMeta::Meta(Meta::Path(path)) => {
								from_enums.entry(PathOrLit::Path(path)).or_default().push(variant_ident.clone());
							},
							NestedMeta::Lit(syn::Lit::Str(lit)) => {
								let parsed_ty = {
									match lit.parse() {
										Err(_) => return error(&from_attr.path, "'inline_from' attribute must be a list of types!"),
										Ok(ok) => ok
									}
								};
								from_enums.entry(PathOrLit::Lit(parsed_ty)).or_default().push(variant_ident.clone());
							}
							_ => return error(&from_attr.path, "'inline_from' attribute must be a list of types!")
						}
					}
				}

				let skip_single_from = flag!(&args, &"skip_single_from");
				
				// If it's not a pure generic variant, implement from
				if !skip_single_from && !generics.type_params().any(|p| primitive_type_path.is_ident(&p.ident)) {
					from_structs.push((primitive_type_path, variant_ident.clone()));
				}

				let no_source = flag!(&args, &"no_source");

				if !no_source {
					err_sources.extend( quote! {
						Self::#variant_ident(x) => Some(x),
					} );
				}
			}

			err_source = quote! {
				match self {
					#err_sources
					_ => None
				}
			};
		},
		Data::Struct(_) => {
			err_source = quote!( None );
		}
		_ => {
			return error(&original_input, "Can only be used on enums!");
		}
	}
	
	let mut generated = proc_macro2::TokenStream::new();
	
	for (from_struct, variant_ident) in from_structs {
		let stream = quote! {
			#[automatically_derived]
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
		let from_enum_path = from_enum.path();
		
		for variant_ident in variant_idents {
			cases.extend(quote!{
				#from_enum_path::#variant_ident( p ) => Self::#variant_ident(p),
			});
		}
		
		let stream = quote! {
			#[automatically_derived]
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

	if !skip_display {
		generated.extend( quote! {
			#[automatically_derived]
			impl #generics_impl std::fmt::Display for #ident #generics_type #generics_where {
				fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
					write!(f, "{}{}", #title, #description) // TODO
				}
			}
		} );
	}

	if !skip_error {
		generated.extend( quote! {
			#[automatically_derived]
			impl #generics_impl std::error::Error for #ident #generics_type #generics_where {
				fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
					#err_source
				}
			}
		} );
	}
	
	generated.into()
}
