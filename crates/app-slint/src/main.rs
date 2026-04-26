mod controller;
mod config;

use log::Level;
use slint::ComponentHandle;
use api::trait_rpc::client::SimpleClient;
use api::trait_rpc::stream::client::StreamClient;
use api::trait_rpc::format::json::Json;
use api::{Api, ApiAsyncClient};
use api::trait_rpc::{client, Rpc};
use api::trait_rpc::client::websocket::{new_websocket_transport, WebsocketError};
use crate::config::Config;
use crate::ui::{App, SavedConfig};

pub mod ui {
    slint::include_modules!();
}

type Client = ApiAsyncClient<SimpleClient<Json, StreamClient>>;

fn main() {
    simple_logger::init_with_level(Level::Info).unwrap();
    let app = App::new().expect("Failed to create splash page");
    let config = smol::spawn(Config::load());
    controller::setup(&app);
    let config = smol::block_on(config);
    app.set_config(SavedConfig {
        server: config.server.unwrap_or_default().into()
    });
    app.run().expect("Failed to run splash application");
}

pub async fn connect_client(url: &str) -> Result<Client, WebsocketError> {
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
            .transport(
                new_websocket_transport(url, Json).await?,
            )
            .build()
    ))
}
