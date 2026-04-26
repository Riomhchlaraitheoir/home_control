#![doc = include_str!("../README.md")]
#![allow(missing_docs)]

use derive_more::Display;
use std::collections::HashMap;
use thiserror::Error;
use trait_rpc::{rpc, serde};

pub use reflect::value::{Range, RangeBound};
use reflect::{Error, SetError};
pub use trait_rpc;
use trait_rpc::serde::{Deserialize, Serialize};

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Display)]
#[serde(crate = "trait_rpc::serde")]
pub enum ValueType {
    #[display("bool")]
    Bool,
    #[display("int({_0})")]
    Int(Range<i64>),
    #[display("float")]
    Float,
    #[display("string({values:?})")]
    String { values: Option<Vec<String>> },
    #[display("option({_0})")]
    Optional(Box<ValueType>),
}

impl From<reflect::value::ValueType> for ValueType {
    fn from(value: reflect::value::ValueType) -> Self {
        match value {
            reflect::value::ValueType::Bool => Self::Bool,
            reflect::value::ValueType::Int(range) => Self::Int(range),
            reflect::value::ValueType::Float => Self::Float {},
            reflect::value::ValueType::String { values } => Self::String { values },
            reflect::value::ValueType::Optional(value) => Self::Optional(Box::new((*value).into())),
        }
    }
}

#[rpc]
pub trait Api {
    /// No-op method for checking connection
    fn ping() -> u8;
    /// Get a list of devices
    fn get_devices() -> Stream<Device>;

    /// call some method for a particular device
    fn device(device_name: String) -> impl DeviceApi;
}

#[rpc]
pub trait DeviceApi {
    /// get this device
    fn get() -> Option<Device>;
    /// get the service for this device's field with the given name
    fn field(field_name: String) -> impl FieldApi;
}

#[rpc]
pub trait FieldApi {
    fn get_and_subscribe() -> Stream<Result<Value, OperationError>>;
    fn subscribe() -> Stream<Result<Value, OperationError>>;
    /// Get the current value of this field
    fn get() -> Result<Value, OperationError>;
    /// Update the current value of this field
    fn set(value: Value) -> Result<(), OperationError>;
    /// Toggle the current value of this field
    fn toggle() -> Result<(), OperationError>;
}

/// A dynamic value which is used when accessing the field dynamically
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Display)]
#[serde(crate = "trait_rpc::serde")]
#[serde(untagged)]
pub enum Value {
    /// A boolean value
    Bool(bool),
    /// A integer value
    Int(i64),
    /// A float value
    Float(f64),
    /// A string value
    String(String),
    /// a absent value
    None,
}

impl Value {
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Bool(_) => "bool",
            Value::Int(_) => "int",
            Value::Float(_) => "float",
            Value::String(_) => "string",
            Value::None => "null"
        }
    }
}

impl From<reflect::value::Value> for Value {
    fn from(value: reflect::value::Value) -> Self {
        match value {
            reflect::value::Value::Bool(value) => Self::Bool(value),
            reflect::value::Value::Int(value) => Self::Int(value),
            reflect::value::Value::Float(value) => Self::Float(value),
            reflect::value::Value::String(value) => Self::String(value),
            reflect::value::Value::None => Self::None,
        }
    }
}

impl From<Value> for reflect::value::Value {
    fn from(value: Value) -> Self {
        match value {
            Value::Bool(value) => Self::Bool(value),
            Value::Int(value) => Self::Int(value),
            Value::Float(value) => Self::Float(value),
            Value::String(value) => Self::String(value),
            Value::None => Self::None,
        }
    }
}

/// An error which can occur when attempting an operation
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Error)]
#[serde(crate = "trait_rpc::serde")]
pub enum OperationError {
    /// No field with the given name was found
    #[error("Field '{field}' does not exist for device: '{device}'")]
    FieldNotFound {
        /// The device name
        device: String,
        /// The field name
        field: String,
    },
    /// This operation is not supported for this field
    #[error("Field '{field}' of device: '{device}' does not support {operation} operations")]
    OperationNotSupported {
        /// The device name
        device: String,
        /// The field name
        field: String,
        /// The operation attempted
        operation: Operation,
    },
    /// device not found
    #[error("Device '{0}' not found")]
    DeviceNotFound(String),
    /// Operation Failed
    #[error("Operation failed: {0}")]
    Failure(String),
    #[error("Invalid value: {0}")]
    /// The provided value was invalid, only occurs on a set operation
    ParseError(#[from] ValueReadError),
}

/// the set of operations supported by a given field
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "trait_rpc::serde")]
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

impl From<reflect::Operations> for Operations {
    fn from(value: reflect::Operations) -> Self {
        Self {
            subscribe: value.subscribe,
            get: value.get,
            set: value.set,
            toggle: value.toggle,
        }
    }
}

/// An operation
#[derive(serde::Serialize, serde::Deserialize, Debug, Copy, Clone, Display)]
#[serde(crate = "trait_rpc::serde")]
#[allow(missing_docs)]
pub enum Operation {
    #[display("subscribe")]
    Subscribe,
    #[display("get")]
    Get,
    #[display("set")]
    Set,
    #[display("toggle")]
    Toggle,
}

