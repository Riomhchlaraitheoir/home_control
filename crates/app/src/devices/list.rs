use crate::Client;
use api::Device;
use futures::future::ready;
use futures::stream::{once, StreamExt};
use iced::widget::container::Style;
use iced::widget::{column, container, mouse_area, row, scrollable, text};
use iced::{Border, Color, Element, Task};

#[derive(Debug)]
pub struct DeviceList {
    devices: Vec<Device>,
    errors: Vec<String>,
    loading: bool,
}

#[derive(Debug, Clone)]
pub enum Message {
    DeviceReceived(Device),
    RequestError(String),
    StreamError(String),
    FinishedLoading,
    ViewDevice(Device),
}

#[must_use]
pub enum Action {
    None,
    ViewDevice(Device),
}

impl DeviceList {
    pub fn new(client: &Client) -> (Self, Task<Message>) {
        let fetch = Task::future({
            let client = client.clone();
            async move { client.get_devices().await }
        })
        .then(|result| match result {
            Ok(stream) => Task::stream(
                stream
                    .map(|result| match result {
                        Ok(device) => Message::DeviceReceived(device),
                        Err(error) => Message::StreamError(error.to_string()),
                    })
                    .chain(once(ready(Message::FinishedLoading))),
            ),
            Err(error) => Task::done(Message::RequestError(error.to_string())),
        });
        (
            Self {
                devices: Vec::new(),
                errors: Vec::new(),
                loading: true,
            },
            fetch,
        )
    }

    pub fn view(&self) -> Element<'_, Message> {
        let mut col = column(self.devices.iter().map(|device| {
            container(
                mouse_area(device_card_contents(device))
                    .on_press(Message::ViewDevice(device.clone())),
            )
            .style(|_| Style {
                border: Border::default().color(Color::BLACK).width(3).rounded(15),
                shadow: Default::default(),
                ..Style::default()
            })
            .padding(8)
            .into()
        }));
        if self.loading {
            col = col.push(iced::widget::text("loading..."));
        }
        scrollable(col).into()
    }

    pub fn update(&mut self, _client: &Client, message: Message) -> Action {
        match message {
            Message::DeviceReceived(device) => {
                self.devices.push(device);
            }
            Message::RequestError(error) | Message::StreamError(error) => self.errors.push(error),
            Message::FinishedLoading => {
                self.loading = false;
            }
            Message::ViewDevice(device) => {
                return Action::ViewDevice(device);
            }
        }
        Action::None
    }
}

fn device_card_contents<'a>(device: &'a Device) -> impl Into<Element<'a, Message>> {
    column![
        row![text("Name: "), text(&device.name)],
        row![text("ID: "), text(&device.id)],
        row![
            text("Description: "),
            text(device.description.as_deref().unwrap_or_default())
        ]
    ]
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use crate::devices::list::DeviceList;
    use crate::devices::Devices;
    use crate::{App, Page};
    use api::{Device, DeviceType};
    use std::collections::VecDeque;

    #[test]
    fn test_list() {
        let list = DeviceList {
            devices: vec![
                Device {
                    id: "office_light".to_string(),
                    name: "Office Light".to_string(),
                    description: Some("The light in the office".to_string()),
                    tags: Default::default(),
                    device_type: DeviceType::Light,
                    fields: vec![],
                },
                Device {
                    id: "office_button".to_string(),
                    name: "Office Button".to_string(),
                    description: Some("The button in the office".to_string()),
                    tags: Default::default(),
                    device_type: DeviceType::Switch,
                    fields: vec![],
                },
            ],
            errors: vec![],
            loading: false,
        };
        let app = App {
            page: Some(Page::Devices(Devices::List(list))),
            stack: VecDeque::new(),
            toasts: Vec::new(),
        };
        let mut ui = iced_test::simulator(app.view());
        let snapshot = ui
            .snapshot(&iced::Theme::Light)
            .expect("failed to generate snapshot");
        assert!(
            snapshot
                .matches_image("snapshots/devices/list.png")
                .expect("does not match previous snapshot")
        );
    }
}
