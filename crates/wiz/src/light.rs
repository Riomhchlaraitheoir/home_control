//! Wiz lights

use crate::{udp_request, Error, Response};
use anyhow::Context;
use bon::bon;
use control::device::{Device};
use control::reflect;
use control::reflect::value::{Value, ValueType};
use control::reflect::{DeviceInfo, Field, Operation, Operations, SetError};
use futures::future::BoxFuture;
use futures::stream::BoxStream;
use futures::FutureExt;
use light_ranged_integers::{RangedU16, RangedU8};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer};
use serde_json::json;
use std::net::Ipv4Addr;
use tokio::sync::Mutex;

/// A Wiz Light
#[derive(Debug)]
pub struct Light
where
    Self: Sync,
{
    info: DeviceInfo,
    addr: Ipv4Addr,
    state: Mutex<State>,
}

impl Light {
    /// Create a new instance of `Light` and verify that it can be reached
    pub async fn verify_new(info: DeviceInfo, addr: Ipv4Addr) -> Result<Self, anyhow::Error> {
        let state = udp_request(addr, json! {{"method": "getPilot", "params": {}}})
            .await?
            .result;
        let state = Mutex::new(state);
        Ok(Self {
            info,
            addr,
            state,
        })
    }

    /// update the tracked state and request to light to change state to match
    pub async fn update_state(&self, f: impl FnOnce(&mut State)) -> Result<(), Error> {
        let mut state = { *self.state.lock().await };
        f(&mut state);
        let msg = if state.state {
            json! {{"method":"setPilot","params":{"dimming":state.brightness,"temp":state.temp,"state":true}}}
        } else {
            json! {{"method":"setPilot","params":{"state":false}}}
        };
        let _: Response<Success> = udp_request(self.addr, msg).await?;
        *self.state.lock().await = state;
        Ok(())
    }

    /// Toggle the light based on the current known state of the light, this state may be outdated,
    /// see [last_state](Self::last_state) for more info.
    ///
    /// To update the state to an accurate state before toggling, call [get_state](Self::get_state) first
    pub async fn toggle(&self) -> Result<(), Error> {
        self.update_state(|state| {
            state.state = !state.state;
        })
        .await
    }

    /// turn on the light
    pub async fn turn_on(&self) -> Result<(), Error> {
        self.update_state(|state| {
            state.state = true;
        })
        .await
    }

    /// turn off the light
    pub async fn turn_off(&self) -> Result<(), Error> {
        self.update_state(|state| {
            state.state = false;
        })
        .await
    }

    /// Returns the last observed state of the light, this is not guaranteed to be accurate since
    /// the light can change state without notice if a command is sent from another source
    pub async fn last_state(&self) -> State {
        *self.state.lock().await
    }

    /// retrieve the current state from the light
    pub async fn get_state(&self) -> Result<State, Error> {
        let state = udp_request(self.addr, json! {{"method": "getPilot", "params": {}}})
            .await?
            .result;
        *self.state.lock().await = state;
        Ok(state)
    }
}

/// The state of the light
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct State {
    /// received signal strength indicator in dBm, numbers are negative, closer to zero is stronger
    pub rssi: i8,
    /// true if the light is on
    pub state: bool,
    /// The colour temperature of the light
    #[serde(deserialize_with = "deserialize_temp")]
    pub temp: Option<RangedU16<1000, 12000>>,
    /// The brightness of the light as a percentage
    #[serde(rename = "dimming")]
    pub brightness: RangedU8<0, 100>,
}

fn deserialize_temp<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<RangedU16<1000, 12000>>, D::Error> {
    let value: u16 = <u16>::deserialize(deserializer)?;
    if value == 0 {
        return Ok(None);
    }
    if let Some(r) = RangedU16::<1000, 12000>::new_try(value) {
        Ok(Some(r))
    } else {
        Err(D::Error::invalid_value(
            serde::de::Unexpected::Other("int"),
            &format!(
                "Value {} is not in the desired range [{},{}]",
                value, 1000, 12000
            )
            .as_ref(),
        ))
    }
}

/// A simple response indicating success or failure
// This has more field, but there is no current use for them
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct Success {
    success: bool,
}

impl Device for Light {
    type Args = Ipv4Addr;
    type Manager = ();

    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    async fn new_with_args(_: &mut Self::Manager, info: DeviceInfo, ip: Ipv4Addr) -> Result<Self, anyhow::Error> {
        Self::verify_new(info, ip).await
    }
}

