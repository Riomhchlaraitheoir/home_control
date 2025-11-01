#![feature(push_mut)]
#![doc = include_str!("../README.md")]

mod attribute;
mod publish;

use crate::publish::Publish;
use control::ReadValue;
use control::Sensor;
use control::ToggleValue;
use control::WriteValue;
use futures::future::{AbortHandle, BoxFuture};
use tracing::{debug, warn};
use rumqttc::{AsyncClient, Event, EventLoop, Incoming, MqttOptions, QoS};
use serde::Deserialize;
use serde_json::Value;
use std::marker::PhantomData;
use std::pin::pin;
use tokio::sync::broadcast::Sender;
use tokio::sync::{broadcast, mpsc};
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use tokio_stream::wrappers::{BroadcastStream, ReceiverStream};
use tokio_stream::Stream;
use tokio_stream::StreamExt;

/// Definitions for all supported zigbee devices
pub mod devices {
    /// Aqara devices
    pub mod aqara;
    /// Aurora devices
    pub mod aurora;
    /// Philips devices
    pub mod philips;
    /// Sonoff devices
    pub mod sonoff;
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
        self.subscriptions.push(Subscription {
            topic: format!("zigbee2mqtt/{topic}"),
            sender: sender.clone(),
        });
        Updates {
            sender,
            _t: PhantomData,
        }
    }

    pub(crate) fn outgoing_publishes(&self) -> mpsc::Sender<Publish> {
        self.publishes.clone()
    }

    /// spawns 2 threads
    /// - one to handle incoming updates, passing them to the relevant channels
    /// - another to handle outgoing publishes, sending them to the MQTT broker
    pub async fn start(self) -> (Vec<BoxFuture<'static, ()>>, Vec<AbortHandle>) {
        let mqttoptions = self.mqtt_options.expect("no mqtt options set");
        println!("creating client");
        let (client, event_loop) = AsyncClient::new(mqttoptions, 10);

        let subscriptions = self.subscriptions;
        for subscription in &subscriptions {
            client
                .subscribe(&subscription.topic, QoS::AtMostOnce)
                .await
                .expect("failed to start subscription")
        }
        let (subscribe_job, abort_subscribe) = Self::subscription_job(event_loop, subscriptions);
        let (publish_job, abort_publish) = Self::publish_job(client, self.outgoing);
        (vec![Box::pin(subscribe_job), Box::pin(publish_job)], vec![abort_subscribe, abort_publish])
    }

    fn subscription_job(event_loop: EventLoop, subscriptions: Vec<Subscription>) -> (impl Future<Output = ()>, AbortHandle) {
        debug!("starting subscription thread");
        let events = futures::stream::unfold(event_loop, |mut event_loop| {
            async {
                match event_loop.poll().await {
                    Ok(event) => Some((event, event_loop)),
                    Err(err) => {
                        warn!("Error from connection: {err}");
                        None
                    }
                }
            }
        });
        let (events, abort_handle) = futures::stream::abortable(events);
        let future = async move {
            let mut events = pin!(events);
            while let Some(event) = events.next().await {
                match event {
                    Event::Outgoing(_) => {}
                    Event::Incoming(message) => {
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
                }
            }
        };
        (future, abort_handle)
    }

    fn publish_job(client: AsyncClient, publishes: mpsc::Receiver<Publish>) -> (impl Future<Output = ()>, AbortHandle) {
        debug!("starting publish thread");
        let (mut publishes, abort_handle) = futures::stream::abortable(ReceiverStream::new(publishes));
        let future = async move {
            while let Some(publish) = publishes.next().await {
                debug!("sending publish: {publish:?}");
                client
                    .publish(
                        format!("zigbee2mqtt/{}", publish.topic),
                        QoS::AtMostOnce,
                        false,
                        publish.raw_payload,
                    )
                    .await
                    .expect("failed to publish payload")
            }
        };
        (future, abort_handle)
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
    T: for<'de> Deserialize<'de>,
{
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
