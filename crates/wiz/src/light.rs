//! Wiz lights

use crate::{udp_request, Error, Response};
use bon::bon;
use control::{Device, ExposesSubManager};
use futures::executor::block_on;
use light_ranged_integers::{RangedU16, RangedU8};
use serde::Deserialize;
use serde_json::json;
use std::net::Ipv4Addr;

/// A Wiz Light
#[derive(Debug, Clone)]
pub struct Light {
    addr: Ipv4Addr,
}

impl Light {
    /// Create a new instance of `Light` and verify that it can be reached
    pub async fn verify_new(addr: Ipv4Addr) -> Result<Self, Error> {
        let response = udp_request(
            addr,
            json! {{
                "method":"registration",
                "params":{
                    "id":"1",
                    "phoneIp":"1.2.3.4",
                    "phoneMac":"AAAAAAAAAAAA",
                    "register":false,
                },
            }},
        )
        .await?;

        let _: Registered = response.result;
        Ok(Self {
            addr,
        })
    }

    /// turn on the light
    pub async fn turn_on(
        &self,
        brightness: RangedU8<0, 100>,
        temp: RangedU16<2700, 6500>,
    ) -> Result<(), Error> {
        let _: Response<Success> = udp_request(
            self.addr,
            json! {{"method":"setPilot","params":{"dimming":brightness,"temp":temp,"state":true}}},
        )
        .await?;
        Ok(())
    }

    /// turn off the light
    pub async fn turn_off(&self) -> Result<(), Error> {
        let _: Response<Success> = udp_request(
            self.addr,
            json! {{"method":"setPilot","params":{"state":false}}},
        )
        .await?;
        Ok(())
    }

    /// retrieve the current state from the light
    pub async fn get_state(&self) -> Result<State, Error> {
        Ok(
            udp_request(self.addr, json! {{"method": "getPilot", "params": {}}})
                .await?
                .result,
        )
    }
}

/// The state of the light
#[derive(Debug, Clone, Deserialize)]
pub struct State {
    /// received signal strength indicator in dBm, numbers are negative, closer to zero is stronger
    pub rssi: i8,
    /// true if the light is on
    pub state: bool,
    /// The colour temperature of the light
    pub temp: RangedU16<2700, 6500>,
    /// The brightness of the light as a percentage
    #[serde(rename = "dimming")]
    pub brightness: RangedU8<0, 100>,
}

/// The response from a light being verified in [Light::verify_new]
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct Registered {
    mac: String,
    success: bool,
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
    type Error = Error;

    fn new(_: &mut Self::Manager, ip: Ipv4Addr) -> Result<Self, Self::Error> {
        block_on(async {
            Self::verify_new(ip).await
        })
    }
}

#[bon]
impl Light {
    #[allow(missing_docs, reason = "This item is hidden since it's only intended for use in macros")]
    #[doc(hidden)]
    #[builder]
    #[allow(unused_variables, reason = "Cannot rename due to compatability issues")]
    pub fn create(manager: &mut impl ExposesSubManager<()>, name: String, ip: Ipv4Addr) -> Result<Self, Error> {
        Self::new(manager.exclusive(), ip)
    }
}
