extern crate proc_macro;

mod util;

use std::collections::HashMap;

use proc_macro::TokenStream;
use quote::quote;
use syn::parse_macro_input;
use syn::Data;
use syn::DeriveInput;
use syn::Fields;
use syn::Ident;
use syn::Meta;
use syn::NestedMeta;
use syn::Path;
use syn::Type;
use util::attr_args;
use util::error;
use util::flag;

macro_rules! try_compile {
	($what:expr, | $err:ident | $ret:expr) => {{
		match $what {
			Err($err) => return $ret,
			Ok(ok) => ok,
		}
	}};
}

macro_rules! flag {
	($args:expr, $arg:expr) => {
		try_compile!(flag($args, $arg), |path| {
			error(path, &format!("'{}' attribute takes no arguments!", $arg))
		})
	};
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
enum PathOrLit {
	Path(syn::Path),
	Lit(syn::TypePath),
}

impl PathOrLit {
	fn path(&self) -> syn::Path {
		let mut path = {
			match self {
				Self::Path(path) => path,
				Self::Lit(type_path) => &type_path.path,
			}
			.clone()
		};

		path.segments.last_mut().unwrap().arguments = syn::PathArguments::None;
		path
	}
}

impl quote::ToTokens for PathOrLit {
	fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
		match self {
			Self::Path(path) => path.to_tokens(tokens),
			Self::Lit(ty) => ty.to_tokens(tokens),
		}
	}
}

