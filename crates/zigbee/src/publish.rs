use serde::{Deserialize, Serialize};

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

impl From<rumqttc::Publish> for Publish {
    fn from(value: rumqttc::Publish) -> Self {
        let payload = String::try_from(value.payload.to_vec()).expect("payload should be UTF-8");
        Self {
            topic: value.topic,
            raw_payload: payload,
        }
    }
}
