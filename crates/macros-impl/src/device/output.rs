use super::{Device, Mode, NumericKind, SubPub, Type, Value, Variant};
use proc_macro2::{Ident, TokenStream};
use quote::{ToTokens, quote};
use syn::LitStr;

impl ToTokens for Device {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        tokens.extend(self.to_token_stream())
    }

    fn to_token_stream(&self) -> TokenStream {
        self.clone().into_token_stream()
    }

    fn into_token_stream(self) -> TokenStream
    where
        Self: Sized,
    {
        // let debug = format!("{self:?}");
        // let debug = LitStr::new(&debug, Span::call_site());
        // return quote! {const DEBUG: &str = #debug;};
        let updates = self.clone().updates();

        let mod_name = self.mod_name();
        let Self { docs, url, name, values } = self;
        let update = Ident::new(&format!("{name}Update"), name.span());
        let fields = values.iter().map(|value| value.field(&update));
        let methods = values.iter().map(Value::method);
        let values_set = values.iter().map(|value| value.set(&update, &mod_name));
        let (publish, set_publish, define_publish) = if values.iter().any(Value::requires_publish) {
            (
                Some(quote! { publish: tokio::sync::mpsc::Sender<crate::publish::Publish>, }),
                Some(quote! { publish, }),
                Some(quote! { let publish = manager.outgoing_publishes(); }),
            )
        } else {
            (None, None, None)
        };
        let (updates_field, set_updates, define_updates) = if values.iter().any(Value::requires_subscribe) {
            (
                Some(quote! { updates: crate::Updates<#update>, }),
                Some(quote! { updates, }),
                Some(quote! { let updates = manager.subscribe(name.clone()); }),
            )
        } else {
            (None, None, None)
        };
        quote! {
        #[derive(Clone)]
        #(#[doc = #docs])*
        #[doc = ""]
        #[doc = concat!("See [zigbee2mqtt.io](", #url, ") for more information")]
        pub struct #name {
            name: String,
            #publish
            #updates_field
            #(#fields),*
        }

        #[::bon::bon]
        impl #name {
            #[builder]
            #[allow(missing_docs, reason = "This item is hidden since it's only intended for use in macros")]
            #[doc(hidden)]
            pub async fn create(name: String, manager: &mut crate::Manager) -> Result<Self, anyhow::Error> {
                    <Self as ::control::device::Device>::new(manager, name).await
            }
        }

            impl ::control::device::Device for #name {
                type Args = String;
                type Manager = crate::Manager;

                async fn new(manager: &mut crate::Manager, name: String) -> Result<Self, anyhow::Error> {
                    #define_publish
                    #define_updates
                    Ok(Self {
                        #(#values_set,)*
                        #set_publish
                        #set_updates
                        name,
                    })
                }
            }

        impl #name {
                        #(#methods)*
                    }

        #updates
                }
    }
}

