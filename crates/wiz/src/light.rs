use crate::{udp_request, Error, Response};
use bon::bon;
use control::{Device, ExposesSubManager};
use futures::executor::block_on;
use light_ranged_integers::{RangedU16, RangedU8};
use serde::Deserialize;
use serde_json::json;
use std::net::Ipv4Addr;

#[derive(Debug, Clone)]
pub struct Light {
    addr: Ipv4Addr,
}

impl Light {
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

    pub async fn turn_off(&self) -> Result<(), Error> {
        let _: Response<Success> = udp_request(
            self.addr,
            json! {{"method":"setPilot","params":{"state":false}}},
        )
        .await?;
        Ok(())
    }

    pub async fn get_state(&self) -> Result<State, Error> {
        Ok(
            udp_request(self.addr, json! {{"method": "getPilot", "params": {}}})
                .await?
                .result,
        )
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct State {
    /// received signal strength indicator in dBm, numbers are negative, closer to zero is stronger
    pub rssi: i8,
    pub state: bool,
    pub temp: RangedU16<2700, 6500>,
    #[serde(rename = "dimming")]
    pub brightness: RangedU8<0, 100>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct Registered {
    mac: String,
    success: bool,
}

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
    #[builder]
    #[allow(unused_variables, reason = "Cannot rename due to compatability issues")]
    pub fn create(manager: &mut impl ExposesSubManager<()>, name: String, ip: Ipv4Addr) -> Result<Self, Error> {
        Self::new(manager.exclusive(), ip)
    }
}
