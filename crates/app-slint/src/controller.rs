mod devices;

use crate::config::Config;
use crate::controller::devices::connect_device_controllers;
use crate::{App, Client, connect_client};
use async_compat::Compat;
use log::{debug, error, info};
use slint::{ComponentHandle, invoke_from_event_loop};
use std::cell::RefCell;

thread_local! {
    /// This should only be accessed from the main thread, keeping it private from rest of crate to help achieve that
    static CLIENT: RefCell<Option<Client>> = const { RefCell::new(None) };
    static CONFIG: RefCell<Config> = RefCell::default();
}

pub fn setup(app: &App) {
    app.on_connect({
        let app = app.as_weak();
        move |server| {
            let app = app.clone();
            smol::spawn(Compat::new(async move {
                let url = server.to_string();
                let result = connect_client(&url).await;
                invoke_from_event_loop(move || {
                    match result {
                        Ok(client) => {
                            CLIENT.set(Some(client));
                            CONFIG.with_borrow_mut(|config| {
                                // Don't try to update config until we have loaded it
                                if config.is_loaded() {
                                    info!("adding server = {server:?} to config");
                                    config.server = Some(server.to_string());
                                    let config = config.clone();
                                    smol::spawn(async move {
                                        if let Err(error) = config.save().await {
                                            error!("Failed to save config: {error}")
                                        }
                                    })
                                    .detach();
                                } else {
                                    debug!("config not yet loaded, not updating server")
                                }
                            });
                            app.unwrap().invoke_connection_ready();
                        }
                        Err(error) => {
                            error!("Failed to connect to server: {error:?}");
                            app.unwrap().set_connection_error(
                                format!("Failed to connect to server: {error}").into(),
                            );
                        }
                    }
                })
                .unwrap();
            }))
            .detach();
        }
    });
    connect_device_controllers(app)
}
