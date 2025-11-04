//! A crate with utilities useful for testing

use bytes::Bytes;
use futures::StreamExt;
use log::{debug, info, warn};
use rumqttc::{AsyncClient, Event, Incoming, MqttOptions, QoS};
use serde_json::{json, Value};
use std::pin::pin;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Arc;
use std::thread::{sleep};
use std::time::Duration;
use futures::executor::block_on;
use tokio::process::{Child, Command};
use tokio::spawn;
use tokio::sync::broadcast::Receiver;
use tokio::sync::mpsc::Sender;
use tokio::task::JoinHandle;
use tokio_stream::wrappers::BroadcastStream;

const CONFIG: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/rumqttd.test.toml");
const RUMQTTD: &str = env!("CARGO_BIN_FILE_RUMQTTD");

/// Start a local MQTT broker at `localhost:1883`
pub fn start_mqtt_broker() -> (Connection, CancelGuard) {
    let broker = Command::new(RUMQTTD)
        .args(["--config", CONFIG])
        .spawn()
        .expect("failed to start mqtt broker");

    // allow time for the broker to start
    sleep(Duration::from_secs(1));

    let (client, event_loop) =
        AsyncClient::new(MqttOptions::new("testing", "localhost", 1883), 10);
    let client = Arc::new(client);
    let (incoming_send, incoming_recv) = tokio::sync::broadcast::channel::<Publish>(10);
    let (outgoing_send, mut outgoing_recv) = tokio::sync::mpsc::channel::<Publish>(10);
    let incoming_job = spawn(async move {
        let events = futures::stream::unfold(event_loop, |mut event_loop| async {
            match event_loop.poll().await {
                Ok(event) => Some((event, event_loop)),
                Err(err) => {
                    warn!("Error from connection: {err}");
                    None
                }
            }
        });
        let mut events = pin!(events);
        while let Some(event) = events.next().await {
            let Event::Incoming(packet) = event else {
                continue;
            };
            debug!("Received incoming packet");

            let Incoming::Publish(publish) = packet else {
                debug!("packet not publish: {packet:?}");
                continue;
            };
            let payload = serde_json::from_slice(&publish.payload)
                .expect("could not deserialize publish payload");
            let publish = Publish {
                topic: publish.topic,
                payload,
            };
            info!("Received: {publish:?}");
            incoming_send
                .send(publish)
                .expect("failed to send incoming publish");
        }
    });
    let outgoing_job = spawn({
        let client = client.clone();
        async move {
            while let Some(publish) = outgoing_recv.recv().await {
                info!("Publishing: {publish:?}");
                let payload = serde_json::to_vec(&publish.payload).expect("could not serialize outgoing publish");
                client
                    .publish(publish.topic, QoS::AtLeastOnce, false, payload)
                    .await
                    .expect("failed to publish");
            }
        }
    });

    (Connection {
        client,
        receiver: incoming_recv,
        sender: outgoing_send,
    }, CancelGuard {
        broker,
        incoming_job,
        outgoing_job,
    })
}

/// A guard which cancels background tasks when dropped
pub struct CancelGuard {
    broker: Child,
    incoming_job: JoinHandle<()>,
    outgoing_job: JoinHandle<()>,
}

impl Drop for CancelGuard {
    fn drop(&mut self) {
        self.incoming_job.abort();
        self.outgoing_job.abort();
        self.broker.start_kill().expect("failed to kill mqtt broker");
    }
}

#[derive(Debug, Clone)]
struct Publish {
    topic: String,
    payload: Value,
}

/// Represents a connection to a Mqtt broker
pub struct Connection {
    client: Arc<AsyncClient>,
    receiver: Receiver<Publish>,
    sender: Sender<Publish>,
}

impl Connection {
    async fn new_device(&self, name: &str) -> (Receiver<Publish>, Sender<Publish>) {
        self.client
            .subscribe(format!("zigbee2mqtt/{name}"), QoS::AtLeastOnce)
            .await
            .expect("failed to subscribe to device");
        (self.receiver.resubscribe(), self.sender.clone())
    }
}

struct MockDevice {
    name: &'static str,
    sender: Sender<Publish>,
}

impl MockDevice {
    async fn new(connection: &Connection, name: &'static str) -> (MockDevice, Receiver<Publish>) {
        let (receiver, sender) = connection.new_device(name).await;
        (Self { name, sender }, receiver)
    }
}

impl MockDevice {
    fn topic(&self) -> String {
        format!("zigbee2mqtt/{}", self.name)
    }

    async fn publish(&self, payload: Value) {
        let payload = serde_json::to_vec(&payload).expect("failed to serialize payload");
        self.sender
            .send(Publish {
                topic: self.topic(),
                payload: payload.into(),
            })
            .await
            .expect("failed to send publish");
    }
}

/// Create a mock Hue Button
pub async fn mock_philips_button(connection: &Connection, name: &'static str) -> MockHueButton {
    MockHueButton(MockDevice::new(connection, name).await.0)
}

/// A Mock hue button
pub struct MockHueButton(MockDevice);

impl MockHueButton {
    /// trigger an action
    pub async fn action(&mut self, action: &'static str) {
        let payload = json! {{"action": action}};
        self.0.publish(payload).await;
    }
}

/// Create a mock hue light
pub async fn mock_philips_light(
    broker: &Connection,
    name: &'static str,
    state: bool,
    brightness: u8,
) -> Arc<MockLight> {
    let (device, receiver) = MockDevice::new(broker, name).await;
    let mock = MockLight {
        device,
        state: AtomicBool::new(state),
        brightness: AtomicU8::new(brightness),
    };
    let mock = Arc::new(mock);
    spawn(mock.clone().run(receiver));
    mock
}

/// A mock light
pub struct MockLight {
    device: MockDevice,
    state: AtomicBool,
    brightness: AtomicU8,
}

impl MockLight {
    async fn run(self: Arc<Self>, receiver: Receiver<Publish>) {
        let mut receiver = BroadcastStream::new(receiver);
        while let Some(result) = receiver.next().await {
            let Ok(publish) = result else {
                continue
            };
            if publish.topic != self.device.topic() {
                continue;
            }
            let value = publish.payload;
            if value == json! {{"state": {}}} {
                self.device.publish(json! {{"state": self.state}}).await
            }
            if value == json! {{"brightness": {}}} {
                self
                    .device
                    .publish(json! {{"brightness": self.brightness}})
                    .await
            }
            if let Value::Object(object) = value {
                if let Some(Value::String(new_state)) = object.get("state") {
                    match new_state.as_str() {
                        "ON" => self.state.store(true, Ordering::Relaxed),
                        "OFF" => self.state.store(false, Ordering::Relaxed),
                        "TOGGLE" => self.state.store(!self.state.load(Ordering::Relaxed), Ordering::Relaxed),
                        other => panic!("unknown state: {}", other),
                    };
                }
                if let Some(Value::Number(brightness)) = object.get("brightness") {
                    self.brightness.store(u8::try_from(brightness.as_u64().unwrap()).unwrap(), Ordering::Relaxed);
                }
            }
        }
    }

    /// Get current state
    pub fn state(&self) -> bool {
        self.state.load(Ordering::Relaxed)
    }

    /// Get current brightness
    pub fn brightness(&self) -> u8 {
        self.brightness.load(Ordering::Relaxed)
    }
}
