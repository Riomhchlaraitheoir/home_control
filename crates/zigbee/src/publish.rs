use serde::{Deserialize, Serialize};
use std::string::FromUtf8Error;

#[derive(Debug, Clone)]
pub(crate) struct Publish {
    pub topic: String,
    pub raw_payload: String
}

impl Publish {
    pub fn new(topic: String, payload: impl Serialize) -> Result<Self, serde_json::Error> {
        let payload = serde_json::to_string(&payload)?;
        Ok(Self {
            topic,
            raw_payload: payload
        })
    }

    pub fn payload<'a, R: Deserialize<'a>>(&'a self) -> Result<R, serde_json::Error> {
        serde_json::from_str(&self.raw_payload)
    }
}

impl TryFrom<rumqttc::Publish> for Publish {
    type Error = FromUtf8Error;
    fn try_from(value: rumqttc::Publish) -> Result<Self, Self::Error> {
        let payload = String::try_from(value.payload.to_vec())?;
        Ok(Self {
            topic: value.topic,
            raw_payload: payload,
        })
    }
}
