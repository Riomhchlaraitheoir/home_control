#![doc = include_str!("../README.md")]

mod attribute;
mod publish;

use crate::publish::Publish;
use bon::bon;
use control::manager::DeviceManager;
use control::ReadValue;
use control::Sensor;
use control::ToggleValue;
use control::WriteValue;
use rumqttc::{AsyncClient, Event, EventLoop, Incoming, MqttOptions, QoS};
use serde::Deserialize;
use serde_json::Value;
use std::marker::PhantomData;
use tokio::sync::broadcast::Sender;
use tokio::sync::{broadcast, mpsc};
use tokio::{select, spawn};
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::Stream;
use tokio_stream::StreamExt;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, warn};

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
    mqtt_options: MqttOptions,
    subscriptions: Vec<Subscription>,
    publishes: mpsc::Sender<Publish>,
    outgoing: mpsc::Receiver<Publish>,
}

#[bon]
impl Manager {
    /// Create a new manager
    #[builder]
    pub fn new(
        /// The MQTT options used to establish a connection
        mqtt_options: MqttOptions
    ) -> Self {
        let (publishes, outgoing) = mpsc::channel::<Publish>(100);
        Self {
            mqtt_options,
            subscriptions: vec![],
            publishes,
            outgoing,
        }
    }
}

impl DeviceManager for Manager {
    fn start(self: Box<Self>, token: CancellationToken) {
        let mqttoptions = self.mqtt_options;
        let (client, event_loop) = AsyncClient::new(mqttoptions, 10);

        spawn(Self::subscription_job(
            event_loop,
            self.subscriptions.clone(),
            token.clone()
        ));
        spawn(Self::publish_job(client, self.outgoing, self.subscriptions, token));
    }
}

impl Manager {
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

    async fn subscription_job(mut event_loop: EventLoop, subscriptions: Vec<Subscription>, token: CancellationToken) {
        loop {
            let event = select! {
                _ = token.cancelled() => break,
                result = event_loop.poll() => {
                    match result {
                        Ok(event) => event,
                        Err(err) => {
                            warn!("Error from connection: {err}");
                            break;
                        }
                    }
                }
            };
            match event {
                Event::Outgoing(_) => {}
                Event::Incoming(message) => {
                    let Incoming::Publish(publish) = message else {
                        continue;
                    };
                    let Ok(publish): Result<Publish, _> = publish.try_into() else {
                        error!("failed to decode incoming publish payload");
                        continue
                    };
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
    }

    async fn publish_job(
        client: AsyncClient,
        mut publishes: mpsc::Receiver<Publish>,
        subscriptions: Vec<Subscription>,
        token: CancellationToken,
    ) {
        debug!("creating subscriptions");
        for subscription in subscriptions {
            if let Err(error) = client
                .subscribe(&subscription.topic, QoS::AtLeastOnce)
                .await {
                error!("Failed to subscribe to topic {}: {error}", subscription.topic);
            }
        }
        debug!("subscriptions created");
        debug!("starting publish loop");
        loop {
            let option = select! {
                _ = token.cancelled() => break,
                option = publishes.recv() => option
            };
            let Some(publish) = option else {
                break
            };
            debug!("sending publish: {publish:?}");
            if let Err(error) = client
                .publish(
                    format!("zigbee2mqtt/{}", publish.topic),
                    QoS::AtMostOnce,
                    false,
                    publish.raw_payload,
                )
                .await {
                error!("Failed to publish payload: {error}");
            }
        }
        debug!("finishing subscription loop");
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
