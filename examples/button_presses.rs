use home_control::zigbee::devices::philips::HueSmartButton;
use home_control::Sensor;
use log::Level;
use rumqttc::MqttOptions;
use simple_log::LogConfigBuilder;
use std::time::Duration;
use tokio_stream::StreamExt;
use control::StreamCustomExt;

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

    let mut worker = zigbee::Manager::new();
    worker.set_mqtt_options(mqttoptions);
    let button: HueSmartButton = worker.add_device("test_button");
    worker.start();

    let button_events = button.events();
    let mut event_stream = button_events.subscribe().count_presses::<5>();
    while let Some(event) = event_stream.next().await {
        println!("{event:?}")
    }
}
