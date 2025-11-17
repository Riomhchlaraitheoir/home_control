#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, reason = "Panics are forgivable while testing")]
//! A working test which mocks some simple devices
//!
//! This test is designed to ensure that automations are triggered and running properly in the general case

use control::{ButtonEvent, Sensor, ToggleValue};
use home_control::automation::Automation;
use home_control::zigbee::devices::philips::{HueSmartButton, Light};
use log::{Level, debug};
use macros::DeviceSet;
use rumqttc::MqttOptions;
use simple_log::LogConfigBuilder;
use std::time::Duration;
use async_scoped::TokioScope;
use tokio::time::sleep;
use testing::{mock_philips_button, mock_philips_light, start_mqtt_broker};
use tokio_stream::StreamExt;
use control::manager::Manager;

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

    let mut manager = Manager::builder()
        .add_device_manager(zigbee::Manager::builder()
            .mqtt_options(mqttoptions)
            .build())
        .build();
    let devices: Devices = manager.create().await.unwrap();
    let automation =
        toggle_light_on_button(devices.test_button.events(), devices.test_light.state());
    TokioScope::scope_and_block(|scope| {
        scope.spawn(async move {
            sleep(Duration::from_millis(50)).await;
            assert!(mock_light.state());
            mock_button.action("off").await;
            assert!(!mock_light.state());
            mock_button.action("on").await;
            assert!(mock_light.state());
        });
        scope.spawn(async move {
            let manager = manager;
            manager.start([automation]).await;
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
    Automation::new("test", button_presses, async |_| {
        light
            .toggle()
            .await
            .map_err(|err| format!("failed to toggle light: {err}"))
    })
}