/// Implement `CompoundError` functionality for the target type.
///
/// If the target is an enum, `From` is implemented for each variant.
/// Additionally, variants can be annotated with
/// `#[compound_error( inline_from(X) )]`
/// to specify that an "inlining from `X`" should be implemented. In addition
/// to the `From` impls, by default also `std::error::Error` and
/// `std::fmt::Display` are implemented for the target type. If the target type
/// is a struct, no `From` impls, but only `std::error::Error` and
/// `std::fmt::Display` impls are generated.
///
/// The generation of the `Error` and `Display` impls can by suppressed by
/// specifying `#[compound_error( skip_error )]` and
/// `#[compound_error( skip_display )]` on the target type.
///
/// If the target type is an enum, all variants must take exactly one argument.
/// By default, this argument must implement `std::error::Error`. This can be
/// circumvented by either specifying the `skip_error` attribute on the target
/// type or by specifying the `no_source` attribute on the respective variant.
/// `no_source` causes `None` to be returned by the implementation of
/// `std::error::Error::source()` on the target type for the respective enum
/// variant.
///
/// # Attributes
///
/// Attributes are specified in the following form:
///
/// ```text
/// #[compound_error( attr1, attr2, attr3, ... )]
/// #[compound_error( attr4, attr5, ... )]
/// <ELEMENT>
/// ```
///
/// `<ELEMENT>` can be the target type or an enum variant. The following
/// attributes are available:
///
/// On the target type:
/// * `title = "<title>"`: Set the title of this error to `"<title>"`. This is
///   relevant for the automatic `Display` implementation on the target type.
/// * `description = "<description>"`: Set the description of this error to
///   `"<description>"`. This is relevant for the automatic `Display`
///   implementation on the target type.
/// * `skip_display`: Skip the automatic implementation of `std::fmt::Display`
///   on the target type.
/// * `skip_error`: Skip the automatic implementation of `std::error::Error` on
///   the target type.
/// * `transparent`: forward the source and Display methods through to all
///   underlying errors without adding an additional message.
///
/// On each enum variant:
/// * `inline_from(A,B,C,...)`: Inline the Errors `A`, `B`, `C`, ... in the
///   target type.
/// * `no_source`: Return `None` from `<Self as std::error::Error>::source()`
///   for this enum variant. This lifts the requirement that `std::error::Error`
///   is implemented for the argument of this variant.
/// * `convert_source(fn)`: Applies `fn` to the error of this variant before
///   returing it from `<Self as std::error::Error>::source()`
/// * `transparent`: forward the source and Display methods through to the
///   argument of this variant without adding an additional message.
///
#[proc_macro_derive(CompoundError, attributes(compound_error))]
pub fn derive_compound_error(input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as DeriveInput);
	let original_input = input.clone();
	let ident = input.ident.clone();
	let generics = input.generics;
	let (generics_impl, generics_type, generics_where) = generics.split_for_impl();

	let mut toplevel_args = try_compile!(
		attr_args(
			&input.attrs,
			"compound_error",
			&[
				"title",
				"description",
				"skip_display",
				"skip_error",
				"transparent"
			]
		),
		|err| err.explain()
	);

	let title_attr = toplevel_args.remove(&"title");
	let title = {
		if let Some(attr) = title_attr {
			if attr.values.len() != 1 {
				return error(&attr.path, "'title' takes exactly one string argument!");
			}
			match &attr.values[0] {
				NestedMeta::Lit(syn::Lit::Str(lit)) => lit.value(),
				_ => return error(&attr.path, "'title' argument must be a string!"),
			}
		} else {
			ident.to_string()
		}
	};

	let description_attr = toplevel_args.remove(&"description");
	let description = {
		if let Some(attr) = description_attr {
			if attr.values.len() != 1 {
				return error(
					&attr.path,
					"'description' takes exactly one string argument!",
				);
			}
			match &attr.values[0] {
				NestedMeta::Lit(syn::Lit::Str(lit)) => {
					format!(" ({})", lit.value())
				},
				_ => return error(&attr.path, "'description' argument must be a string!"),
			}
		} else {
			"".into()
		}
	};

	let skip_display = flag!(&toplevel_args, &"skip_display");
	let skip_error = flag!(&toplevel_args, &"skip_error");

	#[allow(unused_assignments)]
	let mut err_source = proc_macro2::TokenStream::new();
	let mut from_enums: HashMap<PathOrLit, Vec<Ident>> = HashMap::new();
	let mut from_structs: Vec<(Path, Ident)> = Vec::new();

	#[allow(unused_assignments)]
	let mut display = proc_macro2::TokenStream::new();

	match input.data {
		Data::Enum(data) => {
			let transparent_enum = flag!(&toplevel_args, &"transparent");

			let mut err_sources = proc_macro2::TokenStream::new();

			let mut display_cases = Vec::new();

			for variant in data.variants {
				let variant_ident = variant.ident;
				let variant_ident_str = variant_ident.to_string();
				let field = {
					match variant.fields {
						Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
							fields.unnamed[0].clone()
						},
						_ => {
							return error(
								&original_input,
								&format!(
									"Variant '{}' must specify exactly one unnamed field!",
									variant_ident
								),
							)
						},
					}
				};

				let primitive_type_path = {
					if let Type::Path(ty) = field.ty {
						ty.path
					} else {
						return error(
							&original_input,
							&format!(
								"Variant '{}' must specify exactly one unnamed field referencing \
								 a type!",
								variant_ident
							),
						);
					}
				};

				let mut args = {
					match attr_args(
						&variant.attrs,
						"compound_error",
						&[
							"inline_from",
							"skip_single_from",
							"no_source",
							"convert_source",
							"transparent",
						],
					) {
						Err(err) => return err.explain(),
						Ok(ok) => ok,
					}
				};

				if let Some(from_attr) = args.remove(&"inline_from") {
					for nested in from_attr.values {
						match nested {
							NestedMeta::Meta(Meta::Path(path)) => {
								from_enums
									.entry(PathOrLit::Path(path))
									.or_default()
									.push(variant_ident.clone());
							},
							NestedMeta::Lit(syn::Lit::Str(lit)) => {
								let parsed_ty = {
									match lit.parse() {
										Err(_) => {
											return error(
												&from_attr.path,
												"'inline_from' attribute must be a list of types!",
											)
										},
										Ok(ok) => ok,
									}
								};
								from_enums
									.entry(PathOrLit::Lit(parsed_ty))
									.or_default()
									.push(variant_ident.clone());
							},
							_ => {
								return error(
									&from_attr.path,
									"'inline_from' attribute must be a list of types!",
								)
							},
						}
					}
				}

				let skip_single_from = flag!(&args, &"skip_single_from");
				let transparent = flag!(&args, &"transparent") || transparent_enum;

				// If it's not a pure generic variant, implement from
				if !skip_single_from
					&& !generics
						.type_params()
						.any(|p| primitive_type_path.is_ident(&p.ident))
				{
					from_structs.push((primitive_type_path, variant_ident.clone()));
				}

				let variant_display;

				let no_source = flag!(&args, &"no_source");

				if !no_source {
					let src_ret = {
						if let Some(convert_source_attr) = args.remove(&"convert_source") {
							if convert_source_attr.values.len() != 1 {
								return crate::error(
									&convert_source_attr.path,
									"'convert_source' takes exactly one argument!",
								);
							}

							match &convert_source_attr.values[0] {
								NestedMeta::Meta(Meta::Path(path)) => {
									quote!( #path (x) )
								},
								_ => {
									return crate::error(
										&convert_source_attr.path,
										"The argument of 'convert_source' must be a path!",
									)
								},
							}
						} else {
							quote!(x)
						}
					};

					variant_display = quote!(x);

					if transparent {
						err_sources.extend(quote! {
							Self::#variant_ident(x) => std::error::Error::source(x),
						});
					} else {
						err_sources.extend(quote! {
							Self::#variant_ident(x) => Some( #src_ret ),
						});
					}
				} else {
					variant_display = quote!(#variant_ident_str);
				}

				if transparent {
					display_cases.push(quote! {
						Self::#variant_ident (x) => {
							std::fmt::Display::fmt(x, f)?;
						}
					});
				} else {
					display_cases.push(quote! {
						Self::#variant_ident (x) => {
							writeln!(f, "{}{}:", #title, #description)?;
							write!(f, "  └ {}", #variant_display)?;
						}
					});
				}
			}

			display_cases.push(quote! {
				_ => {}
			});

			display = quote! {
				match self {
					#(#display_cases),*
				}
				Ok(())
			};

			err_source = quote! {
				match self {
					#err_sources
					_ => ::core::option::Option::None
				}
			};
		},
		Data::Struct(_) => {
			display = quote! {
				write!(f, "{}{}", #title, #description)
			};

			err_source = quote!(None);
		},
		_ => {
			return error(&original_input, "Can only be used on enums!");
		},
	}

	let mut generated = proc_macro2::TokenStream::new();

	for (from_struct, variant_ident) in from_structs {
		let stream = quote! {
			#[automatically_derived]
			impl #generics_impl ::core::convert::From< #from_struct > for #ident #generics_type #generics_where {
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
			cases.extend(quote! {
				#from_enum_path::#variant_ident( p ) => Self::#variant_ident(p),
			});
		}

		let stream = quote! {
			#[automatically_derived]
			impl #generics_impl ::core::convert::From< #from_enum > for #ident #generics_type #generics_where {
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
		generated.extend(quote! {
			#[automatically_derived]
			impl #generics_impl ::core::fmt::Display for #ident #generics_type #generics_where {
				fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
					#display
				}
			}
		});
	}

	// BTW: requires `std`
	if !skip_error {
		generated.extend(quote! {
			#[automatically_derived]
			impl #generics_impl ::std::error::Error for #ident #generics_type #generics_where {
				fn source(&self) -> ::std::option::Option<&(dyn ::std::error::Error + 'static)> {
					#err_source
				}
			}
		});
	}

	generated.into()
}
