use super::{Device, Mode, NumericKind, SubPub, Type, Value, Variant};
use proc_macro2::{Group, Ident, TokenStream, TokenTree};
use quote::{quote, ToTokens};
use std::collections::HashMap;
use syn::{parse_str, LitStr, Path};

macro_rules! imports {
    ($(use $path:path$(as $alias:ident)?;)*) => {
        const IMPORTS: &[(&str, Option<&str>)] = &[
            $(
            {
                #[allow(unused_variables, reason = "only way to make the macro work, only impacts compile time anyway")]
                let alias = Option::<&str>::None;
                $(let alias = Some(stringify!($alias));)?
                (stringify!(::$path), alias)
            }
            ),*
        ];
    };
}

imports! {
    use bon::bon;
    use serde::Deserialize;
    use serde::Deserializer;
    use control::device::Device;
    use control::Sensor;
    use control::ReadValue;
    use control::ToggleValue;
    use control::WriteValue;
    use control::reflect::Device as ReflectDevice;
    use control::reflect::DeviceInfo;
    use control::reflect::Field;
    use control::reflect::Error;
    use control::reflect::SetError;
    use control::reflect::Operation;
    use control::reflect::value::Value;
    use control::reflect::value::ValueType;
    use futures::stream::Stream;
    use futures::stream::StreamExt;
    use futures::stream::BoxStream;
    use futures::future::BoxFuture;
    use futures::future::FutureExt;
}

impl ToTokens for Device {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        tokens.extend(self.to_token_stream())
    }

    fn to_token_stream(&self) -> TokenStream {
        self.clone().into_token_stream()
    }

    fn into_token_stream(self) -> TokenStream {
        let imports = IMPORTS.iter().map(|(path, alias)| {
            #[allow(clippy::expect_used, reason = "If this panics it would certainly be hit during development")]
            let path: Path = parse_str(path).expect("failed to parse import path");
            let ident = if let Some(alias) = alias {
                alias.to_string()
            } else {
                #[allow(clippy::expect_used, reason = "If this panics it would certainly be hit during development")]
                path.segments.last().expect("import path has no segments").ident.to_string()
            };
            (ident, path.into_token_stream())
        }).collect();
        import_idents(self.build_token_stream(), &imports)
    }
}

fn import_idents(tokens: TokenStream, imports: &HashMap<String, TokenStream>) -> TokenStream {
    Importer {
        imports,
        tokens: tokens.into_iter(),
        state: ImporterState::Normal {
            sep_count: 0
        },
    }.collect()
}

struct Importer<'a> {
    imports: &'a HashMap<String, TokenStream>,
    tokens: <TokenStream as IntoIterator>::IntoIter,
    state: ImporterState
}

enum ImporterState {
    Normal {
        sep_count: u8,
    },
    Path(<TokenStream as IntoIterator>::IntoIter)
}

impl Iterator for Importer<'_> {
    type Item = TokenTree;

    fn next(&mut self) -> Option<Self::Item> {
        let sep_count = match &mut self.state {
            ImporterState::Normal { sep_count } => {
                let count = *sep_count;
                *sep_count = 0;
                count
            },
            ImporterState::Path(iterator) => {
                if let Some(next) = iterator.next() {
                    return Some(next);
                } else {
                    self.state = ImporterState::Normal {
                        sep_count: 0
                    };
                    0
                }
            }
        };
        let tree = self.tokens.next()?;
        let tree = match tree {
            TokenTree::Group(group) => {
                TokenTree::Group(Group::new(group.delimiter(), import_idents(group.stream(), self.imports)))
            }
            TokenTree::Ident(ident) => {
                if sep_count == 2 {
                    // just after a separator, do not import
                    return Some(TokenTree::Ident(ident))
                }
                let s = ident.to_string();
                let Some(path) = self.imports.get(&s) else {
                    return Some(TokenTree::Ident(ident))
                };
                let mut tokens = path.into_token_stream().into_iter();
                #[allow(clippy::expect_used, reason = "If this panics it would certainly be hit during development")]
                let first = tokens.next().expect("import parsed to a empty steam");
                self.state = ImporterState::Path(tokens);
                first
            }
            TokenTree::Punct(punct) => {
                if punct.as_char() == ':' {
                    self.state = ImporterState::Normal {
                        sep_count: sep_count+1
                    };
                }
                TokenTree::Punct(punct)
            }
            other => other
        };
        Some(tree)
    }
}