impl Device {
    fn updates(self) -> impl ToTokens {
        let mod_name = self.mod_name();
        let name = self.name;
        let update = Ident::new(&format!("{name}Update"), name.span());
        let enum_fn = self.values.clone().into_iter().map(|value| {
            let name = value.field_name();
            let fn_name = Ident::new(&format!("deserialize_{name}"), name.span());
            let Type::Enum { path, variants } = &value.value_type else {
                return quote! {}
            };
            let ty = &value.value_type;
            let variants = variants.iter().map(|Variant { zigbee, rust }| {
                quote! {
                            #zigbee => Ok(Some(#path::#rust))
                        }
            });
            quote! {
                pub(super) fn #fn_name<'de, D>(deserializer: D) -> Result<Option<#ty>, D::Error> where D: ::serde::Deserializer<'de> {
                    use serde::de::Error;
                    match <String as ::serde::Deserialize>::deserialize(deserializer)?.as_str() {
                        #(#variants,)*
                        unknown => Err(D::Error::custom(format!("unknown value for {}: {}", stringify!(#name), unknown)))
                    }
                }
            }
        });
        let fields = self.values.clone().into_iter().map(|value| {
            let name = value.field_name();
            let attr = if let Type::Enum { .. } = &value.value_type {
                let deserialize_with =
                    LitStr::new(&format!("{mod_name}::deserialize_{name}"), name.span());
                quote! {
                    #[serde(deserialize_with=#deserialize_with)]
                }
            } else {
                quote! {}
            };
            let ty = value.value_type;
            let docs = value.docs;
            quote! {
                #attr
                #(#[doc = #docs])*
                ///
                /// Will be None only if the value was not included in the received update
                pub #name: Option<#ty>
            }
        });
        let getters = self.values.clone().into_iter().map(|value| {
            let name = value.field_name();
            let ty = value.value_type;
            quote! {
                fn #name(self) -> Option<#ty> {
                    self.#name
                }
            }
        });
        let convert_fn = self.values.into_iter().map(|value| {
            match &value.value_type {
                Type::Enum { path, variants } => {
                    let variants = variants.iter().map(|Variant { zigbee, rust }| {
                        quote! {
                            #path::#rust => #zigbee
                        }
                    });
                    let name = value.convert_ident();
                    quote! {
                        pub(super) fn #name(value: #path) -> String {
                            match value {
                                #(#variants,)*
                            }.to_string()
                        }
                    }
                }
                _ => quote! {}
            }
        });
        quote! {
            #[derive(::serde::Deserialize, Clone)]
            #[doc = concat!("An update from a ", stringify!(#name), " device")]
            pub struct #update {
                #(#fields),*
            }

            impl #update {
                #(#getters)*
            }

            impl #name {
                /// Returns a stream of updates as they are received from the device
                pub fn updates(&self) -> impl ::futures::Stream<Item = #update> {
                    self.updates.subscribe()
                }
            }

            #[allow(non_snake_case)]
            mod #mod_name {
                use super::*;

                #(#enum_fn)*

                #(#convert_fn)*
            }
        }
    }

    fn mod_name(&self) -> Ident {
        let name = &self.name;
        Ident::new(&format!("_{name}"), name.span())
    }
}

impl Value {
    fn field(&self, update: &Ident) -> TokenStream {
        let name = self.field_name();
        let concrete_type = self.concrete_type(update);
        quote! {
            #name: #concrete_type
        }
    }

    fn set(&self, update: &Ident, mod_name: &Ident) -> TokenStream {
        let name = self.field_name();
        let create = self.create(update, mod_name);
        quote! {
            #name: #create
        }
    }

