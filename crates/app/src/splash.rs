use crate::{Client, MainMessage};
use api::{
    trait_rpc::{
        client::{
            self,
            websocket::{new_websocket_transport, WebsocketError},
        },
        format::json::Json,
        Rpc,
    },
    Api,
};
use iced::border::Radius;
use iced::widget::{column, container, text};
use iced::{widget::{button, row, text_input}, Border, Element, Fill, Task};
use crate::config::Config;

#[derive(Debug)]
pub struct Splash {
    server: String,
    error: Option<String>,
}

#[derive(Debug, Clone)]
pub enum SplashMessage {
    ServerInput(String),
    TryConnect,
    ConnectResult(Result<Client, String>)
}

pub enum Action {
    None,
    Task(Task<SplashMessage>),
    ConnectionSuccess(Client),
}

impl From<SplashMessage> for MainMessage {
    fn from(value: SplashMessage) -> Self {
        Self::Splash(value)
    }
}

impl Splash {
    pub fn new(server: String) -> Self {
        Self {
            server,
            error: None,
        }
    }

    pub fn view(&self) -> Element<'_, SplashMessage> {
        let server_field = text_input("http://server.com:443", &self.server)
            .on_input(SplashMessage::ServerInput)
            .on_submit(SplashMessage::TryConnect);
        let connect_button = button("connect").on_press(SplashMessage::TryConnect);
        let input = row![server_field, connect_button].width(500);
        let mut col = column![input];
        if let Some(error) = &self.error {
            let error = container(text!("Connection Error: {error}")).style(|theme: &iced::Theme| {
                let color = theme.extended_palette().danger.weak;
                container::Style {
                    background: Some(color.color.into()),
                    text_color: Some(color.text),
                    border: Border {
                        color: theme.extended_palette().danger.strong.color,
                        width: 1.0,
                        radius: Radius::new(5.0),
                    },
                    ..container::Style::default()
                }
            }).padding(8);
            col = col.push(error);
        }
        container(col)
            .center_x(Fill)
            .center_y(Fill)
            .into()
    }

    pub fn update(&mut self, config: &mut Config, message: SplashMessage) -> Action {
        match message {
            SplashMessage::ServerInput(s) => {
                self.server = s;
                Action::None
            }
            SplashMessage::TryConnect => {
                let connect = Task::perform(
                    tokio::spawn(Self::try_connect(self.server.clone())),
                    |result| {
                        SplashMessage::ConnectResult(match result {
                            Ok(Ok(client)) => Ok(client),
                            Ok(Err(error)) => Err(error.to_string()),
                            Err(error) => Err(format!("failed to join task: {error}")),
                        })
                    },
                );
                Action::Task(connect)
            }
            SplashMessage::ConnectResult(result) => match result {
                Ok(client) => {
                    config.server = Some(self.server.clone());
                    Action::ConnectionSuccess(client)
                },
                Err(error) => {
                    self.error = Some(error);
                    Action::None
                }
            },
        }
    }

    async fn try_connect(url: String) -> Result<Client, WebsocketError> {
        let url = if url.ends_with('/') {
            format!("{url}api")
        } else {
            format!("{url}/api")
        };
        let url = if let Some(stripped) = url.strip_prefix("http:") {
            format!("ws:{}", stripped)
        } else if let Some(stripped) = url.strip_prefix("https:") {
            format!("wss:{}", stripped)
        } else {
            url.to_string()
        };
        Ok(Api::async_client(
            client::builder()
                .non_blocking()
                .format(&Json)
                .transport(new_websocket_transport(url, Json).await?)
                .build(),
        ))
    }
}
