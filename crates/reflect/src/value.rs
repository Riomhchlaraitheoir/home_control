//! This module defines the [Value] enum and related tpyes along with implementing conversions for a number of existing type to and from [Value]

use derive_more::Display;
use light_ranged_integers::{RangedI16, RangedI32, RangedI8, RangedU16, RangedU32, RangedU8};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::ops::RangeInclusive;
use thiserror::Error;

/// A dynamic value which is used when accessing the field dynamically
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
    None
}

/// Implement the needed traits to use this enum type as a value
/// ```
/// use reflect::enum_value;
/// enum EnumType {
///     Variant1,
///     Another
/// }
/// 
/// enum_value!(EnumType,
///     "string_variant" => Variant1,
///     "another-variant" => Another
/// );
#[macro_export]
macro_rules! enum_value {
    ($ty:ty, $($s:literal => $v:ident),+) => {
impl $crate::value::AsValueType for $ty {
    fn value_type() -> $crate::value::ValueType {
        $crate::value::ValueType::String {
            values: Some(vec![$($s.to_string()),+]),
        }
    }
}

impl From<$ty> for $crate::value::Value {
    fn from(enum_value: $ty) -> Self {
        Self::String(match enum_value {
            $(<$ty>::$v => $s),+
        }.to_owned())
    }
}

impl TryFrom<$crate::value::Value> for $ty {
    type Error = $crate::value::ValueReadError;
    fn try_from(value: $crate::value::Value) -> Result<Self, Self::Error> {
        let $crate::value::Value::String(value) = value else {
            return Err($crate::value::ValueReadError::WrongType {
                expected_type: $crate::value::ValueType::String {
                    values: Some(vec![$($s.to_string()),+]),
                },
                actual_type: value.value_type(),
            })
        };
        match value.as_str() {
            $($s => Ok(<$ty>::$v),)+
            unknown => Err($crate::value::ValueReadError::IllegalEnum {
                invalid: unknown.to_string(),
                valid: vec![$($s.to_string()),+],
            })
        }
    }
}

    };
}

impl Value {
    #[doc(hidden)]
    pub fn value_type(&self) -> String {
        match self {
            Value::Bool(_) => "bool",
            Value::Int(_) => "int",
            Value::Float(_) => "float",
            Value::String(_) => "string",
            Value::None => "none",
        }.to_owned()
    }
}

/// A value type
#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Display)]
#[serde(tag = "type")]
pub enum ValueType {
    #[display("bool")]
    Bool,
    #[display("int({_0})")]
    Int(Range<i64>),
    #[display("float")]
    Float,
    #[display("string({values:?})")]
    String {
        values: Option<Vec<String>>
    },
    #[display("option({_0})")]
    Optional(Box<ValueType>),
}

/// Represents a type which can be represented by [ValueType]
pub trait AsValueType {
    /// Get the appropriate [ValueType] to represent [Self]
    fn value_type() -> ValueType;
}

impl ValueType {
    /// Get the ValueType from the given type
    pub fn from_type<T: AsValueType>() -> Self {
        T::value_type()
    }

    /// Validate the given value as acceptable for this type
    pub fn validate(&self, other: &Value) -> Result<(), ValueReadError> {

        match (self, other) {
            (Self::Bool, Value::Bool(_)) => Ok(()),
            (Self::Int(range), Value::Int(value)) => {
                if range.contains(value) {
                    Ok(())
                } else {
                    Err(ValueReadError::IntNotInRange {
                        value: *value,
                        range: *range,
                    })
                }
            },
            (Self::Float, Value::Float(_)) => Ok(()),
            (Self::String { values: Some(values) }, Value::String(value)) => {
                if values.contains(value) {
                    Ok(())
                } else {
                    Err(ValueReadError::IllegalEnum {
                        invalid: value.to_string(),
                        valid: values.clone(),
                    })
                }
            },
            (Self::String { values: None }, Value::String(_)) => Ok(()),
            (Self::Optional(_), Value::None) => Ok(()),
            (Self::Optional(value_type), other) => value_type.validate(other),
            (value_type, value) => {
                Err(ValueReadError::WrongType {
                    expected_type: value_type.clone(),
                    actual_type: value.value_type(),
                })
            },
        }
    }
}

impl AsValueType for bool {
    fn value_type() -> ValueType {
        ValueType::Bool
    }
}

impl AsValueType for f64 {
    fn value_type() -> ValueType {
        ValueType::Float
    }
}

impl AsValueType for String {
    fn value_type() -> ValueType {
        ValueType::String { values: None }
    }
}

impl<V: AsValueType> AsValueType for Option<V> {
    fn value_type() -> ValueType {
        ValueType::Optional(Box::new(V::value_type()))
    }
}

impl<V: Into<Value>> From<Option<V>> for Value {
    fn from(v: Option<V>) -> Self {
        match v {
            None => Self::None,
            Some(value) => value.into(),
        }
    }
}

impl<V> TryFrom<Value> for Option<V>
where
    V: TryFrom<Value, Error=ValueReadError>
{
    type Error = ValueReadError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        if value == Value::None {
            Ok(None)
        } else {
            Ok(Some(V::try_from(value)?))
        }
    }
}