    fn method(&self) -> TokenStream {
        let name = self.field_name();
        let trait_type = self.trait_type();
        let docs = &self.docs;
        quote! {
            #(#[doc = #docs])*
            pub fn #name<'a>(&'a self) -> &'a (impl #trait_type + Send + Sync + Clone + use<>) {
                &self.#name
            }
        }
    }

    fn create(&self, update: &Ident, mod_name: &Ident) -> TokenStream {
        let attr = &self.attribute_name;
        let getter = self.field_name();
        let from_device = quote! {
                    #update::#getter
                };
        let (new, to_device) = match &self.value_type {
            Type::Enum { .. } => {
                    let convert = self.convert_ident();
                (
                    quote! { new_mapped },
                    Some(quote! {
                        #mod_name::#convert
                    })
                    )
            }
            Type::Number { .. } | Type::Bool => (
                quote! { new },
                None
                )
        };
        match self.mode.sub_pub() {
            SubPub::SubOnly => {
                quote! { crate::attribute::SubscribeAttr::new(updates.clone(), #from_device) }
            }
            SubPub::Both => {
                quote! { crate::attribute::SubscribePublishAttr::#new(updates.clone(), publish.clone(), name.clone(), #attr, #from_device, #to_device) }
            }
            SubPub::PubOnly => {
                quote! { crate::attribute::PublishAttr::#new(publish.clone(), name.clone(), #attr, #to_device) }
            }
        }
    }

    fn concrete_type(&self, update: &Ident) -> TokenStream {
        let item = self.value_type.to_token_stream();
        let zigbee = self.zigbee_type();
        match self.mode.sub_pub() {
            SubPub::SubOnly => {
                quote! { crate::attribute::SubscribeAttr<#update, #item> }
            }
            SubPub::Both => {
                quote! { crate::attribute::SubscribePublishAttr<#item, #update, #zigbee> }
            }
            SubPub::PubOnly => {
                quote! { crate::attribute::PublishAttr<#item, #zigbee> }
            }
        }
    }

    fn zigbee_type(&self) -> TokenStream {
        match &self.value_type {
            Type::Enum { .. } => {
                quote! {
                    String
                }
            }
            Type::Number { .. } | Type::Bool => self.value_type.to_token_stream(),
        }
    }

    fn trait_type(&self) -> TokenStream {
        let value = &self.value_type;
        match self.mode {
            Mode::Stream => quote! {
                ::control::Sensor<Item = #value> + Sync
            },
            Mode::StreamGet => quote! {
                ::control::Sensor<Item = #value> + ::control::ReadValue<Item = #value> + Sync
            },
            Mode::Set => quote! {
                ::control::WriteValue<Item = #value> + Sync
            },
            Mode::StreamGetSet => quote! {
                ::control::Sensor<Item = #value> + ::control::ReadValue<Item = #value> + ::control::WriteValue<Item = #value> + Sync
            },
            Mode::SetToggle => quote! {
                ::control::ToggleValue<Item = #value> + Sync
            },
            Mode::StreamGetSetToggle => quote! {
                ::control::Sensor<Item = #value> + ::control::ReadValue<Item = #value> + ::control::ToggleValue<Item = #value> + Sync
            },
            Mode::StreamSet => quote! {
                ::control::Sensor<Item = #value> + ::control::WriteValue<Item = #value> + Sync
            },
        }
    }

    fn convert_ident(&self) -> Ident {
        let name = self.field_name();
        Ident::new(&format!("convert_{name}"), name.span())
    }
}

impl Value {
    fn field_name(&self) -> Ident {
        self.value_name
            .clone()
            .unwrap_or_else(|| Ident::new(&self.attribute_name.value(), self.attribute_name.span()))
    }
}

impl ToTokens for Type {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        tokens.extend(self.to_token_stream())
    }
    fn to_token_stream(&self) -> TokenStream {
        self.clone().into_token_stream()
    }
    fn into_token_stream(self) -> TokenStream
    where
        Self: Sized,
    {
        match self {
            Type::Enum { path, .. } => {
                quote! { #path }
            }
            Type::Number { kind, range } => {
                if let Some((min, max)) = range {
                    let ty = match kind {
                        NumericKind::U8 => quote! {
                            ::light_ranged_integers::RangedU8
                        },
                        NumericKind::U16 => quote! {
                            ::light_ranged_integers::RangedU16
                        },
                        NumericKind::U32 => quote! {
                            ::light_ranged_integers::RangedU32
                        },
                        NumericKind::U64 => quote! {
                            ::light_ranged_integers::RangedU64
                        },
                        NumericKind::U128 => quote! {
                            ::light_ranged_integers::RangedU128
                        },
                        NumericKind::I8 => quote! {
                            ::light_ranged_integers::RangedI8
                        },
                        NumericKind::I16 => quote! {
                            ::light_ranged_integers::RangedI16
                        },
                        NumericKind::I32 => quote! {
                            ::light_ranged_integers::RangedI32
                        },
                        NumericKind::I64 => quote! {
                            ::light_ranged_integers::RangedI64
                        },
                        NumericKind::I128 => quote! {
                            ::light_ranged_integers::RangedI128
                        },
                    };
                    quote! {
                        #ty<#min, #max>
                    }
                } else {
                    match kind {
                        NumericKind::U8 => quote! {u8},
                        NumericKind::U16 => quote! {u16},
                        NumericKind::U32 => quote! {u32},
                        NumericKind::U64 => quote! {u64},
                        NumericKind::U128 => quote! {u128},
                        NumericKind::I8 => quote! {i8},
                        NumericKind::I16 => quote! {i16},
                        NumericKind::I32 => quote! {i32},
                        NumericKind::I64 => quote! {i64},
                        NumericKind::I128 => quote! {i128},
                    }
                }
            }
            Type::Bool => {
                quote! {bool}
            }
        }
    }
}

impl Value {
    fn requires_publish(&self) -> bool {
        self.mode.sub_pub() != SubPub::SubOnly
    }

    fn requires_subscribe(&self) -> bool {
        self.mode.sub_pub() != SubPub::PubOnly
    }
}
