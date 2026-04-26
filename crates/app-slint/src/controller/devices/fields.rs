use crate::ui::{BoolValue, FieldListAdapter};
use api::Value;
use async_compat::Compat;
use slint::{ComponentHandle, invoke_from_event_loop};

use crate::{controller::CLIENT, ui::App};

pub fn connect_field_controller(app: &App) {
    let adapter: FieldListAdapter = app.global();
    let app = app.as_weak();
    adapter.on_get_field_value({
        move |index, field_id| {
            let app = app.clone();
            let device_id = {
                let app = app.unwrap();
                let adapter: FieldListAdapter = app.global();
                adapter.get_device_id().to_string()
            };
            let client = CLIENT.with_borrow(|option| option.as_ref().unwrap().clone());
            smol::spawn(Compat::new(async move {
                let value = client
                    .device(device_id)
                    .field(field_id.into())
                    .get()
                    .await
                    .unwrap()
                    .expect("get not supported");
                let Value::Bool(value) = value else {
                    panic!("Expected boolean value")
                };
                invoke_from_event_loop(move || {
                    let app = app.unwrap();
                    let adapter: FieldListAdapter = app.global();
                    adapter.invoke_update_bool_value(
                        index,
                        BoolValue {
                            loaded: true,
                            null: false,
                            value,
                        },
                    )
                })
            }))
            .detach();
        }
    });
}
