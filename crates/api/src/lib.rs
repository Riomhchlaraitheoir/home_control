#![doc = include_str!("../README.md")]

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use bon::builder;
use control::reflect;
use control::reflect::value::{Value, ValueReadError};
use control::reflect::{Device, SetError};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

#[builder]
/// Build an API [Router]
pub fn api(#[builder(field)] devices: HashMap<String, Box<dyn Device>>) -> Router {
    Router::new()
        .route(
            "/devices/{device_name}/fields/{field_name}",
            get(get_value).post(set_value),
        )
        .with_state(Arc::new(ApiState { devices }))
}

impl<S: api_builder::State> ApiBuilder<S> {
    /// Add a device to be served by this API
    pub fn add_device(mut self, device: impl Device + 'static) -> Self {
        self.devices.insert(device.name(), Box::new(device));
        self
    }

    /// Add devices to be served by this API
    pub fn add_devices(mut self, devices: impl IntoIterator<Item = Box<dyn Device>>) -> Self {
        self.devices
            .extend(devices.into_iter().map(|device| (device.name(), device)));
        self
    }
}

struct ApiState {
    devices: HashMap<String, Box<dyn Device>>,
}

impl ApiState {
    fn get_device(&self, device_name: &str) -> Result<&dyn Device, Error> {
        Ok(self
            .devices
            .get(device_name)
            .ok_or_else(|| Error {
                error: ErrorType::DeviceNotFound,
                message: format!("Unknown device: '{device_name}'"),
            })?
            .as_ref())
    }
}

#[derive(Debug, Deserialize)]
struct GetQuery {
    subscribe: bool,
}

async fn get_value(
    State(state): State<Arc<ApiState>>,
    query: Query<GetQuery>,
    Path((device, field)): Path<(String, String)>,
) -> Result<Json<Value>, Error> {
    let device = state.get_device(&device)?;
    if query.subscribe {
        device
            .subscribe(&field)?
            .next()
            .await
            .map(Json)
            .ok_or(Error {
                error: ErrorType::StreamClosed,
                message: "".to_string(),
            })
    } else {
        Ok(Json(device.get(&field)?.await?))
    }
}

async fn set_value(
    State(state): State<Arc<ApiState>>,
    Path((device, field)): Path<(String, String)>,
    body: Option<Json<Value>>,
) -> Result<(), Error> {
    let device = state.get_device(&device)?;
    if let Some(Json(value)) = body {
        device.set(&field, value)?.await?;
    } else {
        device.toggle(&field)?.await?;
    }
    Ok(())
}


#[derive(Debug, Serialize)]
struct Error {
    error: ErrorType,
    message: String,
}

impl From<reflect::Error> for Error {
    fn from(error: reflect::Error) -> Self {
        Self {
            error: ErrorType::from(&error),
            message: error.to_string(),
        }
    }
}

impl From<SetError> for Error {
    fn from(error: SetError) -> Self {
        match error {
            SetError::Error(error) => error.into(),
            SetError::ParseError(error) => {
                let message = format!("Parsing error: {error}");
                Self {
                    error: ErrorType::InvalidInput(error),
                    message,
                }
            }
        }
    }
}

impl From<anyhow::Error> for Error {
    fn from(error: anyhow::Error) -> Self {
        Self {
            error: ErrorType::Backend,
            message: format!("Failed to process request: {}", error),
        }
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        let status = self.error.status();
        if let ErrorType::StreamClosed = &self.error {
            // Do not include any error with StreamClosed response
            return status.into_response();
        }
        (status, Json(self)).into_response()
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum ErrorType {
    DeviceNotFound,
    FieldNotFound,
    OperationNotSupported,
    InvalidInput(ValueReadError),
    StreamClosed,
    Backend,
}

impl ErrorType {
    fn status(&self) -> StatusCode {
        match self {
            ErrorType::DeviceNotFound => StatusCode::NOT_FOUND,
            ErrorType::FieldNotFound => StatusCode::NOT_FOUND,
            ErrorType::OperationNotSupported => StatusCode::METHOD_NOT_ALLOWED,
            ErrorType::InvalidInput(_) => StatusCode::BAD_REQUEST,
            ErrorType::StreamClosed => StatusCode::NO_CONTENT,
            ErrorType::Backend => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<&reflect::Error> for ErrorType {
    fn from(error: &reflect::Error) -> Self {
        match error {
            reflect::Error::FieldNotFound { .. } => ErrorType::FieldNotFound,
            reflect::Error::OperationNotSupported { .. } => ErrorType::OperationNotSupported,
        }
    }
}
