use control::{ButtonEvent, Sensor, ToggleValue};
use futures::executor::{block_on, block_on_stream};
use home_control::Manager;
use home_control::automation::Automation;
use home_control::zigbee::devices::philips::{HueSmartButton, Light};
use log::{Level, debug};
use macros::DeviceSet;
use rumqttc::MqttOptions;
use simple_log::LogConfigBuilder;
use std::thread;
use std::thread::sleep;
use std::time::Duration;
use testing::{mock_philips_button, mock_philips_light, start_mqtt_broker};
use tokio_stream::StreamExt;

#[tokio::test]
async fn test_automation() {
    simple_log::new(
        LogConfigBuilder::builder()
            .level(Level::Debug)
            .unwrap()
            .output_console()
            .build(),
    )
    .expect("failed to start logger");
    let (conn, _guard) = start_mqtt_broker();

    let mut mock_button = mock_philips_button(&conn, "test_button").await;
    let mock_light = mock_philips_light(&conn, "test_light", true, 254).await;

    let mut mqttoptions = MqttOptions::new("rumqtt-sync", "localhost", 1883);
    mqttoptions.set_keep_alive(Duration::from_secs(5));

    let mut manager = Manager::new();
    manager.zigbee.set_mqtt_options(mqttoptions);
    let devices: Devices = manager.create().unwrap();
    // let automation =
    //     toggle_light_on_button(devices.test_button.events(), devices.test_light.state());
    thread::scope(move |scope| {
        scope.spawn(move || {
            // sleep(Duration::from_millis(50));
            // assert!(mock_light.state());
            loop {
                block_on(mock_button.action("off"));
                sleep(Duration::from_secs(2));
                block_on(mock_button.action("off"));
                sleep(Duration::from_secs(2));
            }
            // assert!(!mock_light.state());
            // block_on(mock_button.action("on"));
            // assert!(mock_light.state());
        });
        scope.spawn(move || {
            for event in block_on_stream(devices.test_button.events().subscribe()) {
                println!("Button event: {:?}", event);
            }
        });
        scope.spawn(move || {
            let manager = manager;
            manager.start([]);
        });
    });
}

#[derive(DeviceSet)]
struct Devices {
    test_button: HueSmartButton,
    test_light: Light,
}

fn toggle_light_on_button<'a>(
    button: &'a impl Sensor<Item = ButtonEvent>,
    light: &'a (impl ToggleValue + Send + Sync),
) -> Automation<'a> {
    let button_presses = button.subscribe().filter(|event| {
        debug!("received button event: {event:?}");
        *event == ButtonEvent::Press
    });
    Automation::parallel(button_presses, async |_| {
        light
            .toggle()
            .await
            .map_err(|err| format!("failed to toggle light: {err}"))
    })
}
