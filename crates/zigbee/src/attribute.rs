#![allow(dead_code, reason = "some types are not used yet, but may be as support for more devices is added")]

use crate::get_request;
use crate::publish::Publish;
use crate::Sensor;
use crate::ToggleValue;
use crate::WriteValue;
use crate::{ReadValue, Updates};
use futures::future::join;
use futures::{FutureExt, Stream, TryFutureExt};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::convert::identity;
use std::marker::PhantomData;
use tokio::sync::mpsc::Sender;
use tokio_stream::StreamExt;
use anyhow::Error;
use control::InputStreamClosed;

#[derive(Clone)]
pub struct SubscribeAttr<Update, Item> {
    updates: Updates<Update>,
    func: fn(Update) -> Option<Item>,
}

impl<Update, Item> Sensor for SubscribeAttr<Update, Item>
where
        Item: Sync,
        Update: for<'de> Deserialize<'de>
{
    type Item = Item;

    fn subscribe(&self) -> Box<dyn Stream<Item=Self::Item> + Unpin + Send + '_> {
        Box::new(self.updates.subscribe().filter_map(self.func))
    }
}

impl<Update, Item> SubscribeAttr<Update, Item>
where
    for<'de> Update: Deserialize<'de>,
{
    pub fn new(
        updates: Updates<Update>,
        func: fn(Update) -> Option<Item>
    ) -> Self {
        Self {
            updates,
            func,
        }
    }
}

#[derive(Clone)]
pub struct PublishAttr<Item, Zigbee> {
    attribute_name: &'static str,
    func: fn(Item) -> Zigbee,
    _t: PhantomData<(Item, Zigbee)>,
    publisher: Sender<Publish>,
    device_name: String,
}

impl<Item> PublishAttr<Item, Item>
where
    for<'de> Item: Deserialize<'de>,
{
    pub fn new(
        publisher: Sender<Publish>,
        device_name: String,
        attribute_name: &'static str,
    ) -> Self {
        Self {
            attribute_name,
            func: identity,
            _t: Default::default(),
            publisher,
            device_name,
        }
    }
}

impl<Item, Zigbee> PublishAttr<Item, Zigbee>
where
        for<'de> Zigbee: Deserialize<'de>,
{
    pub fn new_mapped(
        publisher: Sender<Publish>,
        device_name: String,
        attribute_name: &'static str,
        func: fn(Item) -> Zigbee,
    ) -> Self {
        Self {
            attribute_name,
            func,
            _t: Default::default(),
            publisher,
            device_name,
        }
    }
    }

impl<Item, Zigbee> WriteValue for PublishAttr<Item, Zigbee>
where
    Zigbee: Serialize,
{
    type Item = Item;

    fn set(&self, value: Self::Item) -> Box<dyn Future<Output=Result<(), Error>> + Unpin + Send + '_> {
        let key = self.attribute_name;
        let value = (self.func)(value);
        Box::new(Box::pin(self.publisher
            .send(
                Publish::new(format!("{}/set", self.device_name), json!({key: value}))
                    .expect("failed to serialise JSON"),
            )
            .unwrap_or_else(|error| panic!("failed to send {:?}", error.0))
            .map(Ok)))
    }
}

impl<Item> ToggleValue for PublishAttr<Item, String>
where
    Item: Serialize
{
    fn toggle(&self) -> Box<dyn Future<Output=Result<(), Error>> + Unpin + Send + '_> {
        let key = self.attribute_name;
        let publish = Publish::new(
            format!("{}/set", self.device_name),
            json!({key: "TOGGLE"}),
        ).expect("failed to serialize JSON");
        Box::new(Box::pin(self.publisher
            .send(publish)
            .unwrap_or_else(|error| panic!("failed to send {:?}", error.0))
            .map(Ok)))
    }
}

#[derive(Clone)]
pub struct SubscribePublishAttr<Item, Update, Zigbee>
where
    Update: for<'de> Deserialize<'de> {
    attribute_name: &'static str,
    from_device: fn(Update) -> Option<Item>,
    to_device: fn(Item) -> Zigbee,
    _t: PhantomData<(Item, Zigbee)>,
    updates: Updates<Update>,
    publisher: Sender<Publish>,
    device_name: String,
}

