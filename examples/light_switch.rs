use home_control::zigbee::devices::philips::{HueSmartButton, Light};
use home_control::{ButtonEvent, Manager, Sensor, ToggleValue};
use log::{debug, Level};
use rumqttc::MqttOptions;
use simple_log::LogConfigBuilder;
use std::time::Duration;
use tokio_stream::StreamExt;

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

    let mut manager = Manager::new();
    manager.zigbee.set_mqtt_options(mqttoptions);
    let button: HueSmartButton = manager.add_device("test_button".to_string()).unwrap();
    let light: Light = manager.add_device("office_light".to_string()).unwrap();
    manager.start();

    toggle_light_on_button(button.events(), light.state()).await;
}

async fn toggle_light_on_button(
    button: &impl Sensor<Item = ButtonEvent>,
    light: &impl ToggleValue,
) {
    let mut button_presses = button.subscribe().filter(|event| {
        debug!("received button event: {event:?}");
        *event == ButtonEvent::Press
    });
    while button_presses.next().await.is_some() {
        light.toggle().await.expect("failed to toggle light")
    }
}
