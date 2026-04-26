use crate::ui::{
    App, Device, Field, Operations, RangeBound, RangeBoundType, ValueKind,
    ValueType,
};
use slint::SharedString;

mod list;
mod view;
mod fields;

pub fn connect_device_controllers(app: &App) {
    list::connect_device_list_controller(app);
    view::connect_device_view_controller(app);
    fields::connect_field_controller(app);
}

impl From<api::Device> for Device {
    fn from(device: api::Device) -> Self {
        Self {
            id: device.id.into(),
            name: device.name.into(),
            description: device.description.unwrap_or_default().into(),
            fields: device
                .fields
                .into_iter()
                .map(Into::into)
                .collect::<Vec<_>>()
                .as_slice()
                .into(),
        }
    }
}

impl From<api::Field> for Field {
    fn from(value: api::Field) -> Self {
        Self {
            description: value.description.into(),
            name: value.name.into(),
            operations: value.operations.into(),
            value_type: value.value_type.into(),
        }
    }
}

impl From<api::Operations> for Operations {
    fn from(value: api::Operations) -> Self {
        Self {
            get: value.get,
            set: value.set,
            subscribe: value.subscribe,
            toggle: value.toggle,
        }
    }
}

impl From<api::ValueType> for ValueType {
    fn from(mut value: api::ValueType) -> Self {
        let mut value_type = Self::default();
        loop {
            match value {
                api::ValueType::Bool => {
                    value_type.kind = ValueKind::Bool;
                }
                api::ValueType::Int(range) => {
                    value_type.int_range.0 = range.start.into();
                    value_type.int_range.1 = range.end.into();
                }
                api::ValueType::Float => {
                    value_type.kind = ValueKind::Float;
                }
                api::ValueType::String { values } => {
                    value_type.kind = ValueKind::String;
                    value_type.string_values = values
                        .unwrap_or_default()
                        .into_iter()
                        .map(SharedString::from)
                        .collect::<Vec<_>>()
                        .as_slice()
                        .into()
                }
                api::ValueType::Optional(t) => {
                    value_type.optional = true;
                    value = *t;
                    continue;
                }
            }
            break value_type;
        }
    }
}

impl From<api::RangeBound<i64>> for RangeBound {
    fn from(value: api::RangeBound<i64>) -> Self {
        match value {
            api::RangeBound::Included(value) => Self {
                r#type: RangeBoundType::Included,
                value: value as i32,
            },
            api::RangeBound::Excluded(value) => Self {
                r#type: RangeBoundType::Excluded,
                value: value as i32,
            },
            api::RangeBound::Open => Self {
                r#type: RangeBoundType::Open,
                value: 0,
            },
        }
    }
}
