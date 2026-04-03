//! A mock server for testing purposes

use control::reflect::{
        value::{Value, ValueType},
        Operation,
        Operations,
        DeviceInfo,
        Error,
        Field,
        SetError
};
use futures::future::{BoxFuture};
use futures::stream::BoxStream;
use std::collections::HashMap;
use anyhow::Context;
use tokio::sync::watch::{channel, Sender};
use tokio::sync::RwLock;
use tokio_stream::wrappers::WatchStream;
use tower_http::cors::CorsLayer;

#[allow(clippy::expect_used)]
#[tokio::main]
async fn main() {
    simple_logger::init_with_level(log::Level::Debug).expect("Unable to setup logging");
    let app = web_ui::api()
        .add_device(MockDevice {
            info: DeviceInfo {
                id: "office_light".to_string(),
                name: "Office light".to_string(),
                description: None,
                tags: [
                    ("room", "Office"),
                    ("type", "light")
                ].into_iter()
                    .map(|(key, value)| (key.to_string(), value.to_string()))
                    .collect(),
            },
            fields: [
                ("switch".to_string(), (
                    Field {
                        name: "Switch".to_string(),
                        description: "Control whether the light is on or not".to_string(),
                        operations: Operations {
                            subscribe: true,
                            get: true,
                            set: true,
                            toggle: true,
                        },
                        value_type: ValueType::Bool,
                    },
                    RwLock::new(MockField::new(Value::Bool(false)))
                )),
                ("brightness".to_string(), (
                    Field {
                        name: "Brightness".to_string(),
                        description: "The light's brightness".to_string(),
                        operations: Operations {
                            subscribe: true,
                            get: true,
                            set: true,
                            toggle: false,
                        },
                        value_type: ValueType::Int((0..=100).into()),
                    },
                    RwLock::new(MockField::new(Value::Bool(false)))
                ))
            ].into_iter().collect(),
        })
        .build();
    let cors_layer = CorsLayer::permissive();
    let app = app.layer(cors_layer);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8000").await.expect("Unable to bind TCP listener");
    axum::serve(listener, app).await.expect("server error");
}

struct MockDevice {
    info: DeviceInfo,
    fields: HashMap<String, (Field, RwLock<MockField>)>,
}

impl control::reflect::Device for MockDevice {
    fn info(&self) -> DeviceInfo {
        self.info.clone()
    }

    fn fields(&self) -> Vec<Field> {
        self.fields.values().map(|(x, _)| x.clone()).collect()
    }

    fn subscribe(&self, field_name: &str) -> Result<BoxStream<'_, Value>, Error> {
        let Some((info, field)) = self.fields.get(field_name) else {
            return Err(Error::FieldNotFound {
                device: self.info.name.to_string(),
                field: field_name.to_string()
            })
        };
        if !info.operations.subscribe {
            return Err(Error::OperationNotSupported {
                operation: Operation::Subscribe,
                device: self.info.name.to_string(),
                field: field_name.to_string()
            })
        }
        Ok(Box::pin(WatchStream::new(field.blocking_read().sender.subscribe())))
    }

    fn get(&self, field_name: &str) -> Result<BoxFuture<'_, anyhow::Result<Value>>, Error> {
        let Some((info, field)) = self.fields.get(field_name) else {
            return Err(Error::FieldNotFound {
                device: self.info.name.to_string(),
                field: field_name.to_string()
            })
        };
        if !info.operations.get  {
            return Err(Error::OperationNotSupported {
                operation: Operation::Get,
                device: self.info.name.to_string(),
                field: field_name.to_string()
            })
        }
        Ok(Box::pin(async {
            Ok(field.read().await.value.clone())
        }))
    }

    fn set(&self, field_name: &str, value: Value) -> Result<BoxFuture<'_, anyhow::Result<()>>, SetError> {
        let Some((info, field)) = self.fields.get(field_name) else {
            return Err(SetError::Error(Error::FieldNotFound {
                device: self.info.name.to_string(),
                field: field_name.to_string()
            }))
        };
        if !info.operations.set {
            return Err(SetError::Error(Error::OperationNotSupported {
                operation: Operation::Set,
                device: self.info.name.to_string(),
                field: field_name.to_string()
            }))
        }
        info.value_type.validate(&value)?;
        Ok(Box::pin(async {
            let mut field = field.write().await;
            if field.value == value {
                return Ok(());
            }
            field.value = value.clone();
            field.sender.send(value).context("Failed to send value to channel")?;
            Ok(())
        }))
    }

    fn toggle(&self, field_name: &str) -> Result<BoxFuture<'_, anyhow::Result<()>>, Error> {
        let Some((info, field)) = self.fields.get(field_name) else {
            return Err(Error::FieldNotFound {
                device: self.info.name.to_string(),
                field: field_name.to_string()
            })
        };
        if !info.operations.toggle {
            return Err(Error::OperationNotSupported {
                operation: Operation::Toggle,
                device: self.info.name.to_string(),
                field: field_name.to_string()
            })
        }
        #[allow(clippy::panic, reason = "This will do for a test")]
        Ok(Box::pin(async {
            let mut field = field.write().await;
            if let Value::Bool(value) = &mut field.value {
                *value = !*value;
            } else {
                panic!("Expected boolean value")
            }
            field.sender.send(field.value.clone()).context("Failed to send value to channel")?;
            Ok(())
        }))
    }
}

struct MockField {
    value: Value,
    sender: Sender<Value>
}

impl MockField {
    fn new(value: Value) -> Self {
        let (sender, _) = channel(value.clone());
        Self {
            value, sender
        }
    }
}
