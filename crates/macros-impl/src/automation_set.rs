use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;
use syn::{parse_quote, ExprField, Index, Member};

pub fn automation_sets(max: usize) -> TokenStream {
    (1..=max).map(automation_set).collect()
}

fn automation_set(size:usize) -> TokenStream {
    let types: Vec<_> = (1..=size).map(|x| Ident::new(&format!("A{x}"), Span::call_site())).collect();
    let assertions = types.iter().map(|ty| quote! { #ty: AutomationSet<'a> });
    let indices: Vec<_> = (0..size).map(|i| {
        let mut index: ExprField = parse_quote! { self.0 };
        index.member = Member::Unnamed(Index {
            index: i as u32,
            span: Span::call_site(),
        });
        index
    }).collect();
    quote! {

impl<'a, #(#types),*> AutomationSet<'a> for (#(#types,)*)
where
        #(#assertions),*
{
    fn futures<'b>(&'b mut self, futures: &mut Vec<BoxFuture<'b, ()>>) where 'a: 'b {
        #(
            #indices.futures(futures);
        )*;
    }

    fn size(&self) -> usize {
        let mut count = 0;
        #(count+=#indices.size();)*
        count
    }
}

    }
}