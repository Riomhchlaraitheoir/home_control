#![doc = include_str!("../README.md")]
#![allow(missing_docs, reason = "This is an internal crate that does not need to be extensively documented")]

use proc_macro2::TokenStream;
use quote::quote;
pub use crate::device::Device;

mod device;
mod device_set;
mod automation_set;

pub fn device(input: Device) -> TokenStream {
    quote! { #input }
}

pub use device_set::device_set;
pub use automation_set::automation_sets;