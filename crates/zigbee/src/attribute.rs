#![allow(
    dead_code,
    reason = "some types are not used yet, but may be as support for more devices is added"
)]

use crate::Sensor;
use crate::ToggleValue;
use crate::WriteValue;
use crate::get_request;
use crate::publish::Publish;
use crate::{ReadValue, Updates};
use anyhow::Result;
use anyhow::{Context, Error};
use control::InputStreamClosed;
use futures::FutureExt;
use futures::future::{BoxFuture, join};
use futures::stream::BoxStream;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::convert::identity;
use tokio::sync::mpsc::Sender;
use tokio_stream::StreamExt;

#[derive(Clone)]
pub struct SubscribeAttr<Update, Item> {
    updates: Updates<Update>,
    func: fn(Update) -> Option<Item>,
}

impl<Update, Item> Sensor for SubscribeAttr<Update, Item>
where
    Update: for<'de> Deserialize<'de>,
{
    type Item = Item;

    fn subscribe(&self) -> BoxStream<'_, Self::Item> {
        Box::pin(self.updates.subscribe().filter_map(self.func))
    }
}

impl<Update, Item> SubscribeAttr<Update, Item>
where
    for<'de> Update: Deserialize<'de>,
{
    pub fn new(updates: Updates<Update>, func: fn(Update) -> Option<Item>) -> Self {
        Self { updates, func }
    }
}

#[derive(Clone)]
pub struct PublishAttr<Item, Zigbee> {
    attribute_name: &'static str,
    func: fn(Item) -> Zigbee,
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

    fn set(&self, value: Self::Item) -> BoxFuture<'_, Result<()>> {
        let key = self.attribute_name;
        let value = (self.func)(value);
        let publish = Publish::new(format!("{}/set", self.device_name), json!({key: value}));

        Box::pin(async move {
            self.publisher
                .send(publish.context("serialize JSON")?)
                .await
                .context("publish set request")
        })
    }
}

impl<Item> ToggleValue for PublishAttr<Item, String>
where
    Item: Serialize,
{
    fn toggle(&self) -> BoxFuture<'_, Result<()>> {
        Box::pin(async {
            let key = self.attribute_name;
            let publish = Publish::new(format!("{}/set", self.device_name), json!({key: "TOGGLE"}))
                .context("serialize JSON")?;
            self.publisher
                .send(publish)
                .await
                .context("publish toggle request")
        })
    }
}

#[derive(Clone)]
pub struct SubscribePublishAttr<Item, Update, Zigbee>
where
    Update: for<'de> Deserialize<'de>,
{
    attribute_name: &'static str,
    from_device: fn(Update) -> Option<Item>,
    to_device: fn(Item) -> Zigbee,
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
        from_device: fn(Update) -> Option<Item>,
    ) -> Self {
        Self {
            updates,
            attribute_name,
            from_device,
            to_device: identity,
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
            publisher,
            device_name,
        }
    }
}

impl<Item, Update, Zigbee> Sensor for SubscribePublishAttr<Item, Update, Zigbee>
where
    for<'de> Update: Deserialize<'de>,
    Zigbee: Serialize,
{
    type Item = Item;

    fn subscribe(&self) -> BoxStream<'_, Self::Item> {
        Box::pin(self.updates.subscribe().filter_map(self.from_device))
    }
}

impl<Item, Update, Zigbee> ReadValue for SubscribePublishAttr<Item, Update, Zigbee>
where
    for<'de> Update: Deserialize<'de>,
    Zigbee: Serialize,
    Item: Send,
{
    type Item = Item;

    fn get(&self) -> BoxFuture<'_, Result<Self::Item>> {
        let mut stream = self.subscribe();
        let response = async move { stream.next().await };
        let publish = Publish::new(
            format!("{}/get", self.device_name),
            get_request(self.attribute_name),
        ).context("serialize JSON");
        let publisher = &self.publisher;
        let request = async move {
            publisher
                .send(publish?)
                .await
                .context("publish toggle request")
        };

        Box::pin(
            join(request, response).map(|(_, value)| value.ok_or(Error::new(InputStreamClosed))),
        )
    }
}

impl<Item, Update, Zigbee> WriteValue for SubscribePublishAttr<Item, Update, Zigbee>
where
    for<'de> Update: Deserialize<'de>,
    Zigbee: Serialize,
{
    type Item = Item;

    fn set(&self, value: Self::Item) -> BoxFuture<'_, Result<()>> {
        let key = self.attribute_name;
        let value = (self.to_device)(value);
        let publish = Publish::new(format!("{}/set", self.device_name), json!({key: value}));
        let publisher = &self.publisher;
        Box::pin(async {
            publisher
                .send(publish.context("serialize JSON")?)
                .await
                .context("publish set request")
        })
    }
}

impl<Item, Update> ToggleValue for SubscribePublishAttr<Item, Update, String>
where
    Update: for<'de> Deserialize<'de>,
{
    fn toggle(&self) -> BoxFuture<'_, Result<()>> {
        let publish = Publish::new(
            format!("{}/set", self.device_name),
            json!({self.attribute_name: "TOGGLE"}),
        );
        let publisher = &self.publisher;
        Box::pin(async move {
            publisher
                .send(publish.context("serialize JSON")?)
                .await
                .context("publish toggle request")
        })
    }
}
