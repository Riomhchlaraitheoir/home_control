//! This module provides an interface to interact with devices and their values dynamically

pub mod value;

use std::convert::Infallible;
use derive_more::Display;
use futures::future::BoxFuture;
use futures::stream::BoxStream;
use thiserror::Error;
use crate::reflect::value::{Value, ValueReadError, ValueType};

/// A Device which supports dynamic access
pub trait Device: Send + Sync {
    /// Returns the name of this device
    fn name(&self) -> String {
        self.info().name
    }
    /// Return the information for this device
    fn info(&self) -> DeviceInfo;
    /// subscribe to updates from the given field
    fn subscribe(&self, field: &str) -> Result<BoxStream<'_, Value>, Error>;

    /// get the current state of the given field
    fn get(&self, field: &str) -> Result<BoxFuture<'_, anyhow::Result<Value>>, Error>;

    /// set the given field with a certain value
    fn set(&self, field: &str, value: Value) -> Result<BoxFuture<'_, anyhow::Result<()>>, SetError>;

    /// toggle the given field
    fn toggle(&self, field: &str) -> Result<BoxFuture<'_, anyhow::Result<()>>, Error>;
}

impl dyn Device {}

/// A device's info
pub struct DeviceInfo {
    /// The name of the device
    pub name: String,
    /// The device's fields
    pub fields: Vec<Field>,
}

/// A device field specification
pub struct Field {
    /// The field's name
    pub name: String,
    /// Does this field support subscribe operations
    pub allow_subscribe: bool,
    /// Does this field support get operations
    pub allow_get: bool,
    /// Does this field support set operations
    pub allow_set: bool,
    /// Does this field support toggle operations
    pub allow_toggle: bool,
    /// The value type of this field
    pub value_type: ValueType,
}

#[derive(Debug, Error)]
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
#[derive(Debug, Display)]
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

#[derive(Debug, Error)]
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
