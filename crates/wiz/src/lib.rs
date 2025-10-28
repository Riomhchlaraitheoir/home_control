#![cfg(target_os = "linux")]

pub mod light;

use futures::future::FusedFuture;
use futures::FutureExt;
use riz::models::Light as WizLight;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::net::{Ipv4Addr, UdpSocket};

pub struct Light {
    wiz: WizLight,
}

impl Light {
    pub fn new(addr: Ipv4Addr, name: Option<&str>) -> Self {
        Self {
            wiz: WizLight::new(addr, name),
        }
    }

    // fn switch(&self) {
    //     self.wiz.set()
    // }
}

struct LightSwitch<'a> {
    wiz: &'a WizLight,
}
/*
impl ReadValue for LightSwitch<'_> {
    type Item = SwitchState;

    fn get(&self) -> Box<dyn Future<Output=Result<Self::Item, Error>> + Unpin + Send + '_> {
        let status = self.wiz.get_status().map_err(|error| {
            Error::Communication(format!("failed to communicate with light: {error}"))
        })?;
        Ok(if status.emitting() {
            SwitchState::On
        } else {
            SwitchState::Off
        })
    }
}

impl WriteValue for LightSwitch<'_> {
    type Item = SwitchState;

    fn set(&self, value: Self::Item) -> Box<dyn Future<Output=()> + Unpin + Send + '_> {
        Box::new({
            let mode = match value {
                SwitchState::On => PowerMode::On,
                SwitchState::Off => PowerMode::Off
            };
            self.wiz.set_power(&mode).map_err(|error| {
                Error::Communication(format!("failed to communicate with light: {error}"))
            })?;
        })
    }
}
*/

#[derive(Debug, Clone, Deserialize)]
struct Response<T> {
    method: String,
    env: String,
    result: T
}

async fn udp_request<Request, Data>(addr: Ipv4Addr, msg: Request) -> Result<Response<Data>, Error>
where
    Request: Serialize,
    for<'de> Data: Deserialize<'de>,
{
    // dump the control message to string
    let msg = match serde_json::to_vec(&msg) {
        Ok(v) => v,
        Err(e) => return Err(Error::JsonSerialize(e)),
    };

    // get some udp socket from the os
    let socket = match UdpSocket::bind("0.0.0.0:38899") {
        Ok(s) => s,
        Err(e) => return Err(Error::socket("bind", e)),
    };

    // connect to the remote bulb at their standard port
    match socket.connect((addr, 38899)) {
        Ok(_) => {}
        Err(e) => return Err(Error::socket("connect", e)),
    }

    // send the control message
    match socket.send(&msg) {
        Ok(_) => {}
        Err(e) => return Err(Error::socket("send", e)),
    };

    // declare a buffer of the max message size
    let mut buffer = [0; 4096];
    let bytes = match socket.recv(&mut buffer) {
        Ok(b) => b,
        Err(e) => return Err(Error::socket("receive", e)),
    };

    // create some JSON object from the string
    match serde_json::from_slice(&buffer[..bytes]) {
        Ok(v) => Ok(v),
        Err(e) => Err(Error::JsonSerialize(e)),
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Used when failing to serialize json
    #[error("failed to serialize json: {0:?}")]
    JsonSerialize(serde_json::Error),

    /// Used when failing to deserialize json
    #[error("failed to deserialize json: {0:?}")]
    JsonDeserialize(serde_json::Error),

    /// Some socket error when communicating with a bulb
    #[error("socket {action} error: {err:?}")]
    Socket { action: String, err: std::io::Error },

    /// Attempting to look up or modify a light which doesn't exist
    #[error("light {light_id:?} not found")]
    LightNotFound { light_id: Ipv4Addr },

    /// Attempting to add a light with an invalid IP
    #[error("light with ip {ip} is invalid because the IP is {reason}")]
    InvalidIP { ip: Ipv4Addr, reason: String },
}

impl Error {
    /// Create a new socket error
    pub fn socket(action: &str, err: std::io::Error) -> Self {
        Self::Socket {
            action: action.to_string(),
            err,
        }
    }

    /// Create a new light not found error
    pub fn light_not_found(light_id: &Ipv4Addr) -> Self {
        Self::LightNotFound {
            light_id: *light_id,
        }
    }

    /// Create a new invalid IP error
    pub fn invalid_ip(ip: &Ipv4Addr, reason: &str) -> Self {
        Self::InvalidIP {
            ip: *ip,
            reason: reason.to_string(),
        }
    }
}
