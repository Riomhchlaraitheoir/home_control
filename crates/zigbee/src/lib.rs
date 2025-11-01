#![feature(push_mut)]
#![doc = include_str!("../README.md")]

mod attribute;
mod publish;

use crate::publish::Publish;
use control::ReadValue;
use control::Sensor;
use control::ToggleValue;
use control::WriteValue;
use log::{debug, warn};
use rumqttc::{Client, Connection, Event, Incoming, MqttOptions, QoS};
use serde::Deserialize;
use serde_json::Value;
use std::marker::PhantomData;
use std::thread;
use tokio::sync::broadcast::Sender;
use tokio::sync::{broadcast, mpsc};
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::Stream;
use tokio_stream::StreamExt;

/// Definitions for all supported zigbee devices
pub mod devices {
    /// Philips devices
    pub mod philips;
    /// Sonoff devices
    pub mod sonoff;
    /// Aqara devices
    pub mod aqara;
    /// Aurora devices
    pub mod aurora;
}

/// sets up the zigbee environment, defining MQTT connection parameters and devices
pub struct Manager {
    mqtt_options: Option<MqttOptions>,
    subscriptions: Vec<Subscription>,
    publishes: mpsc::Sender<Publish>,
    outgoing: mpsc::Receiver<Publish>,
}

impl Default for Manager {
    fn default() -> Self {
        Self::new()
    }
}

impl Manager {
    /// Create a new manager
    pub fn new() -> Self {
        let (publishes, outgoing) = mpsc::channel::<Publish>(100);
        Self {
            mqtt_options: None,
            subscriptions: vec![],
            publishes,
            outgoing,
        }
    }

    /// Define the MQTT connection parameters
    pub fn set_mqtt_options(&mut self, options: MqttOptions) {
        self.mqtt_options = Some(options)
    }

    pub(crate) fn subscribe<T>(&mut self, topic: String) -> Updates<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        let (sender, _) = broadcast::channel::<Publish>(100);
        self.subscriptions
            .push(Subscription {
                topic: format!("zigbee2mqtt/{topic}"),
                sender: sender.clone(),
            });
        Updates { sender, _t: PhantomData }
    }

    pub(crate) fn outgoing_publishes(&self) -> mpsc::Sender<Publish> {
        self.publishes.clone()
    }

    /// spawns 2 threads
    /// - one to handle incoming updates, passing them to the relevant channels
    /// - another to handle outgoing publishes, sending them to the MQTT broker
    pub fn start(self) {
        let mqttoptions = self.mqtt_options.expect("no mqtt options set");
        println!("creating client");
        let (client, connection) = Client::new(mqttoptions, 10);

        let subscriptions = self.subscriptions;
        for subscription in &subscriptions {
            client
                .subscribe(&subscription.topic, QoS::AtMostOnce)
                .expect("failed to start subscription")
        }

        thread::spawn(move || Self::subscription_job(connection, subscriptions));
        thread::spawn(move || Self::publish_job(client, self.outgoing));
    }

    fn subscription_job(mut connection: Connection, subscriptions: Vec<Subscription>) {
        debug!("starting subscription thread");
        for notification in connection.iter() {
            match notification {
                Ok(Event::Outgoing(_)) => {}
                Ok(Event::Incoming(message)) => {
                    let Incoming::Publish(publish) = message else {
                        continue;
                    };
                    let publish: Publish = publish.into();
                    debug!("received publish: {publish:?}");
                    for Subscription { sender, .. } in subscriptions
                        .iter()
                        .filter(|s| publish.topic.starts_with(&s.topic))
                    {
                        // send will only fail when there are no subscribers, continue in this
                        // case since subscribers may join later
                        let _ = sender.send(publish.clone());
                    }
                }
                Err(err) => panic!("Error from connection: {err}"),
            }
        }
    }

    fn publish_job(client: Client, mut publishes: mpsc::Receiver<Publish>) {
        debug!("starting publish thread");
        while let Some(publish) = publishes.blocking_recv() {
            debug!("sending publish: {publish:?}");
            client
                .publish(
                    format!("zigbee2mqtt/{}", publish.topic),
                    QoS::AtMostOnce,
                    false,
                    publish.raw_payload,
                )
                .expect("failed to publish payload")
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Subscription {
    topic: String,
    sender: broadcast::Sender<Publish>,
}

#[derive(Debug, Clone)]
pub(crate) struct Updates<T> {
    sender: Sender<Publish>,
    _t: PhantomData<T>,
}

impl<T> Updates<T>
where
    T: for<'de> Deserialize<'de> {
    fn subscribe(&self) -> impl Stream<Item = T> {
        BroadcastStream::new(self.sender.subscribe())
            .ignore_lag()
            .payload::<T>()

        // warn!("failed to parse value: '{error}' from payload: {object:?} for topic: '{}'", topic);
    }
}

trait BroadcastStreamExt<T> {
    fn ignore_lag(self) -> impl Stream<Item = T>;
}

impl<T: 'static + Clone + Send> BroadcastStreamExt<T> for BroadcastStream<T> {
    fn ignore_lag(self) -> impl Stream<Item = T> {
        self.filter_map(|result| match result {
            Ok(publish) => Some(publish),
            Err(BroadcastStreamRecvError::Lagged(n)) => {
                warn!("dropped {n} messages");
                None
            }
        })
    }
}

trait StreamCustomExt: Stream {
    fn payload<P>(self) -> impl Stream<Item = P>
    where
        P: for<'de> Deserialize<'de>,
        Self: Stream<Item = Publish> + Sized,
    {
        self.filter_map(|publish| match publish.payload() {
            Ok(payload) => Some(payload),
            Err(error) => {
                warn!("failed to parse payload: '{error}' for publish: {publish:?}");
                None
            }
        })
    }
}

impl<S: Stream> StreamCustomExt for S {}

fn get_request(field: &str) -> Value {
    serde_json::json!({field: ""})
}
