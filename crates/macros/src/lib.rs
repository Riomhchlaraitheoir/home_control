use macros_impl::{device_set, Device};
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
pub fn device_set_macro(tokens: TokenStream) -> TokenStream {
    let input = parse_macro_input!(tokens as DeriveInput);
    device_set(input).into()
}

/// an internal helper macro for implementing AutomationSet for tuples
#[proc_macro]
pub fn automation_sets(tokens: TokenStream) -> TokenStream {
    let input = parse_macro_input!(tokens as LitInt);
    macros_impl::automation_sets(input.base10_parse().expect("failed to parse literal")).into()
}
