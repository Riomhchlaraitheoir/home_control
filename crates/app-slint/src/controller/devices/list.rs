use std::rc::Rc;

use async_compat::Compat;
use futures::{SinkExt, StreamExt};
use log::warn;
use slint::{ComponentHandle, VecModel, spawn_local};

use crate::{
    controller::CLIENT,
    ui::{App, Device, DeviceListAdapter},
};

pub fn connect_device_list_controller(app: &App) {
    let adapter: DeviceListAdapter = app.global();
    adapter.on_load_devices({
        let app = app.as_weak();
        move || {
            let app = app.unwrap();
            let adapter: DeviceListAdapter = app.global();
            let devices: Rc<VecModel<Device>> = Rc::default();
            adapter.set_devices(devices.clone().into());
            let (mut send_device, mut recv_device) =
                futures::channel::mpsc::channel::<api::Device>(10);
            let client = CLIENT.with_borrow(|option| option.as_ref().unwrap().clone());
            smol::spawn(Compat::new(async move {
                let mut device_stream = client.get_devices().await.unwrap();
                while let Some(device) = device_stream.next().await {
                    let Ok(device) = device else {
                        warn!("Error from get_devices stream");
                        continue;
                    };
                    send_device.send(device).await.unwrap();
                }
            }))
            .detach();
            spawn_local(async move {
                while let Some(device) = recv_device.next().await {
                    devices.push(device.into());
                }
                app.global::<DeviceListAdapter>().set_loaded(true);
            })
            .unwrap();
        }
    });
}
