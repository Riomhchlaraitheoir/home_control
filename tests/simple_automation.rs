use home_control::zigbee::devices::philips::{HueSmartButton, Light};
use home_control::{ButtonEvent, Manager, Sensor, ToggleValue};
use log::{debug, Level};
use rumqttc::MqttOptions;
use simple_log::LogConfigBuilder;
use std::time::Duration;
use tokio::spawn;
use tokio::time::sleep;
use tokio_stream::StreamExt;
use control::automations::{single, Automation};
use macros::DeviceSet;
use testing::{mock_philips_button, mock_philips_light, start_mqtt_broker};

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
    spawn(async move {
        let mut automations = toggle_light_on_button(devices.test_button.events(), devices.test_light.state());
        manager.start(&mut automations).await
    });
    sleep(Duration::from_millis(50)).await;
    assert!(mock_light.state());
    mock_button.action("off").await;
    assert!(!mock_light.state());
    mock_button.action("on").await;
    assert!(mock_light.state());
}

#[derive(DeviceSet)]
struct Devices {
    test_button: HueSmartButton,
    test_light: Light,
}

fn toggle_light_on_button(
    button: &impl Sensor<Item = ButtonEvent>,
    light: &(impl ToggleValue + Send + Sync),
) -> impl Automation {
    let button_presses = button.subscribe().filter(|event| {
        debug!("received button event: {event:?}");
        *event == ButtonEvent::Press
    });
    single("switch_light".to_string(), button_presses, async |_| {
        light.toggle().await.map_err(|err| format!("failed to toggle light: {err}"))
    })
}
