use std::convert::Infallible;
use crate::components::Status;
use crate::{Client, Toast};
use api::trait_rpc::RpcError;
use api::trait_rpc::stream::client::StreamError;
use api::{Device, Field, OperationError, ValueType};
use futures::stream::once;
use futures::{Stream, StreamExt};
use iced::alignment::Vertical;
use iced::font::Weight;
use iced::widget::{button, column, combo_box, row, stack, text};
use iced::{Element, Font, Task};
use iced_aw::Spinner;
use std::future::ready;
use std::pin::Pin;

#[derive(Debug)]
pub struct ViewDevice {
    device: Device,
    field_values: Vec<FieldState>,
}

#[derive(Debug)]
enum Value<T = api::Value> {
    /// The value is loading
    Loading,
    /// The value cannot be loaded right now (eg: subscribe-only value)
    Unknown,
    /// The value is loaded
    Loaded(T),
}

impl Value<Infallible> {
    fn into<T>(self) -> Value<T> {
        match self {
            Value::Loading => Value::Loading,
            Value::Unknown => Value::Unknown,
            Value::Loaded(value) => match value {},
        }
    }
}

impl<T> Value<T> {
    fn as_ref(&self) -> Value<&T> {
        match self {
            Value::Loading => Value::Loading,
            Value::Unknown => Value::Unknown,
            Value::Loaded(value) => Value::Loaded(value),
        }
    }

    fn map<R>(self, map: impl FnOnce(T) -> R) -> Value<R> {
        match self {
            Value::Loading => Value::Loading,
            Value::Unknown => Value::Unknown,
            Value::Loaded(value) => Value::Loaded(map(value)),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    Loaded(usize, api::Value),
    SetUpdated(usize, SetValue),
    Submit(usize),
    LoadFailure(String, String),
}

#[derive(Debug, Clone)]
pub enum SetValue {
    Bool(bool),
    Int(String)
}

impl SetValue {
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Bool(_) => "bool",
            Self::Int(_) => "int",
        }
    }
}

#[must_use]
pub enum Action {
    None,
    Task(Task<Message>),
    Toast(Toast),
}

impl ViewDevice {
    pub fn new(client: &Client, device: Device) -> (Self, Task<Message>) {
        let (field_values, tasks): (Vec<_>, Vec<_>) = device
            .fields
            .iter()
            .enumerate()
            .map(|(i, field)| {
                let device_id = device.id.clone();
                let field_name = field.name.clone();
                let mut value: Value<Infallible> = Value::Loading;
                let task = match (field.operations.get, field.operations.subscribe) {
                    (true, true) => {
                        Task::stream(get_and_subscribe(client.clone(), device_id, field_name, i))
                    }
                    (true, false) => Task::future(get(client.clone(), device_id, field_name, i)),
                    (false, true) => {
                        Task::stream(subscribe(client.clone(), device_id, field_name, i))
                    }
                    (false, false) => {
                        value = Value::Unknown;
                        Task::none()
                    }
                };
                let state = if field.operations.set {
                    match field.value_type {
                        ValueType::Bool => FieldState::Bool {
                            combo_box: combo_box::State::new(vec![false, true]),
                            value: value.into(),
                            set_value: None,
                        },
                        ValueType::Int(range) => FieldState::Int {
                            value: value.into(),
                            set_value: None,
                            range,
                            warning: None
                        },
                        ValueType::Float => todo!(),
                        ValueType::String { .. } => todo!(),
                        ValueType::Optional(_) => todo!(),
                    }
                } else {
                    FieldState::NoSet {
                        value: value.into(),
                    }
                };
                (state, task)
            })
            .unzip();
        let task = Task::batch(tasks);
        (
            Self {
                device,
                field_values,
            },
            task,
        )
    }

