use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::parse::{Parse, ParseStream, Parser};
use syn::{
    Attribute, Data, DeriveInput, Expr, Member, Meta, MetaList, MetaNameValue, Token, parse_quote,
};

pub fn device_set(input: DeriveInput) -> TokenStream {
    let name = input.ident;
    let Data::Struct(data) = input.data else {
        return quote! {
            ::core::compile_error!("can only derive DeviceSet for structs");
        };
    };
    let fields = data.fields.into_iter().enumerate().map(|(i, field)| {
        let mut extra_args = extra_args(field.attrs).expect("failed to parse attributes");
        if !extra_args
            .iter()
            .any(|(name, _)| name == "name")
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
        quote! {
            #member: #ty::create()
                .manager(manager)
                #(#args)*
                .call()?
        }
    });
    quote! {
        impl ::home_control::DeviceSet for #name {
            fn new(manager: &mut ::home_control::Manager) -> Result<Self, Box<dyn ::std::error::Error>> {
                Ok(Self {
                    #(#fields),*
                })
            }
        }
    }
}

fn extra_args(attrs: Vec<Attribute>) -> Result<Vec<(Ident, Expr)>, syn::Error> {
    let mut args = Vec::new();
    for attr in attrs {
        match attr.meta {
            Meta::Path(path) => {
                if path.segments.len() != 1
                    || path
                        .segments
                        .first()
                        .expect("just checked len")
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
                        .expect("just checked len")
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
                    path.segments.first().expect("checked len").ident.clone(),
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
