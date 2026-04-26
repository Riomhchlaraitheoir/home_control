#![allow(missing_docs)]

use crate::components::{Toast, ToastManager};
use crate::{
    components::AppBar,
    config::Config,
    devices::Devices,
    splash::{Splash, SplashMessage},
};
use api::{
    trait_rpc::{client::SimpleClient, format::json::Json, stream::client::StreamClient},
    ApiAsyncClient,
};
use derive_more::From;
use iced::{widget::{stack, Container}, Element, Length::Fill, Padding, Task};
use iced_aw::Spinner;
use log::{info, warn};
use std::collections::VecDeque;
use std::mem;

mod components;
mod config;
mod devices;
mod splash;

type Client = ApiAsyncClient<SimpleClient<Json, StreamClient>>;

fn main() -> Result<(), String> {
    simple_logger::init_with_level(log::Level::Info).map_err(|err| format!("Failed to init logger: {err}"))?;
    iced::application(Main::boot, Main::update, Main::view)
        .run()
        .map_err(|err| format!("Iced error: {err}"))?;
    Ok(())
}

#[derive(Debug)]
enum Main {
    Loading,
    Splash(Splash, Config),
    App {
        app: Box<App>,
        client: Client,
        #[allow(dead_code, reason = "Keeping it here in case it is needed in the future")]
        config: Config,
    },
}

#[derive(Debug, Clone)]
enum MainMessage {
    ConfigFileLoaded(Config),
    Splash(SplashMessage),
    App(Message),
    /// A message that indicates no action is needed, useful for tasks that do not result in UI updates
    NoAction,
}

impl Main {
    fn boot() -> (Self, Task<MainMessage>) {
        (
            Self::Loading,
            Task::perform(Config::load(), MainMessage::ConfigFileLoaded),
        )
    }

    fn update(&mut self, message: MainMessage) -> Task<MainMessage> {
        info!("Processing message: {:?}", message);
        match message {
            MainMessage::ConfigFileLoaded(config) => {
                *self = Self::Splash(
                    Splash::new(config.server.clone().unwrap_or_default()),
                    config,
                );
                info!("Updated state: {:?}", self);
                Task::none()
            }
            MainMessage::Splash(message) => {
                if let Self::Splash(splash, config) = self {
                    match splash.update(message) {
                        splash::Action::None => Task::none(),
                        splash::Action::Task(task) => task.map(MainMessage::Splash),
                        splash::Action::ConnectionSuccess(client) => {
                            let config = config.clone();
                            *self = Self::App {
                                app: Box::new(App {
                                    page: None,
                                    stack: VecDeque::new(),
                                    toasts: Vec::new(),
                                }),
                                client,
                                config: config.clone(),
                            };
                            info!("Updated state: {:?}", self);
                            Task::batch([
                                Task::done(MainMessage::App(Message::Init)),
                                Task::future(async move {
                                    let result = config.save().await;
                                    if let Err(error) = result {
                                        warn!("Failed to save config file: {error}");
                                    } else {
                                        info!("config file updated")
                                    }
                                    MainMessage::NoAction
                                }),
                            ])
                        }
                    }
                } else {
                    warn!("Received {message:?} while state: {self:?}");
                    info!("Updated state: {:?}", self);
                    Task::none()
                }
            }
            MainMessage::App(message) => {
                let Self::App {
                    app,
                    client,
                    config: _,
                } = self
                else {
                    info!("Updated state: {:?}", self);
                    return Task::none();
                };
                let task = app.update(client, message).map(MainMessage::App);
                info!("Updated state: {:?}", self);
                task
            }
            MainMessage::NoAction => Task::none(),
        }
    }

    pub fn view(&'_ self) -> iced::Element<'_, MainMessage> {
        match self {
            Main::Loading => Spinner::new().into(),
            Main::Splash(splash, _) => splash.view(),
            Main::App { app, client: _, config: _, } => app.view().map(MainMessage::App),
        }
    }
}

#[derive(Debug)]
struct App {
    page: Option<Page>,
    stack: VecDeque<Page>,
    toasts: Vec<Toast>,
}

impl App {
    fn view(&self) -> Element<'_, Message> {
        let page = match &self.page {
            None => stack!().into(),
            Some(page) => page.view(),
        };
        let back = if self.stack.is_empty() {
            None
        } else {
            Some(Message::Back)
        };
        let app_bar = AppBar::<Message>::new("Home Control").on_back_button_maybe(back);
        let page = Container::new(page)
            .padding(Padding::ZERO.top(app_bar.height()))
            .center(Fill);

        ToastManager::new(stack!(page, app_bar), &self.toasts, Message::CloseToast).into()
    }

    fn update(&mut self, client: &Client, message: Message) -> Task<Message> {
        let Some(page) = &mut self.page else {
            if !matches!(message, Message::Init) {
                return Task::none();
            }
            let (devices, task) = Devices::list(client);
            self.page = Some(Page::Devices(devices));
            return task.map(|msg| Message::Page(PageMessage::Devices(msg)));
        };
        let action: Action = match message {
            Message::Init => {
                return Task::none();
            }
            Message::Page(message) => page.update(client, message),
            Message::Back => {
                if let Some(previous) = self.stack.pop_back() {
                    *page = previous;
                }
                return Task::none();
            }
            Message::CloseToast(index) => {
                self.toasts.remove(index);
                return Task::none();
            }
        };
        match action {
            Action::None => Task::none(),
            Action::Task(task) => task,
            Action::Navigate(mut app, task) => {
                mem::swap(page, &mut app);
                self.stack.push_back(app);
                task
            }
            Action::Toast(toast) => {
                self.toasts.push(toast);
                Task::none()
            }
        }
    }
}

#[derive(Debug, From)]
enum Page {
    Devices(Devices),
}

impl Page {
    fn view(&self) -> Element<'_, Message> {
        match self {
            Page::Devices(devices) => devices.view().map(PageMessage::Devices),
        }
        .map(Message::Page)
    }

    fn update(&mut self, client: &Client, message: PageMessage) -> Action {
        match self {
            Page::Devices(devices) => {
                let PageMessage::Devices(message) = message/* else {
                    return Action::None;
                }*/;
                devices.update(client, message).into_base()
            }
        }
    }
}

#[derive(Debug, Clone)]
enum Message {
    Init,
    Page(PageMessage),
    Back,
    CloseToast(usize)
}

#[derive(Debug, Clone, From)]
enum PageMessage {
    Devices(devices::Message),
}

enum Action<M = Message, S = Page> {
    None,
    Task(Task<M>),
    Navigate(S, Task<M>),
    Toast(Toast),
}

impl<M, S> Action<M, S>
where
    M: Into<PageMessage> + Send + 'static,
    S: Into<Page>,
{
    fn into_base(self) -> Action<Message, Page> {
        match self {
            Self::None => Action::None,
            Self::Task(task) => Action::Task(task.map(Into::into).map(Message::Page)),
            Self::Navigate(app, task) => Action::Navigate(app.into(), task.map(Into::into).map(Message::Page)),
            Self::Toast(toast) => Action::Toast(toast),
        }
    }
}