    pub fn view(&self) -> Element<'_, Message> {
        let device = &self.device;
        column![
            row![
                text(&device.name).size(25),
                text!("({})", &device.id)
                    .align_y(Vertical::Bottom)
                    .height(25)
            ]
            .spacing(15),
            device.description.as_deref().unwrap_or_default(),
            field_list(&device.fields, &self.field_values),
        ]
        .spacing(20)
        .into()
    }

    pub fn update(&mut self, message: Message) -> Action {
        match message {
            Message::Loaded(index, value) => self.field_values[index].update(value),
            Message::LoadFailure(field_name, error) => Action::Toast(Toast {
                title: format!("Failed to get value for {field_name}"),
                body: error,
                status: Status::Warning,
            }),
            Message::SetUpdated(index, value) => self.field_values[index].update_set(value),
            Message::Submit(index) => Action::Task(self.field_values[index].set_task())
        }
    }
}

fn field_list<'a>(fields: &'a [Field], values: &'a [FieldState]) -> Element<'a, Message> {
    use iced::widget::{table, table::column};
    let header = Font {
        weight: Weight::Bold,
        ..Font::default()
    };
    let name = column(
        text("Name").font(header),
        |(_, field): (usize, &'a Field)| field.name.as_str(),
    );
    let desc = column(
        text("Description").font(header),
        |(_, field): (usize, &'a Field)| field.description.as_str(),
    );
    let ty = column(
        text("Type").font(header),
        |(_, field): (usize, &'a Field)| text(field.value_type.to_string()),
    );
    let value = column(
        text("Value").font(header),
        |(i, _): (usize, &'a Field)| -> Element<Message> {
            match &values[i].value() {
                Value::Loaded(value) => text(value.to_string()).into(),
                Value::Loading => Spinner::new().into(),
                Value::Unknown => text("Unknown".to_string()).into(),
            }
        },
    );
    let set = column(
        text("Set").font(header),
        |(i, field): (usize, &'a Field)| -> Element<Message> {
            if !field.operations.set {
                return stack![].into();
            }
            value_setter(&values[i], i)
        },
    );
    table([name, desc, ty, value, set], fields.iter().enumerate()).into()
}

impl Message {
    fn from_result(
        result: Result<Result<api::Value, OperationError>, RpcError<StreamError>>,
        field_name: &str,
        field_index: usize,
    ) -> Self {
        match result {
            Ok(Ok(value)) => Message::Loaded(field_index, value),
            Ok(Err(error)) => Message::LoadFailure(field_name.to_string(), error.to_string()),
            Err(error) => Message::LoadFailure(field_name.to_string(), error.to_string()),
        }
    }
}

async fn get(client: Client, device_id: String, field_name: String, field_index: usize) -> Message {
    let result = client
        .device(device_id)
        .field(field_name.clone())
        .get()
        .await;
    Message::from_result(result, &field_name, field_index)
}

fn subscribe(
    client: Client,
    device_id: String,
    field_name: String,
    field_index: usize,
) -> impl Stream<Item = Message> {
    once({
        let field_name = field_name.clone();
        async move {
            client
                .device(device_id)
                .field(field_name.clone())
                .subscribe()
                .await
        }
    })
    .flat_map(move |result| match result {
        Ok(value) => Box::pin(value.map({
            let field_name = field_name.clone();
            move |result| Message::from_result(result, &field_name, field_index)
        })) as Pin<Box<dyn Stream<Item = Message> + Send>>,
        Err(error) => Box::pin(once(ready(Message::LoadFailure(
            field_name.clone(),
            error.to_string(),
        )))),
    })
}

fn get_and_subscribe(
    client: Client,
    device_id: String,
    field_name: String,
    field_index: usize,
) -> impl Stream<Item = Message> {
    once({
        let field_name = field_name.clone();
        async move {
            client
                .device(device_id)
                .field(field_name.clone())
                .get_and_subscribe()
                .await
        }
    })
    .flat_map(move |result| match result {
        Ok(value) => Box::pin(value.map({
            let field_name = field_name.clone();
            move |result| Message::from_result(result, &field_name, field_index)
        })) as Pin<Box<dyn Stream<Item = Message> + Send>>,
        Err(error) => Box::pin(once(ready(Message::LoadFailure(
            field_name.clone(),
            error.to_string(),
        )))),
    })
}

#[derive(Debug)]
enum FieldState {
    Bool {
        value: Value<bool>,
        combo_box: combo_box::State<bool>,
        set_value: Option<bool>,
    },
    Int {
        value: Value<i64>,
        set_value: Option<String>,
        range: api::Range<i64>,
        warning: Option<String>,
    },
    NoSet {
        value: Value,
    },
}

impl FieldState {
    fn value_setter(&self, field_index: usize) -> Element<'_, Message> {
        let input: Element<Message> = match self {
            FieldState::Bool {
                combo_box: state,
                set_value,
                ..
            } => combo_box(state, "", set_value.as_ref(), move |value| {
                Message::SetUpdated(field_index, SetValue::Bool(value))
            })
                .into(),
            FieldState::Int { set_value, .. } => iced::widget::text_input( // TODO: show warning
                "",
                set_value
                    .as_deref()
                    .unwrap_or_default(),
            ).on_input(move |value| {
                Message::SetUpdated(field_index, SetValue::Int(value))
            }).on_submit(Message::Submit(field_index))
                .into(),
            FieldState::NoSet { .. } => stack![].into(),
        };
        row![input, button("Set").on_press(Message::Submit(field_index))].into()
    }

    fn update(&mut self, new_value: api::Value) -> Action {
        match self {
            FieldState::Bool { value, .. } => {
                let api::Value::Bool(new_value) = new_value else {
                    return Action::Toast(Toast {
                        title: "Received wrong vlaue type from server".to_string(),
                        body: format!("Expected bool, but received: {}", new_value.type_name()),
                        status: Status::Warning,
                    });
                };
                *value = Value::Loaded(new_value);
            }
            FieldState::Int { value, .. } => {
                let api::Value::Int(new_value) = new_value else {
                    return Action::Toast(Toast {
                        title: "Received wrong value type from server".to_string(),
                        body: format!("Expected int, but received: {}", new_value.type_name()),
                        status: Status::Warning,
                    });
                };
                *value = Value::Loaded(new_value);
            }
            FieldState::NoSet { .. } => {}
        }
        Action::None
    }

    fn update_set(&mut self, value: SetValue) -> Action {
        match self {
            FieldState::Bool { set_value, .. } => {
                let SetValue::Bool(value) = value else {
                    return Action::Toast(Toast {
                        title: "Received wrong value type from ui".to_string(),
                        body: format!("Expected bool, but received: {}", value.type_name()),
                        status: Status::Warning,
                    });
                };
                *set_value = Some(value)
            }
            FieldState::Int { set_value, range, warning, .. } => {
                let SetValue::Int(value) = value else {
                    return Action::Toast(Toast {
                        title: "Received wrong value type from ui".to_string(),
                        body: format!("Expected int, but received: {}", value.type_name()),
                        status: Status::Warning,
                    });
                };
                if let Ok(int_value) = value.parse::<i64>() {
                    if !range.contains(&int_value) {
                        *warning = Some(format!("value outside range: {range}"))
                    }
                    *set_value = Some(value)
                }
            }
            FieldState::NoSet { .. } => {}
        }
        Action::None
    }

    fn value(&self) -> Value<String> {
        match self {
            FieldState::Bool { value, .. } => value.as_ref().map(|value| value.to_string()),
            FieldState::Int { value, .. } => value.as_ref().map(|value| value.to_string()),
            FieldState::NoSet { value, .. } => value.as_ref().map(|value| value.to_string()),
        }
    }

    fn set_task(&mut self) -> Task<Message> {
        todo!()
    }
}
