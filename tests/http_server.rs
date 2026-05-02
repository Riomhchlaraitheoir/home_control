#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, reason = "Panics are forgivable while testing")]
//! A working test which mocks some simple devices
//!
//! This test is designed to ensure that automations are triggered and running properly in the general case

use async_scoped::TokioScope;
use control::Manager;
use log::Level;
use rumqttc::MqttOptions;
use simple_log::LogConfigBuilder;
use std::time::Duration;
use testing::{mock_philips_button, mock_philips_light, start_mqtt_broker};
use tokio::time::sleep;
use tintean::web::axum::Router;
use web::axum::routing::get;
use web::WebServer;

#[tokio::test]
async fn http_server() {
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
    manager.add_service(
            WebServer::builder()
                .bind_address("0.0.0.0")
                .port(8088)
                .router(Router::new()
                    .route("/", get("Hello world")))
                .build()
        );
    // let devices: Devices = manager.create().await.unwrap();
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
            manager.start([]).await;
        });
    });
}

// #[derive(DeviceSet)]
// struct Devices {
//     test_button: HueSmartButton,
//     test_light: Light,
// }
