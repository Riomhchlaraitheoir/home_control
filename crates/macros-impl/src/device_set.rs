use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::parse::{Parse, ParseStream, Parser};
use syn::spanned::Spanned;
use syn::{braced, parse_quote, Attribute, Data, DeriveInput, Expr, ExprAssign, ExprLit, ExprPath, Lit, Member, Meta, MetaList, MetaNameValue, Token};

pub fn device_set(input: DeriveInput) -> syn::Result<TokenStream> {
    let input_span = input.span();
    let name = input.ident;
    let Data::Struct(data) = input.data else {
        return Err(syn::Error::new(
            input_span,
            "can only derive DeviceSet for structs",
        ));
    };
    let members = data.fields.members();
    let member_count = data.fields.len();
    let fields = data
        .fields
        .iter()
        .cloned()
        .enumerate()
        .map(|(i, field)| -> syn::Result<_> {
            let span = field.span();
            let (extra_args, docs) = extra_args(field.attrs)?;
            let mut id = None;
            let mut device_name = None;
            let mut description = None;
            let mut tags_map = None;
            let args: Vec<_> = extra_args.into_iter().filter_map(|arg| {
                match arg {
                    Arg::Normal(name, expr) => match name.to_string().as_str() {
                        "id" => {
                            id = Some(expr);
                            None
                        }
                        "name" => {
                            device_name = Some(expr);
                            None
                        }
                        "description" => {
                            description = Some(expr);
                            None
                        }
                        _ => Some(quote! {
                            .#name(#expr)
                        })
                    },
                    Arg::Tags(tags) => {
                        let inserts = tags.into_iter().map(|[key, value]| {
                            quote!{tags.insert(#key.to_string(), #value.to_string());}
                        });
                        tags_map = Some(quote! {{
                                let mut tags = std::collections::HashMap::<String, String>::new();
                                #(#inserts)*
                                tags
                            }
                        });
                        None
                    }
                }
            }).collect();
            let id = match (id, &field.ident) {
                (None, None) => {
                    return Err(syn::Error::new(span, "explicit id param required for unnamed fields"))
                }
                (None, Some(name)) => {
                    let name = name.to_string();
                    parse_quote!(#name)
                }
                (Some(id), _) => id
            };
            let device_name = device_name.unwrap_or_else(|| id.clone());
            let description = description.unwrap_or_else(|| if docs.is_empty() {
                parse_quote!(None)
            } else {
                parse_quote! {
                    Some(String::from(#docs))
                }
            });
            let tags = tags_map.unwrap_or_else(|| parse_quote! {
                std::collections::HashMap::<String, String>::default()
            });

            let member = if let Some(name) = field.ident {
                Member::Named(name)
            } else {
                Member::Unnamed(i.into())
            };
            let ty = field.ty;
            Ok(quote! {
                #member: #ty::create()
                    .manager(manager.device_manager()?)
                    .info(::home_control::reflect::DeviceInfo {
                        id: #id.to_string(),
                        name: #device_name.to_string(),
                        description: #description,
                        tags: #tags,
                    })
                    #(#args)*
                    .call()
                    .await?
            })
        })
        .collect::<syn::Result<Vec<_>>>()?;
    Ok(quote! {
        impl ::home_control::device::DeviceSet for #name {
            async fn new(manager: &mut ::home_control::Manager<'_>) -> Result<Self, ::home_control::device::CreateDeviceError> {
                Ok(Self {
                    #(#fields),*
                })
            }
        }

        impl IntoIterator for #name {
            type Item = Box<dyn ::home_control::reflect::Device>;
            type IntoIter = std::array::IntoIter<Box<dyn ::home_control::reflect::Device>, #member_count>;

            fn into_iter(self) -> Self::IntoIter {
                [
                    #(
                    Box::new(self.#members) as Box<dyn ::home_control::reflect::Device>
                    ),*
                ].into_iter()
            }
        }
    })
}

fn extra_args(attrs: Vec<Attribute>) -> Result<(Vec<Arg>, String), syn::Error> {
    let mut args = Vec::new();
    let mut docs = Vec::new();
    for attr in attrs {
        let attr_span = attr.span();
        match attr.meta {
            Meta::Path(path) => {
                if path.segments.len() != 1
                    || path
                        .segments
                        .first()
                        .ok_or(syn::Error::new(attr_span, "expected at least one segment"))?
                        .ident
                        != "device"
                {
                    continue;
                }
            }
            Meta::List(MetaList {
                path,
                delimiter: _,
                tokens,
            }) => {
                if path.segments.len() != 1
                    || path
                        .segments
                        .first()
                        .ok_or(syn::Error::new(attr_span, "expected at least one segment"))?
                        .ident
                        != "device"
                {
                    continue;
                }
                let parser = |input: ParseStream| -> syn::Result<Vec<Arg>> {
                    let arg: Arg = input.parse()?;
                    let mut args = vec![arg];
                    while input.peek(Token![,]) {
                        input.parse::<Token![,]>()?;
                        if input.is_empty() {
                            break;
                        }
                        let arg: Arg = input.parse()?;
                        args.push(arg);
                    }
                    Ok(args)
                };
                let mut parsed = parser.parse2(tokens)?;
                args.append(&mut parsed);
            }
            Meta::NameValue(MetaNameValue {
                path,
                eq_token: _,
                value,
            }) => {
                if path.segments.len() != 1 {
                    continue;
                }
                if let Some(ident) = path.get_ident()
                    && ident == "doc"
                    && let Expr::Lit(lit) = value
                    && let ExprLit { lit, .. } = lit
                    && let Lit::Str(string) = lit {
                    docs.push(string.value().trim().to_string())
                }
            }
        }
    }
    Ok((args, docs.join("\n")))
}

enum Arg {
    Normal(Ident, Expr),
    Tags(Vec<[Expr; 2]>)
}

impl Parse for Arg {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: Ident = input.parse()?;
        input.parse::<Token![=]>()?;
        if name != "tags" {
            let expr = input.parse()?;
            return Ok(Self::Normal(name, expr))
        }
        let tags;
        braced!(tags in input);
        let mut values = vec![];
        while !tags.is_empty() {
            let key = tags.parse::<Expr>()?;
            if let Expr::Assign(ExprAssign { left, right, .. }) = &key &&
                let Expr::Path(ExprPath { path, .. }) = &**left &&
                let Some(ident) = path.get_ident() {
                values.push([parse_quote!(stringify!(#ident)), *right.clone()]);
                break
            }
            input.parse::<Token![=]>()?;
            let value = tags.parse::<Expr>()?;
            values.push([key, value]);
            if tags.is_empty() {
                break
            }
            let _ = input.parse::<Token![,]>()?;
        }
        Ok(Self::Tags(values))
    }
}
