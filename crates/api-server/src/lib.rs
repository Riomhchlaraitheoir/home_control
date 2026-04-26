#![doc = include_str!("../README.md")]

use api::trait_rpc::server::axum::Axum;
use api::trait_rpc::server::{IntoHandler, StreamError};
use api::{
    Api, ApiServer, DeviceApi, DeviceApiServer, FieldApi, FieldApiServer,
    OperationError,
};
use api::{Device as ApiDevice, Value};
use axum::extract::{FromRequestParts, State};
use axum::Router;
use bon::builder;
use control::device::DeviceSet;
use control::reflect::Device;
use futures::{Sink, SinkExt, StreamExt};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::pin::pin;
use std::sync::Arc;
use tracing::warn;

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
            .allow_json()
            .allow_cbor()
            .enable_websockets()
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
    async fn ping(&self) -> u8 { 0 }

    async fn get_devices<'a>(&'a self, sink: impl Sink<ApiDevice, Error=StreamError> + Send + 'a) {
        let mut sink = pin!(sink);
        for device in self.state.devices.values() {
            let device = ApiDevice::from((device.info(), device.fields()));
            if let Err(error) =sink.send(device).await {
                warn!("Failed to send device on stream: {error}");
                return;
            };
        }
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
    #[allow(clippy::expect_used, reason = "TODO: find better solution, return error?")]
    async fn get_and_subscribe<'a>(&'a self, sink: impl Sink<Result<Value, OperationError>, Error=StreamError> + Send + 'a) {
        let mut sink = pin!(sink);
        let result = self.devices
            .get(&self.device_name.to_string())
            .ok_or(OperationError::DeviceNotFound(self.device_name.to_string()));
        let device = match result {
            Ok(device) => device,
            Err(error) => {
                sink.send(Err(error)).await.expect("failed to send error");
                return;
            }
        };
        // establish subscribe before get to ensure no missed updates, may result in duplicates
        let mut stream = match device.subscribe(&self.field_name) {
            Ok(stream) => stream.await,
            Err(error) => {
                sink.send(Err(error.into())).await.expect("failed to send error");
                return;
            }
        };
        let future = match device.get(&self.field_name) {
            Ok(value) => value,
            Err(error) => {
                sink.send(Err(error.into())).await.expect("failed to send error");
                return;
            }
        };
        let current = match future.await {
            Ok(value) => value,
            Err(error) => {
                sink.send(Err(error.into())).await.expect("failed to send error");
                return;
            }
        };
        sink.send(Ok(current.into())).await.expect("failed to send error");
        while let Some(value) = stream.next().await {
            sink.send(Ok(value.into())).await.expect("failed to send value");
        }
    }

    #[allow(clippy::expect_used, reason = "TODO: find better solution, return error?")]
    async fn subscribe<'a>(&'a self, sink: impl Sink<Result<Value, OperationError>, Error=StreamError> + Send + 'a) {
        let mut sink = pin!(sink);
        let result = self.devices
            .get(&self.device_name.to_string())
            .ok_or(OperationError::DeviceNotFound(self.device_name.to_string()));
        let device = match result {
            Ok(device) => device,
            Err(error) => {
                sink.send(Err(error)).await.expect("failed to send error");
                return;
            }
        };
        let mut stream = match device.subscribe(&self.field_name) {
            Ok(stream) => stream.await,
            Err(error) => {
                sink.send(Err(error.into())).await.expect("failed to send error");
                return;
            }
        };
        while let Some(value) = stream.next().await {
            sink.send(Ok(value.into())).await.expect("failed to send value");
        }
    }

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