impl<Update, Item> SubscribePublishAttr<Item, Update, Item>
where
    for<'de> Update: Deserialize<'de>,
{
    pub fn new(
        updates: Updates<Update>,
        publisher: Sender<Publish>,
        device_name: String,
        attribute_name: &'static str,
        from_device: fn(Update) -> Option<Item>
    ) -> Self {
        Self {
            updates,
            attribute_name,
            from_device,
            to_device: identity,
            _t: Default::default(),
            publisher,
            device_name,
        }
    }
}

impl<Item, Update, Zigbee> SubscribePublishAttr<Item, Update, Zigbee>
where
    for<'de> Update: Deserialize<'de>,
    Zigbee: Serialize,
{
    pub fn new_mapped(
        updates: Updates<Update>,
        publisher: Sender<Publish>,
        device_name: String,
        attribute_name: &'static str,
        from_device: fn(Update) -> Option<Item>,
        to_device: fn(Item) -> Zigbee,
    ) -> Self {
        Self {
            updates,
            attribute_name,
            from_device,
            to_device,
            _t: Default::default(),
            publisher,
            device_name,
        }
    }
}

impl<Item, Update, Zigbee> Sensor for SubscribePublishAttr<Item, Update, Zigbee>
where
    for<'de> Update: Deserialize<'de>,
    Zigbee: Serialize + Sync,
    Item: Sync
{
    type Item = Item;

    fn subscribe(&self) -> Box<dyn Stream<Item=Self::Item> + Unpin + Send + '_> {
        Box::new(self.updates.subscribe().filter_map(self.from_device))
    }
}

impl<Item, Update, Zigbee> ReadValue for SubscribePublishAttr<Item, Update, Zigbee>
where
        for<'de> Update: Deserialize<'de> + Sync,
        Zigbee: Serialize,
        Item: Sync + Send,
        Zigbee: Sync,
{
    type Item = Item;

    fn get(&self) -> Box<dyn Future<Output=Result<Self::Item, Error>> + Unpin + Send + '_> {
        Box::new(Box::pin(async {
            let mut stream = Box::pin(self.subscribe());
            let response = stream.next();
            let request = self.publisher.send(
                Publish::new(
                    format!("{}/get", self.device_name),
                    get_request(self.attribute_name),
                )
                    .expect("JSON serialisation failed"),
            ).unwrap_or_else(|err| panic!("failed to publish: {err}"));

            let (_, value) = join(request, response).await;
            value.ok_or(Error::new(InputStreamClosed))
        }))
    }
}

impl<Item, Update, Zigbee> WriteValue for SubscribePublishAttr<Item, Update, Zigbee>
where
    for<'de> Update: Deserialize<'de>,
    Zigbee: Serialize,
{
    type Item = Item;

    fn set(&self, value: Self::Item) -> Box<dyn Future<Output=Result<(), Error>> + Unpin + Send + '_> {
        let key = self.attribute_name;
        let value = (self.to_device)(value);
        Box::new(Box::pin(self.publisher
            .send(
                Publish::new(format!("{}/set", self.device_name), json!({key: value}))
                    .expect("failed to serialise JSON"),
            )
            .unwrap_or_else(|error| panic!("failed to send {:?}", error.0))
            .map(Ok)))
    }
}

impl<Item, Update> ToggleValue for SubscribePublishAttr<Item, Update, String>
where
    Update: for<'de> Deserialize<'de>,
{
    fn toggle(&self) -> Box<dyn Future<Output=Result<(), Error>> + Unpin + Send + '_> {
        let publish = Publish::new(
            format!("{}/set", self.device_name),
            json!({self.attribute_name: "TOGGLE"}),
        ).expect("failed to serialise JSON");
        Box::new(Box::pin(self.publisher
            .send(publish)
            .unwrap_or_else(|error| panic!("failed to send {:?}", error.0))
            .map(Ok)))
    }
}