impl From<reflect::Operation> for Operation {
    fn from(value: reflect::Operation) -> Self {
        match value {
            reflect::Operation::Subscribe => Self::Subscribe,
            reflect::Operation::Get => Self::Get,
            reflect::Operation::Set => Self::Set,
            reflect::Operation::Toggle => Self::Toggle,
        }
    }
}

impl From<reflect::Error> for OperationError {
    fn from(error: reflect::Error) -> Self {
        match error {
            Error::FieldNotFound { device, field } => Self::FieldNotFound { device, field },
            Error::OperationNotSupported {
                device,
                field,
                operation,
            } => Self::OperationNotSupported {
                device,
                field,
                operation: operation.into(),
            },
        }
    }
}

impl From<reflect::SetError> for OperationError {
    fn from(error: reflect::SetError) -> Self {
        match error {
            SetError::Error(error) => error.into(),
            SetError::ParseError(error) => Self::ParseError(error.into()),
        }
    }
}

impl From<anyhow::Error> for OperationError {
    fn from(error: anyhow::Error) -> Self {
        Self::Failure(error.to_string())
    }
}

/// A Device struct send between the backend and ui
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
#[serde(crate = "trait_rpc::serde")]
pub struct Device {
    /// The device's internal ID string
    pub id: String,
    /// The device's display name
    pub name: String,
    /// A description of the device
    pub description: Option<String>,
    /// Device tags
    pub tags: HashMap<String, String>,
    /// The type of the device
    pub device_type: DeviceType,
    /// The device's fields
    pub fields: Vec<Field>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
#[serde(crate = "trait_rpc::serde")]
pub enum DeviceType {
    /// A light
    Light,
    /// A switch/button
    Switch,
    /// Some kind of sensor
    Sensor,
    /// Other indicates that this device does not fit any of the other types
    Other,
    /// Unknown indicates a new device type that this version of the API isn't aware of
    #[serde(other)]
    Unknown,
}

impl From<reflect::DeviceType> for DeviceType {
    fn from(value: reflect::DeviceType) -> Self {
        match value {
            reflect::DeviceType::Light => Self::Light,
            reflect::DeviceType::Switch => Self::Switch,
            reflect::DeviceType::Sensor => Self::Sensor,
            reflect::DeviceType::Other => Self::Other,
        }
    }
}

#[cfg(feature = "server")]
impl From<(reflect::DeviceInfo, Vec<reflect::Field>)> for Device {
    fn from((info, fields): (reflect::DeviceInfo, Vec<reflect::Field>)) -> Self {
        Self {
            id: info.id,
            name: info.name,
            description: info.description,
            tags: info.tags,
            device_type: info.device_type.into(),
            fields: fields.into_iter().map(Into::into).collect(),
        }
    }
}

#[cfg(feature = "server")]
impl From<&dyn reflect::Device> for Device {
    fn from(value: &dyn reflect::Device) -> Self {
        (value.info(), value.fields()).into()
    }
}

#[cfg(feature = "server")]
impl From<reflect::Field> for Field {
    fn from(info: reflect::Field) -> Self {
        Self {
            name: info.name,
            description: info.description,
            operations: info.operations.into(),
            value_type: info.value_type.into(),
        }
    }
}

/// A field of a device
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
#[serde(crate = "trait_rpc::serde")]
pub struct Field {
    /// The name of the field
    pub name: String,
    /// A description of the field
    pub description: String,
    /// Detail which operations are supported by this field
    pub operations: Operations,
    /// The value type of this field
    pub value_type: ValueType,
}

#[derive(Debug, Error, Serialize, Deserialize, Clone)]
#[serde(crate = "trait_rpc::serde")]
/// An error in converting a [reflect::value::Value] to the correct type for a certain field
pub enum ValueReadError {
    /// the provided value was the wrong type
    #[error("wrong type, expected: {expected_type}, found: {actual_type}")]
    WrongType {
        /// The expected type
        expected_type: ValueType,
        /// The actual type received
        actual_type: String,
    },
    #[error("Invalid enum value, {invalid} is not an acceptable value, values: {valid:?}")]
    /// The given string does not match any of the allowed enum values
    IllegalEnum {
        /// The invalid string
        invalid: String,
        /// The set of valid strings
        valid: Vec<String>,
    },
    #[error("Integer not in expected range, value: {value}, range: {range}")]
    /// The provided int was not in range
    IntNotInRange {
        /// The provided int
        value: i64,
        /// The expected Range
        range: Range<i64>,
    },
    #[error("Float not in expected range, value: {value}, range: {range}")]
    /// The provided float was not in range
    FloatNotInRange {
        /// The provided float
        value: f64,
        /// The expected Range
        range: Range<f64>,
    },
}

impl From<reflect::value::ValueReadError> for ValueReadError {
    fn from(value: reflect::value::ValueReadError) -> Self {
        match value {
            reflect::value::ValueReadError::WrongType {
                expected_type,
                actual_type,
            } => Self::WrongType {
                expected_type: expected_type.into(),
                actual_type,
            },
            reflect::value::ValueReadError::IllegalEnum { invalid, valid } => {
                Self::IllegalEnum { invalid, valid }
            }
            reflect::value::ValueReadError::IntNotInRange { value, range } => {
                Self::IntNotInRange { value, range }
            }
        }
    }
}
