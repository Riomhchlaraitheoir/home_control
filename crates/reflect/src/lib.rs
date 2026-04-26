//! This module provides an interface to interact with devices and their values dynamically

pub mod value;

use std::collections::HashMap;
use std::convert::Infallible;
use derive_more::Display;
use futures::future::BoxFuture;
use futures::stream::BoxStream;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use value::{Value, ValueReadError, ValueType};

#[derive(Serialize, Deserialize, Debug, Clone)]
/// Basic information regarding a device
pub struct DeviceInfo {
    /// The device's internal ID string
    pub id: String,
    /// The device's display name
    pub name: String,
    /// A description of the device
    pub description: Option<String>,
    /// The type of device this is
    pub device_type: DeviceType,
    /// Device tags
    pub tags: HashMap<String, String>,
}

/// The broad category of a device
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum DeviceType {
    /// A light
    Light,
    /// A switch/button
    Switch,
    /// Some kind of sensor
    Sensor,
    /// Other indicates that this device does not fit any of the other types
    Other
}

/// A Device which supports dynamic access
pub trait Device: Send + Sync {
    /// Returns the name of this device
    fn name(&self) -> String {
        self.info().name
    }
    /// Return the information for this device
    fn info(&self) -> DeviceInfo;
    /// Return the fields for this device
    fn fields(&self) -> Vec<Field>;
    /// subscribe to updates from the given field
    fn subscribe(&self, field: &str) -> Result<BoxFuture<'_, BoxStream<'_, Value>>, Error>;

    /// get the current state of the given field
    fn get(&self, field: &str) -> Result<BoxFuture<'_, anyhow::Result<Value>>, Error>;

    /// set the given field with a certain value
    fn set(&self, field: &str, value: Value) -> Result<BoxFuture<'_, anyhow::Result<()>>, SetError>;

    /// toggle the given field
    fn toggle(&self, field: &str) -> Result<BoxFuture<'_, anyhow::Result<()>>, Error>;
}

impl dyn Device {}

#[derive(Serialize, Deserialize, Clone)]
/// A device field specification
pub struct Field {
    /// The field's name
    pub name: String,
    /// A description of the field
    pub description: String,
    /// Detail which operations are supported by this field
    pub operations: Operations,
    /// The value type of this field
    pub value_type: ValueType,
}

/// the set of operations supported by a given field
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Operations {
    /// Supports subscribe operations
    pub subscribe: bool,
    /// Supports get operations
    pub get: bool,
    /// Supports set operations
    pub set: bool,
    /// Supports toggle operations
    pub toggle: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, Error)]
/// An error which can occur when accessing a device dynamically
#[allow(missing_docs)]
pub enum Error {
    /// No field with the given name was found
    #[error("Field '{field}' does not exist for device: '{device}'")]
    FieldNotFound {
        device: String,
        field: String,
    },
    /// This operation is not supported for this field
    #[error("Field '{field}' of device: '{device}' does not support {operation} operations")]
    OperationNotSupported {
        device: String,
        field: String,
        operation: Operation,
    },
}

/// An operation
#[derive(serde::Serialize, serde::Deserialize, Debug, Display, Copy, Clone)]
#[allow(missing_docs)]
pub enum Operation {
    #[display("subscribe")]
    Subscribe,
    #[display("get")]
    Get,
    #[display("set")]
    Set,
    #[display("toggle")]
    Toggle
}

#[derive(Debug, Error, Serialize,Deserialize, Clone)]
/// An error that can happen when trying to set a field dynamically
pub enum SetError {
    /// A [Error]
    #[error(transparent)]
    Error(#[from] Error),
    #[error("Invalid value: {0}")]
    /// The provided value was invalid
    ParseError(#[from] ValueReadError),
}

impl From<Infallible> for SetError {
    fn from(err: Infallible) -> Self {
        match err {  }
    }
}

impl From<Infallible> for Error {
    fn from(err: Infallible) -> Self {
        match err {  }
    }
}