macro_rules! unsupported_op {
    ($operation:ident) => {
        quote! {
            Err(Error::OperationNotSupported {
                device: self.info.name.to_owned(),
                field: field.to_owned(),
                operation: Operation::$operation
            }.into())
        }
    };
}

impl Device {
    fn build_token_stream(self) -> TokenStream
    where
        Self: Sized,
    {
        // let debug = format!("{self:?}");
        // let debug = LitStr::new(&debug, Span::call_site());
        // return quote! {const DEBUG: &str = #debug;};
        let updates = self.clone().updates();
        let reflect = self.reflect();

        let mod_name = self.mod_name();
        let Self {
            docs,
            url,
            name,
            values,
        } = self;
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
        let (updates_field, set_updates, define_updates) =
            if values.iter().any(Value::requires_subscribe) {
                (
                    Some(quote! { updates: crate::Updates<#update>, }),
                    Some(quote! { updates, }),
                    Some(quote! { let updates = manager.subscribe(info.name.clone()); }),
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
            info: DeviceInfo,
            #publish
            #updates_field
            #(#fields),*
        }

        #[bon]
        impl #name {
            #[builder]
            #[allow(missing_docs, reason = "This item is hidden since it's only intended for use in macros")]
            #[doc(hidden)]
            pub async fn create(manager: &mut crate::Manager, info: DeviceInfo) -> Result<Self, anyhow::Error> {
                    <Self as Device>::new(manager, info).await
            }
        }

            impl Device for #name {
                type Args = ();
                type Manager = crate::Manager;

                fn info(&self) -> &DeviceInfo {
                    &self.info
                }

                async fn new_with_args(manager: &mut crate::Manager, info: ::control::reflect::DeviceInfo, _: ()) -> Result<Self, anyhow::Error> {
                    #define_publish
                    #define_updates
                    Ok(Self {
                        #(#values_set,)*
                        #set_publish
                        #set_updates
                        info
                    })
                }
            }

        impl #name {
                        #(#methods)*
                    }

        #updates

        #reflect
                }
    }

    fn updates(self) -> impl ToTokens {
        let mod_name = self.mod_name();
        let name = self.name;
        let update = Ident::new(&format!("{name}Update"), name.span());
        let enum_fn = self.values.clone().into_iter().map(|value| {
            let name = value.field_name();
            let fn_name = Ident::new(&format!("deserialize_{name}"), name.span());
            match &value.value_type {
                Type::Enum { path, variants } => {
                    let ty = &value.value_type;
                    let variants = variants.iter().map(|Variant { zigbee, rust }| {
                        quote! {
                            Some(#zigbee) => Some(#path::#rust)
                        }
                    });
                    quote! {
                pub(super) fn #fn_name<'de, D>(deserializer: D) -> Result<Option<#ty>, D::Error> where D: Deserializer<'de> {
                    use serde::de::Error;
                    Ok(match <Option<String> as Deserialize>::deserialize(deserializer)?.as_deref() {
                        #(#variants,)*
                        Some(unknown) => return Err(D::Error::custom(format!("unknown value for {}: {}", stringify!(#name), unknown))),
                        None => None
                    })
                }
            }
                }
                Type::Bool(Some([false_value, true_value])) => {
                    let ty = &value.value_type;
                    quote! {
                pub(super) fn #fn_name<'de, D>(deserializer: D) -> Result<Option<#ty>, D::Error> where D: Deserializer<'de> {
                    use serde::de::Error;
                    Ok(match <Option<String> as Deserialize>::deserialize(deserializer)?.as_deref() {
                        Some(#false_value) => Some(false),
                        Some(#true_value) => Some(true),
                        Some(unknown) => return Err(D::Error::custom(format!("unknown value for {}: {}", stringify!(#name), unknown))),
                        None => None
                    })
                }
            }
                }
                Type::Number { .. } | Type::Bool(None) => quote! {}
            }
        });
        let fields = self.values.clone().into_iter().map(|value| {
            let name = value.field_name();
            let attr = if let Type::Enum { .. } | Type::Bool(Some(_)) = &value.value_type {
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
                ///Will be None only if the value was not included in the received update
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
        let convert_fn = self
            .values
            .into_iter()
            .map(|value| match &value.value_type {
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
                Type::Bool(Some([false_str, true_str])) => {
                    let name = value.convert_ident();
                    quote! {
                        pub(super) fn #name(value: bool) -> String {
                            if value {
                                #true_str
                            } else {
                                #false_str
                            }.to_string()
                        }
                    }
                }
                Type::Number { .. } | Type::Bool(None) => quote! {},
            });
        quote! {
            #[derive(Deserialize, Clone)]
            #[doc = concat!("An update from a ", stringify!(#name), " device")]
            pub struct #update {
                #(#fields),*
            }

            impl #update {
                #(#getters)*
            }

            impl #name {
                /// Returns a stream of updates as they are received from the device
                pub fn updates(&self) -> impl Stream<Item = #update> {
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

    pub(crate) fn reflect(&self) -> TokenStream {
        let info = {
            let fields = self.values.iter().map(|value| {
                let name = value.field_name().to_string();
                let desc: String = value.docs.iter().map(|desc| if desc.value().is_empty() {
                    "\n".to_string()
                } else {
                    desc.value()
                }).collect();
                let allow_subscribe = value.mode.allow_subscribe();
                let allow_get = value.mode.allow_get();
                let allow_set = value.mode.allow_set();
                let allow_toggle = value.mode.allow_toggle();
                let value_type = &value.value_type;
                quote! {
                    Field {
                        name: #name.to_string(),
                        description: #desc.to_string(),
                        operations: ::control::reflect::Operations {
                            subscribe: #allow_subscribe,
                            get: #allow_get,
                            set: #allow_set,
                            toggle: #allow_toggle,
                        },
                        value_type: ValueType::from_type::<#value_type>(),
                    }
                }
            });
            quote! {
                        fn info(&self) -> DeviceInfo {
                self.info.clone()
            }
                fn fields(&self) -> Vec<Field> {
                    vec![
                        #(#fields,)*
                    ]
                }
                    }
        };
        let subscribe = {
            let fields = self.values.iter().map(|value| {
                let name = value.field_name();
                let block = if value.mode.allow_subscribe() {
                    quote! { Ok(Box::pin(self.#name.subscribe().map(Value::from))) }
                } else {
                    unsupported_op!(Subscribe)
                };

                let name = name.to_string();
                quote! { #name => #block }
            });
            quote! {
            fn subscribe(&self, field: &str) -> Result<BoxStream<'_, Value>, Error> {
                    use Sensor;
                    use StreamExt;
                match field {
                    #(#fields,)*
                    _ => Err(Error::FieldNotFound {
                        device: self.info.name.to_owned(),
                        field: field.to_owned(),
                    })
                }
            }
                    }
        };
        let get = {
            let fields = self.values.iter().map(|value| {
                let name = value.field_name();
                let block = if value.mode.allow_get() {
                    quote! { Ok(Box::pin(self.#name.get().map(|result| result.map(Value::from)))) }
                } else {
                    unsupported_op!(Get)
                };
                let name = name.to_string();
                quote! { #name => #block }
            });
            quote! {
            fn get(&self, field: &str) -> Result<BoxFuture<'_, anyhow::Result<Value>>, Error> {
                use ReadValue;
                use FutureExt;
                match field {
                    #(#fields,)*
                    _ => Err(Error::FieldNotFound {
                        device: self.info.name.to_owned(),
                        field: field.to_owned(),
                    })
                }
            }
                    }
        };
        let set = {
            let fields = self.values.iter().map(|value| {
                let name = value.field_name();
                let block = if value.mode.allow_set() {
                    quote! {
                        {
                            let value = value.try_into()?;
                            Ok(Box::pin(self.#name.set(value)))
                        }
                    }
                } else {
                    unsupported_op!(Set)
                };
                let name = name.to_string();
                quote! { #name => #block }
            });
            quote! {
            fn set(&self, field: &str, value: Value) -> Result<BoxFuture<'_, anyhow::Result<()>>, SetError> {
                use WriteValue;
                match field {
                    #(#fields,)*
                    _ => Err(Error::FieldNotFound {
                        device: self.info.name.to_owned(),
                        field: field.to_owned(),
                    }.into())
                }
            }
                    }
        };
        let toggle = {
            let fields = self.values.iter().map(|value| {
                let name = value.field_name();
                let block = if value.mode.allow_toggle() {
                    quote! { Ok(self.#name.toggle()) }
                } else {
                    unsupported_op!(Toggle)
                };
                let name = name.to_string();
                quote! { #name => #block }
            });
            quote! {
            fn toggle(&self, field: &str) -> Result<futures::future::BoxFuture<'_, anyhow::Result<()>>, Error> {
                use ToggleValue;
                match field {
                    #(#fields,)*
                    _ => Err(Error::FieldNotFound {
                        device: self.info.name.to_owned(),
                        field: field.to_owned(),
                    })
                }
            }
                    }
        };
        let name = &self.name;
        quote! {
        impl ReflectDevice for #name {
                        #info
                        #subscribe
                        #get
                        #set
                        #toggle
        }
                }
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
            Type::Enum { .. } | Type::Bool(Some(_)) => {
                let convert = self.convert_ident();
                (
                    quote! { new_mapped },
                    Some(quote! {
                        #mod_name::#convert
                    }),
                )
            }
            Type::Number { .. } | Type::Bool(None) => (quote! { new }, None),
        };
        match self.mode.sub_pub() {
            SubPub::SubOnly => {
                quote! { crate::attribute::SubscribeAttr::new(updates.clone(), #from_device) }
            }
            SubPub::Both => {
                quote! { crate::attribute::SubscribePublishAttr::#new(updates.clone(), publish.clone(), info.name.clone(), #attr, #from_device, #to_device) }
            }
            SubPub::PubOnly => {
                quote! { crate::attribute::PublishAttr::#new(publish.clone(), info.name.clone(), #attr, #to_device) }
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
            Type::Enum { .. } | Type::Bool(Some(_)) => {
                quote! {
                    String
                }
            }
            Type::Number { .. } | Type::Bool(None) => self.value_type.to_token_stream(),
        }
    }

    fn trait_type(&self) -> TokenStream {
        let value = &self.value_type;
        match self.mode {
            Mode::Stream => quote! {
                Sensor<Item = #value> + Sync
            },
            Mode::StreamGet => quote! {
                Sensor<Item = #value> + ReadValue<Item = #value> + Sync
            },
            Mode::Set => quote! {
                WriteValue<Item = #value> + Sync
            },
            Mode::StreamGetSet => quote! {
                Sensor<Item = #value> + ReadValue<Item = #value> + WriteValue<Item = #value> + Sync
            },
            Mode::SetToggle => quote! {
                ToggleValue<Item = #value> + Sync
            },
            Mode::StreamGetSetToggle => quote! {
                Sensor<Item = #value> + ReadValue<Item = #value> + ToggleValue<Item = #value> + Sync
            },
            Mode::StreamSet => quote! {
                Sensor<Item = #value> + WriteValue<Item = #value> + Sync
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
            Type::Bool(_) => {
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
