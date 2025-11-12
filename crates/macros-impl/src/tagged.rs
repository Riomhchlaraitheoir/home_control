use convert_case::{Case, Casing};
use proc_macro2::{Ident, Span, TokenStream};
use quote::{quote, ToTokens, TokenStreamExt};
use std::collections::{HashMap, HashSet};
use syn::parse::{ParseStream, Parser};
use syn::spanned::Spanned;
use syn::{
    Attribute, Data, DeriveInput, Expr, ExprStruct, FieldValue, Fields, ImplItem, ImplItemFn,
    Index, Item, ItemImpl, ItemStruct, Member, Meta, MetaList, MetaNameValue, Path, Token, Type,
    TypePath, Visibility, parse_quote,
};

pub fn tagged(args: TokenStream, input: ItemStruct) -> TokenStream {
    if !args.is_empty() {
        return syn::Error::new(args.span(), "#[tagged] does not accept arguments")
            .to_compile_error();
    }
    match tagged_impl(input) {
        Ok(items) => items.into_iter().map(ToTokens::into_token_stream).collect(),
        Err(error) => error.into_compile_error(),
    }
}

pub fn tagged_impl(mut input: ItemStruct) -> syn::Result<Vec<Item>> {
    // let Fields::Named(fields) = input.fields else {
    //     return Err(syn::Error::new(input.fields.span(), "Only named fields are supported."))
    // };
    let mut tag_sets = HashMap::new();
    for (i, field) in input.fields.iter_mut().enumerate() {
        for tag in tags(&mut field.attrs)? {
            let fields = &mut tag_sets
                .entry(tag.to_string().to_lowercase())
                .or_insert(Tag::new(tag.to_string(), &input.vis))
                .fields;
            fields.push(Field {
                name: field.ident.clone(),
                member: match &field.ident {
                    None => Member::Unnamed(Index {
                        index: fields.len() as u32,
                        span: field.span(),
                    }),
                    Some(name) => Member::Named(name.clone()),
                },
                original_member: match &field.ident {
                    None => Member::Unnamed(Index {
                        index: i as u32,
                        span: field.span(),
                    }),
                    Some(name) => Member::Named(name.clone()),
                },
                ty: field.ty.clone(),
            });
        }
    }

    let tag_type = {
        let vis = &input.vis;
        let tag_type = Ident::new(format!("{}Tag", input.ident).as_str(), input.ident.span());
        let variants = tag_sets.values().map(|tag| tag.pascal_name);
        parse_quote! {
        #vis enum #tag_type {
            #(#variants),*
        }
    }
    };

    let impl_block = {
        let tag_type = tag_type.ident;
        let name = &input.ident;
        parse_quote! {
            impl #name {
                fn with_tag<'a, T>(&'a self, tags: #tag_type) -> impl IntoIterator<Item = T> {

                }
            }
        }
    };
    let mut items = vec![Item::Struct(input), Item::Enum(tag_type), Item::Impl(impl_block)];
    items.extend(tag_sets.values().flat_map(Tag::items));
    Ok(items)
}

fn tags(attrs: &mut Vec<Attribute>) -> syn::Result<HashSet<Ident>> {
    let mut tags = HashSet::new();
    *attrs = attrs
        .into_iter()
        .filter_map(|attr| {
            match &attr.meta {
                Meta::Path(_) => {
                    return Some(Err(syn::Error::new(attr.span(), "tag must be specified")));
                }
                Meta::List(MetaList { path, tokens, .. }) => {
                    if !path.is_ident("tag") {
                        return Some(Ok(attr.clone()));
                    }
                    let parser = |input: ParseStream| -> syn::Result<Vec<Ident>> {
                        let idents =
                            syn::punctuated::Punctuated::<Ident, Token![,]>::parse_terminated(
                                input,
                            )?;
                        Ok(idents.into_iter().collect())
                    };
                    let idents = match parser.parse2(tokens.clone()) {
                        Ok(idents) => idents,
                        Err(error) => return Some(Err(error)),
                    };
                    tags.extend(idents);
                }
                Meta::NameValue(MetaNameValue { path, value, .. }) => {
                    if !path.is_ident("tag") {
                        return Some(Ok(attr.clone()));
                    }
                    let Expr::Path(value) = value else {
                        return Some(Err(syn::Error::new(
                            value.span(),
                            "tag must be an identifier",
                        )));
                    };
                    tags.insert(
                        match value.path.require_ident() {
                            Ok(ident) => ident,
                            Err(error) => return Some(Err(error)),
                        }
                            .clone(),
                    );
                }
            }
            None
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(tags)
}

struct Tag {
    visibility: Visibility,
    pascal_name: String,
    snake_name: String,
    fields: Vec<Field>,
}

#[derive(Debug, Clone)]
struct Field {
    name: Option<Ident>,
    member: Member,
    original_member: Member,
    ty: Type,
}

impl Tag {
    fn new(name: String, visibility: &Visibility) -> Self {
        Self {
            visibility: visibility.clone(),
            pascal_name: name.to_case(Case::Pascal),
            snake_name: name.to_case(Case::Snake),
            fields: vec![],
        }
    }

    fn items(&self) -> Vec<Item> {
        let vis = &self.visibility;
        let name = Ident::new(&self.pascal_name, Span::call_site());
        let fields = self.fields.iter().map(|field| -> syn::Field {
            let ty = &field.ty;
            let name = field.name.iter();
            parse_quote! { #(#name: )*&'a #ty }
        });
        let struc = parse_quote! {
            #[derive(Clone, Copy)]
            #vis struct #name<'a> {
                #(#fields),*
            }
        };
        let members = self.fields.iter().map(|field| &field.member).collect::<Vec<_>>();
        let clauses = self.fields.iter().map(|field| {
            let ty = &field.ty;
            quote! {
                &'a #ty: Into<T>
            }
        }).collect::<Vec<_>>();
        let impl_block = parse_quote! {
            impl<'a> #name<'a> {
                fn all<T>(self) -> impl IntoIterator<Item = T>
                where
                    #(#clauses),*
                {
                    [
                        #(self.#members.into()),*
                    ]
                }
            }
        };
        vec![Item::Struct(struc), Item::Impl(impl_block)]
    }

    fn function(&self) -> ImplItemFn {
        let vis = &self.visibility;
        let ty = Ident::new(&self.pascal_name, Span::call_site());
        let name = Ident::new(&self.snake_name, Span::call_site());
        let members = self
            .fields
            .iter()
            .enumerate()
            .map(|(i, field)| -> FieldValue {
                let original = &field.original_member;
                let member = match original {
                    Member::Unnamed(_) => Member::Unnamed(Index {
                        index: i as u32,
                        span: Span::call_site(),
                    }),
                    Member::Named(name) => Member::Named(name.clone()),
                };
                parse_quote!(#member : &self.#member)
            });
        parse_quote! {
            #vis fn #name(&self) -> #ty<'_> {
                #ty {
                    #(#members),*
                }
            }
        }
    }
}
