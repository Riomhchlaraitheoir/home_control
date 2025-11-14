//! An internal crate for procedural macros, any public macro should be re-exported

use macros_impl::Device;
use proc_macro::TokenStream;
use syn::__private::ToTokens;
use syn::{parse_macro_input, DeriveInput, LitInt};

/// an internal macro to define a zigbee device without having to write complicated boilerplate code
#[proc_macro]
pub fn zigbee_device(tokens: TokenStream) -> TokenStream {
    let device = parse_macro_input!(tokens as Device);
    device.into_token_stream().into()
}

/// a public derive macro for deriving home_control::DeviceSet
#[proc_macro_derive(DeviceSet, attributes(device))]
pub fn device_set(tokens: TokenStream) -> TokenStream {
    let input = parse_macro_input!(tokens as DeriveInput);
    match macros_impl::device_set(input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

/// an internal helper macro for implementing AutomationSet for tuples
#[proc_macro]
pub fn automation_sets(tokens: TokenStream) -> TokenStream {
    let input = parse_macro_input!(tokens as LitInt);
    let input = match input.base10_parse() {
        Ok(value) => value,
        Err(err) => return err.to_compile_error().into(),
    };
    macros_impl::automation_sets(input).into()
}
