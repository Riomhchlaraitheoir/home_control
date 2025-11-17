#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, reason = "Panics are forgivable while testing")]
//! A simple example showing how a lightswitch could be implemented

use control::{ButtonEvent, Sensor, ToggleValue};
use home_control::automation::Automation;
use home_control::zigbee::devices::philips::{HueSmartButton, Light};
use log::{Level, debug};
use rumqttc::MqttOptions;
use simple_log::LogConfigBuilder;
use std::time::Duration;
use tokio_stream::StreamExt;
use control::manager::Manager;

#[tokio::main]
async fn main() {
    simple_log::new(
        LogConfigBuilder::builder()
            .level(Level::Debug)
            .unwrap()
            .output_console()
            .build(),
    )
    .expect("failed to start logger");
    let mut mqttoptions = MqttOptions::new("rumqtt-sync", "localhost", 1883);
    mqttoptions.set_keep_alive(Duration::from_secs(5));

    let mut manager = Manager::builder()
        .add_device_manager(zigbee::Manager::builder()
            .mqtt_options(mqttoptions)
            .build())
        .build();
    let button: HueSmartButton = manager.add_device("test_button".to_string()).await.unwrap();
    let light: Light = manager.add_device("office_light".to_string()).await.unwrap();
    let automation = toggle_light_on_button(button.events(), light.state());
    manager.start([automation]).await;
}

fn toggle_light_on_button<'a>(
    button: &'a impl Sensor<Item = ButtonEvent>,
    light: &'a (impl ToggleValue + Send + Sync),
) -> Automation<'a> {
    let button_presses = button.subscribe().filter(|event| {
        debug!("received button event: {event:?}");
        *event == ButtonEvent::Press
    });
    Automation::new("test", button_presses, async |_| {
        light
            .toggle()
            .await
            .map_err(|err| format!("failed to toggle light: {err}"))
    })
}
