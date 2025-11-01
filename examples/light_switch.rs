use home_control::zigbee::devices::philips::{HueSmartButton, Light};
use home_control::{ButtonEvent, Manager, Sensor, ToggleValue};
use log::{debug, Level};
use rumqttc::MqttOptions;
use simple_log::LogConfigBuilder;
use std::time::Duration;
use tokio_stream::StreamExt;
use control::automations::{single, Automation};

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
    let mut automations = toggle_light_on_button(button.events(), light.state());
    manager.start(&mut automations);
}

fn toggle_light_on_button(
    button: &impl Sensor<Item = ButtonEvent>,
    light: &(impl ToggleValue + Send + Sync),
) -> impl Automation {
    let button_presses = button.subscribe().filter(|event| {
        debug!("received button event: {event:?}");
        *event == ButtonEvent::Press
    });
    single(button_presses, async |_| {
        light.toggle().await.map_err(|err| format!("failed to toggle light: {err}"))
    })
}
