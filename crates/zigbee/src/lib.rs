#![feature(push_mut)]
#![allow(clippy::new_without_default)]
#![allow(dead_code)] // TODO: remove once we have more complete examples
// #![warn(missing_docs)]
#![doc = include_str!("../README.md")]

mod attribute;
mod publish;
pub mod devices {
    pub mod philips;
    pub mod sonoff;
    pub mod aqara;
    pub mod aurora;
}

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
use std::thread::JoinHandle;
use tokio::sync::{broadcast, mpsc};
use tokio::sync::broadcast::Sender;
use tokio_stream::Stream;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;

pub use macros::DeviceSet;

pub struct Manager {
    mqtt_options: Option<MqttOptions>,
    subscriptions: Vec<Subscription>,
    publishes: mpsc::Sender<Publish>,
    outgoing: mpsc::Receiver<Publish>,
}

pub trait Device {
    fn new(name: String, worker: &mut Manager) -> Self;

    fn name(&self) -> &str;
}

pub trait DeviceSet {
    fn new(worker: &mut Manager) -> Self;
}

impl Manager {
    pub fn new() -> Self {
        let (publishes, outgoing) = mpsc::channel::<Publish>(100);
        Self {
            mqtt_options: None,
            subscriptions: vec![],
            publishes,
            outgoing,
        }
    }

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

    pub fn add_device<D: Device>(&mut self, name: impl Into<String>) -> D {
        D::new(name.into(), self)
    }

    pub fn add_device_set<D: DeviceSet>(&mut self) -> D {
        D::new(self)
    }

    pub fn start(self) -> Worker {
        let mqttoptions = self.mqtt_options.expect("no mqtt options set");
        println!("creating client");
        let (client, connection) = Client::new(mqttoptions, 10);

        let subscriptions = self.subscriptions;
        for subscription in &subscriptions {
            client
                .subscribe(&subscription.topic, QoS::AtMostOnce)
                .expect("failed to start subscription")
        }

        Worker {
            subscriber: thread::spawn(move || Self::subscription_job(connection, subscriptions)),
            publisher: thread::spawn(move || Self::publish_job(client, self.outgoing)),
        }
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

#[allow(dead_code)] // may be used in the future, cost nothing now
pub struct Worker {
    subscriber: JoinHandle<()>,
    publisher: JoinHandle<()>,
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
