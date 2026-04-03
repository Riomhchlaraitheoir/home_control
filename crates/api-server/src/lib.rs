#![doc = include_str!("../README.md")]

use api::{Device as ApiDevice, Value};
use api::trait_rpc::server::axum::Axum;
use api::{
    Api, ApiServer, DeviceApi, DeviceApiServer, FieldApi, FieldApiServer,
    OperationError,
};
use axum::Router;
use bon::builder;
use control::device::DeviceSet;
use control::reflect::Device;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;
use axum::extract::{FromRequestParts, State};
use api::trait_rpc::server::IntoHandler;

#[builder]
#[builder(finish_fn = build)]
/// Build an API [Router]
pub fn api(#[builder(field)] devices: HashMap<String, Box<dyn Device>>) -> Router {
    Router::new().route_service(
        "/api",
        Axum::builder()
            .rpc(PhantomData::<Api>)
            .server(PhantomData::<Server>)
            .state(Arc::new(ServerState { devices }))
            .allow_post()
            .allow_json()
            .allow_cbor()
            .build(),
    )
}

impl<S: api_builder::State> ApiBuilder<S> {
    /// Add a device to the Api server
    pub fn add_device(mut self, device: impl Device + 'static) -> Self {
        self.devices
            .insert(device.info().id.to_string(), Box::new(device));
        self
    }

    /// Add a set of devices to the API server
    pub fn add_device_set(mut self, set: impl DeviceSet + 'static) -> Self {
        for device in set {
            self.devices.insert(device.info().id.to_string(), device);
        }
        self
    }
}

struct ServerState {
    devices: HashMap<String, Box<dyn Device>>,
}

#[derive(FromRequestParts)]
struct Server {
    state: State<Arc<ServerState>>
}

impl ApiServer for Server {
    async fn get_devices(&self) -> Vec<ApiDevice> {
        self.state
            .devices
            .values()
            .map(|v| ApiDevice::from((v.info(), v.fields())))
            .collect()
    }

    async fn device(&self, device_name: String) -> impl IntoHandler<DeviceApi> {
        DeviceServer {
            devices: &self.state.devices,
            device_id: device_name,
        }
    }
}

struct DeviceServer<'a> {
    devices: &'a HashMap<String, Box<dyn Device>>,
    device_id: String,
}

impl DeviceApiServer for DeviceServer<'_> {
    async fn get(&self) -> Option<ApiDevice> {
        Some(self.devices.get(&self.device_id.to_string())?.as_ref().into())
    }

    async fn field(&self, field_name: String) -> impl IntoHandler<FieldApi> {
        FieldServer {
            devices: self.devices,
            device_name: &self.device_id,
            field_name,
        }
    }
}

struct FieldServer<'a> {
    devices: &'a HashMap<String, Box<dyn Device>>,
    device_name: &'a String,
    field_name: String,
}

impl FieldApiServer for FieldServer<'_> {
    async fn get(&self) -> Result<Value, OperationError> {
        Ok(
            self.devices
            .get(&self.device_name.to_string())
            .ok_or(OperationError::DeviceNotFound(self.device_name.to_string()))?
            .get(&self.field_name)?
            .await?
            .into()
        )
    }

    async fn set(&self, value: Value) -> Result<(), OperationError> {
        self.devices
            .get(&self.device_name.to_string())
            .ok_or(OperationError::DeviceNotFound(self.device_name.to_string()))?
            .set(&self.field_name, value.into())?
            .await?;
        Ok(())
    }

    async fn toggle(&self) -> Result<(), OperationError> {
        todo!()
    }
}
