extern crate proc_macro;

use std::collections::HashMap;
use syn::parse_macro_input;
use syn::DeriveInput;
use syn::Data;
use syn::AttrStyle;
use syn::Meta;
use syn::NestedMeta;
use syn::Fields;
use syn::Type;
use syn::Path;
use syn::Ident;
use quote::quote;
use proc_macro_error::emit_error;
use proc_macro_error::abort_if_dirty;
use proc_macro_error::proc_macro_error;
use proc_macro::TokenStream;

#[proc_macro_derive(CompositeError, attributes(from))]
#[proc_macro_error]
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
							emit_error!(original_input, "Variant '{}' must specify exactly one unnamed field!", variant_ident.clone());
							continue
						}
					}
				};
				
				let primitive_type_path = {
					if let Type::Path(ty) = field.ty {
						ty.path
					} else {
						emit_error!(original_input, "Variant '{}' must specify exactly one unnamed field referencing a type!", variant_ident.clone());
						continue
					}
				};
				
				let from_attr = variant.attrs.into_iter().find(|attr| {
					if let AttrStyle::Outer = attr.style {
						attr.path.segments[0].ident == "from"
					} else {
						false
					}
				});
				
				if let Some(from_attr) = from_attr {
					// "from" attr is present
					
					// TODO
					
					match from_attr.parse_meta().unwrap() {
						Meta::List(list) => {
							for nested in list.nested {
								match nested {
									NestedMeta::Meta(Meta::Path(path)) => {
											from_enums.entry(path).or_default().push(variant_ident.clone());
									},
									_ => emit_error!(original_input, "from attribute must be a list of types!")
								}
							}
						},
						_ => emit_error!(original_input, "from attribute must be a list!")
					}
				}
				
				from_structs.push(primitive_type_path);
			}
		},
		_ => {
			emit_error!(original_input, "Can only be used on enums!");
		}
	}
	
	abort_if_dirty();
	
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