#[derive(Debug, Error, Serialize, Deserialize, Clone)]
/// An error in converting a [Value] to the correct type for a certain field
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
}

/// Custom range type to describe any kind of range in a single concrete type
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Range<T> {
    /// start of the range
    pub start: RangeBound<T>,
    /// end of the range
    pub end: RangeBound<T>
}

impl<T: Display> Display for Range<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.start {
            RangeBound::Included(start) => { write!(f, "{start}=")?; },
            RangeBound::Excluded(start) => { write!(f, "{start}")?; },
            RangeBound::Open => {}
        }
        f.write_str("..")?;
        match &self.end {
            RangeBound::Included(end) => { write!(f, "={end}")?; },
            RangeBound::Excluded(end) => { write!(f, "{end}")?; },
            RangeBound::Open => {}
        }
        Ok(())
    }
}

impl<T: Ord> Range<T> {
    /// Returns true if the given value is within this range
    pub fn contains(&self, value: &T) -> bool {
        let start = match &self.start {
            RangeBound::Included(start) => value >= start,
            RangeBound::Excluded(start) => value > start,
            RangeBound::Open => true
        };
        let end = match &self.end {
            RangeBound::Included(end) => value <= end,
            RangeBound::Excluded(end) => value < end,
            RangeBound::Open => true
        };
        start && end
    }
}

/// A bound of a range
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "bound")]
pub enum RangeBound<T> {
    /// an inclusive bound
    Included(T),
    /// an exclusive bound
    Excluded(T),
    /// an open bound
    Open
}

impl<T: Copy, V: From<T>> From<RangeInclusive<T>> for Range<V> {
    fn from(value: RangeInclusive<T>) -> Self {
        Self {
            start: RangeBound::Included((*value.start()).into()),
            end: RangeBound::Included((*value.end()).into())
        }
    }
}

impl<T: Copy, V: From<T>> From<std::ops::Range<T>> for Range<V> {
    fn from(value: std::ops::Range<T>) -> Self {
        Self {
            start: RangeBound::Included(value.start.into()),
            end: RangeBound::Excluded(value.end.into())
        }
    }
}

impl<T: Copy, V: From<T>> From<std::ops::RangeFrom<T>> for Range<V> {
    fn from(value: std::ops::RangeFrom<T>) -> Self {
        Self {
            start: RangeBound::Included(value.start.into()),
            end: RangeBound::Open
        }
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Value::Bool(value)
    }
}

impl TryFrom<Value> for bool {
    type Error = ValueReadError;
    fn try_from(value: Value) -> Result<Self, Self::Error> {
        let Value::Bool(value) = value else {
            return Err(ValueReadError::WrongType {
                expected_type: ValueType::Bool,
                actual_type: value.value_type(),
            })
        };
        Ok(value)
    }
}

impl From<i64> for Value {
    fn from(value: i64) -> Self {
        Value::Int(value)
    }
}

impl TryFrom<Value> for i64 {
    type Error = ValueReadError;
    fn try_from(value: Value) -> Result<Self, Self::Error> {
        let Value::Int(value) = value else {
            return Err(ValueReadError::WrongType {
                expected_type: ValueType::Bool,
                actual_type: value.value_type(),
            })
        };
        Ok(value)
    }
}

macro_rules! impl_int {
    ($($int:ident:$ranged:ident),*) => {
$(
impl AsValueType for $int {
    fn value_type() -> ValueType {
        ValueType::Int(Range::from($int::MIN..=$int::MAX))
    }
}
impl<const MIN: $int, const MAX: $int> AsValueType for $ranged<MIN, MAX> {
    fn value_type() -> ValueType {
        ValueType::Int(Range::from(MIN..=MAX))
    }
}

impl TryFrom<Value> for $int {
    type Error = ValueReadError;
    fn try_from(value: Value) -> Result<Self, Self::Error> {
        let value32: i64 = value.try_into()?;
        value32.try_into().map_err(|_| {
            ValueReadError::IntNotInRange {
                value: value32,
                range: Range::from($int::MIN..=$int::MAX),
            }
        })
    }
}

impl From<$int> for Value {
    fn from(value: $int) -> Self {
        Value::Int(value as i64)
    }
}

impl<const MIN: $int, const MAX: $int> From<$ranged<MIN, MAX>> for Value {
    fn from(value: $ranged<MIN, MAX>) -> Self {
        value.inner().into()
    }
}

impl<const MIN: $int, const MAX: $int> TryFrom<Value> for $ranged<MIN, MAX> {
    type Error = ValueReadError;
    fn try_from(value: Value) -> Result<Self, Self::Error> {
        let value: $int = value.try_into()?;
        Self::new_try(value).ok_or_else(|| {
            ValueReadError::IntNotInRange {
                value: value as i64,
                range: Range::from(MIN..=MAX),
            }
        })
    }
}
)*
    };
}

impl_int!(
    i8: RangedI8,
    i16: RangedI16,
    i32: RangedI32,
    u8: RangedU8,
    u16: RangedU16,
    u32: RangedU32
);

impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Value::Float(value)
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Value::String(value)
    }
}
