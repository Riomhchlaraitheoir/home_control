use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::parse::{Parse, ParseStream, Parser};
use syn::spanned::Spanned;
use syn::{
    Attribute, Data, DeriveInput, Expr, Member, Meta, MetaList, MetaNameValue, Token, parse_quote,
};

pub fn device_set(input: DeriveInput) -> syn::Result<TokenStream> {
    let input_span = input.span();
    let name = input.ident;
    let Data::Struct(data) = input.data else {
        return Err(syn::Error::new(
            input_span,
            "can only derive DeviceSet for structs",
        ));
    };
    let fields = data
        .fields
        .into_iter()
        .enumerate()
        .map(|(i, field)| -> syn::Result<_> {
            let mut extra_args = extra_args(field.attrs)?;
            if !extra_args.iter().any(|(name, _)| name == "name")
                && let Some(name) = field.ident.clone()
            {
                extra_args.push((
                    Ident::new("name", name.span()),
                    parse_quote!(::core::stringify!(#name).to_string()),
                ))
            }
            let args = extra_args.into_iter().map(|(name, expr)| {
                quote! {
                    .#name(#expr)
                }
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
                    #(#args)*
                    .call()
                    .await?
            })
        })
        .collect::<syn::Result<Vec<_>>>()?;
    Ok(quote! {
        impl ::home_control::device::DeviceSet for #name {
            async fn new(manager: &mut ::home_control::manager::Manager) -> Result<Self, ::home_control::device::CreateDeviceError> {
                Ok(Self {
                    #(#fields),*
                })
            }
        }
    })
}

fn extra_args(attrs: Vec<Attribute>) -> Result<Vec<(Ident, Expr)>, syn::Error> {
    let mut args = Vec::new();
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
                let parser = |input: ParseStream| -> syn::Result<Vec<(Ident, Expr)>> {
                    let arg: Arg = input.parse()?;
                    let mut args = vec![(arg.0, arg.1)];
                    while input.peek(Token![,]) {
                        input.parse::<Token![,]>()?;
                        if input.is_empty() {
                            break;
                        }
                        let arg: Arg = input.parse()?;
                        args.push((arg.0, arg.1))
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
                args.push((
                    path.segments
                        .first()
                        .ok_or(syn::Error::new(attr_span, "expected at least one segment"))?
                        .ident
                        .clone(),
                    value,
                ))
            }
        }
    }
    Ok(args)
}

struct Arg(Ident, Expr);

impl Parse for Arg {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name = input.parse()?;
        input.parse::<Token![=]>()?;
        let expr = input.parse()?;
        Ok(Self(name, expr))
    }
}
