use async_compat::Compat;
use log::warn;
use slint::{ComponentHandle, invoke_from_event_loop};

use crate::{
    controller::CLIENT,
    ui::{App, DeviceViewAdapter},
};

pub fn connect_device_view_controller(app: &App) {
    let adapter: DeviceViewAdapter = app.global();
    adapter.on_load_device({
        let app = app.as_weak();
        move |device_id| {
            let client = CLIENT.with_borrow(|option| option.as_ref().unwrap().clone());
            let app = app.clone();
            smol::spawn(Compat::new(async move {
                let device = client.device(device_id.to_string()).get().await;
                let Ok(device) = device else {
                    warn!("Error from get_devices stream");
                    return;
                };
                let Some(device) = device else {
                    warn!("Device not found: {device_id}");
                    return;
                };
                invoke_from_event_loop(move || {
                    let app = app.unwrap();
                    let adapter: DeviceViewAdapter = app.global();
                    adapter.set_device(device.into());
                    adapter.set_loaded(true);
                })
                .unwrap();
            }))
            .detach();
        }
    });
}
