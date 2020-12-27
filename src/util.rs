
use std::collections::HashMap;
use std::hash::Hash;

use proc_macro::TokenStream;

use quote::quote_spanned;

use syn::Meta;
use syn::MetaList;
use syn::MetaNameValue;
use syn::NestedMeta;

pub fn error(spanned: &impl syn::spanned::Spanned, message: &str) -> TokenStream {
	let span = spanned.span();
	let output = quote_spanned! {
		span => compile_error!(#message);
	};

	output.into()
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum AttrArgsError<'attr> {
	InvalidArg(syn::Path),
	LitArg(syn::Lit),
	DupliateArg(syn::Path),
	NoMeta(&'attr syn::Attribute),
	NoNestedMeta(MetaNameValue)
}

impl<'attr> AttrArgsError<'attr> {
	pub fn explain(self) -> TokenStream {
		match self {
			Self::InvalidArg(key) => error(&key, "Invalid argument."),
			Self::LitArg(lit) => error(&lit, "Invalid first-level literal."),
			Self::DupliateArg(key) => error(&key, "Duplicate argument."),
			Self::NoMeta(attr) => error(attr, "Must be of 'meta' type."),
			Self::NoNestedMeta(mnv) => error(&mnv, "Invalid name-value pair. Expected path or list."),
		}
	}
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct AttrArg {
	pub path: syn::Path,
	pub values: Vec<NestedMeta>
}

impl AttrArg {
	pub fn new(
		path: syn::Path,
		values: Vec<NestedMeta>
	) -> Self {
		Self {
			path,
			values
		}
	}
}

pub fn attr_args<'attr, 'ident, I: ?Sized>(
	attrs: &'attr [syn::Attribute],
	required_key: &'ident I,
	known_arg_keys: &[&'ident I]
) -> Result<HashMap<&'ident I, AttrArg>, AttrArgsError<'attr>> where
	syn::Ident: PartialEq<I>,
	I: Hash + Eq
{
	let mut args: HashMap<&'ident I, AttrArg> = HashMap::new();

	for attr in attrs {
		if let Some(specified_key) = attr.path.get_ident() {
			if specified_key != required_key {
				continue
			}
		} else {
			continue
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
											args.insert(known_arg_key, AttrArg::new(path, vec![ ]));
										},
										Meta::List(list) => {
											let nesteds = list.nested.into_iter().collect();
											args.insert(known_arg_key, AttrArg::new(path, nesteds));
										},
										Meta::NameValue(name_value) => {
											args.insert(known_arg_key, AttrArg::new(path, vec![
												NestedMeta::Lit(name_value.lit)
											]));
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

pub fn flag<'a, I: ?Sized + Eq + Hash>(args: &'a HashMap<&I, AttrArg>, ident: &I) -> Result<bool, &'a syn::Path> {
	if let Some(skip) = args.get(ident) {
		if !skip.values.is_empty() {
			return Err(&skip.path)
		}
		
		Ok(true)
	} else {
		Ok(false)
	}
}