#[bon]
impl Light {
    #[allow(
        missing_docs,
        reason = "This item is hidden since it's only intended for use in macros"
    )]
    #[doc(hidden)]
    #[builder]
    #[allow(unused_variables, reason = "Cannot rename due to compatability issues")]
    pub async fn create(
        manager: &mut (),
        info: DeviceInfo,
        ip: Ipv4Addr,
    ) -> Result<Self, anyhow::Error> {
        Self::new_with_args(manager, info, ip).await
    }
}

impl reflect::Device for Light {
    fn info(&self) -> DeviceInfo {
        self.info.clone()
    }

    fn fields(&self) -> Vec<Field> {
        vec![
            Field {
                name: "state".to_owned(),
                description: "is true if the light is on".to_string(),
                value_type: ValueType::from_type::<bool>(),
                operations: Operations {
                    subscribe: false,
                    get: true,
                    set: true,
                    toggle: true,
                }
            },
            Field {
                name: "temp".to_owned(),
                description: "The light's colour temperature".to_string(),
                value_type: ValueType::from_type::<Option<RangedU16<1000, 12000>>>(),
                operations: Operations {
                    subscribe: false,
                    get: true,
                    set: true,
                    toggle: true,
                }
            },
            Field {
                name: "brightness".to_owned(),
                description: "The light's brightness".to_string(),
                value_type: ValueType::from_type::<RangedU8<0, 100>>(),
                operations: Operations {
                    subscribe: false,
                    get: true,
                    set: true,
                    toggle: true,
                }
            },
        ]
    }

    fn subscribe(&self, field: &str) -> Result<BoxFuture<'_, BoxStream<'_, Value>>, reflect::Error> {
        Err(match field {
            "state" | "temp" | "brightness" => reflect::Error::OperationNotSupported {
                device: self.info.name.to_string(),
                field: field.to_string(),
                operation: Operation::Subscribe,
            },
            unknown => reflect::Error::FieldNotFound {
                device: self.info.name.to_string(),
                field: unknown.to_string(),
            },
        })
    }

    fn get(&self, field: &str) -> Result<BoxFuture<'_, anyhow::Result<Value>>, reflect::Error> {
        match field {
            "state" => Ok(Box::pin(
                self.get_state()
                    .map(|result| {
                        result.map(|state| state.state.into())
                            .context("failed to fetch state from light")
                    }),
            )),
            "temp" => Ok(Box::pin(
                self.get_state()
                    .map(|result| {
                        result.map(|state| state.temp.into())
                            .context("failed to fetch state from light")
                    }),
            )),
            "brightness" => Ok(Box::pin(
                self.get_state()
                    .map(|result| {
                        result.map(|state| state.brightness.into())
                            .context("failed to fetch state from light")
                    }),
            )),
            unknown => Err(reflect::Error::FieldNotFound {
                device: self.info.name.to_string(),
                field: unknown.to_string(),
            }),
        }
    }

    fn set(
        &self,
        field: &str,
        value: Value,
    ) -> Result<BoxFuture<'_, anyhow::Result<()>>, SetError> {
        match field {
            "state" => {
                let value = value.try_into()?;
                Ok(Box::pin(
                    self.update_state(move |state| state.state = value)
                        .map(|result| result.context("failed to update state")),
                ))
            },
            "temp" => {
                let value = value.try_into()?;
                Ok(Box::pin(
                    self.update_state(move |state| state.temp = value)
                        .map(|result| result.context("failed to update temp")),
                ))
            },
            "brightness" => {
                let value = value.try_into()?;
                Ok(Box::pin(
                    self.update_state(move |state| state.brightness = value)
                        .map(|result| result.context("failed to update brightness")),
                ))
            },
            unknown => Err(reflect::Error::FieldNotFound {
                device: self.info.name.to_string(),
                field: unknown.to_string(),
            }.into()),
        }
    }

    fn toggle(&self, field: &str) -> Result<BoxFuture<'_, anyhow::Result<()>>, reflect::Error> {
        match field {
            "state" => {
                Ok(Box::pin(
                    self.update_state(move |state| state.state = !state.state)
                        .map(|result| result.context("failed to update state")),
                ))
            },
            "temp" | "brightness" => {
                Err(reflect::Error::OperationNotSupported {
                    device: self.info.name.to_string(),
                    field: field.to_string(),
                    operation: Operation::Toggle,
                })
            },
            unknown => Err(reflect::Error::FieldNotFound {
                device: self.info.name.to_string(),
                field: unknown.to_string(),
            }),
        }
    }
}
