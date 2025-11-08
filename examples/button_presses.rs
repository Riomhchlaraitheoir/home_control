use control::StreamCustomExt;
use home_control::automation::Automation;
use home_control::zigbee::devices::philips::HueSmartButton;
use home_control::{Manager, Sensor};
use log::Level;
use rumqttc::MqttOptions;
use simple_log::LogConfigBuilder;
use std::time::Duration;

#[tokio::main]
async fn main() {
    simple_log::new(
        LogConfigBuilder::builder()
            .level(Level::Warn)
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
    let event_stream = button.events().subscribe().count_presses::<5>();

    let automation = Automation::parallel(event_stream, async |event| {
        println!("{event:?}");
        Ok(())
    });
    manager.start([automation]);
}
