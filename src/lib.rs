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
use util::AttrArgsError;

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
