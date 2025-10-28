use std::marker::PhantomData;
use influxdb::{Client, InfluxDbWriteable};
use tracing::warn;
use control::WriteValue;

struct InfluxDBData<T> {
    client: Client,
    _t: PhantomData<T>
}

impl<T: InfluxDbWriteable> WriteValue for InfluxDBData<T> {
    type Item = T;

    async fn set(&self, value: Self::Item) {
        if let Err(error) = self.client.query(value).await {
            warn!("error while writing to influxdb: {error}");
        }
    }
}