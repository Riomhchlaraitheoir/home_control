use iced::{Element, Task};

use super::Action;
use crate::devices::view::ViewDevice;
use crate::{devices::list::DeviceList, Client};

mod list;
mod view;

#[derive(Debug)]
pub enum Devices {
    List(DeviceList),
    View(ViewDevice)
}

#[derive(Debug, Clone)]
pub enum Message {
    List(list::Message),
    View(view::Message)
}

impl Devices {
    pub fn list(client: &Client) -> (Devices, Task<Message>) {
        let (list, task) = DeviceList::new(client);
        (Self::List(list), task.map(Message::List))
    }
}

impl Devices {
    pub fn view(&self) -> Element<'_, Message> {
        match self {
            Devices::List(device_list) => device_list.view().map(Message::List),
            Devices::View(device_view) => device_view.view().map(Message::View)
        }
    }

    pub fn update(&mut self, client: &Client, message: Message) -> Action<Message, Self> {
        match message {
            Message::List(message) => {
                let Self::List(list) = self else {
                    return Action::None
                };
                match list.update(client, message) {
                    list::Action::None => Action::None,
                    list::Action::ViewDevice(device) => {
                        let (view, task) = ViewDevice::new(client, device);
                        Action::Navigate(Self::View(view), task.map(Message::View))
                    }
                }
            }
            Message::View(message) => {
                let Self::View(view) = self else {
                    return Action::None
                };
                match view.update(message) {
                    view::Action::None => {
                        Action::None
                    }
                    view::Action::Task(task) => {
                        Action::Task(task.map(Message::View))
                    }
                    view::Action::Toast(toast) => {
                        Action::Toast(toast)
                    }
                }
            }
        }
    }
}