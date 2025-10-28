use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput};

pub fn device_set(input: DeriveInput) -> TokenStream {
    let name = input.ident;
    let Data::Struct(data) = input.data else {
        return quote! {
            ::core::compile_error!("can only derive DeviceSet for structs");
        }
    };
    let fields = data.fields.into_iter().map(|field| {
        let Some(field_name) = field.ident else {
            return quote! {
            ::core::compile_error!("cannot derive DeviceSet for tuple structs");
        }
        };
        let ty = field.ty;
        quote! {
            #field_name: <#ty as ::home_control::zigbee::Device>::new(::core::stringify!(#field_name).to_string(), builder)
        }
    });
    quote! {
        impl ::home_control::zigbee::DeviceSet for #name {
            fn new(builder: &mut ::home_control::zigbee::Manager) -> Self {
                Self {
                    #(#fields),*
                }
            }
        }
    }
}