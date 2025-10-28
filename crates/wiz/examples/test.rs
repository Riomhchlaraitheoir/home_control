use simple_log::{Level, LogConfigBuilder};
use wiz::light::Light;

#[tokio::main]
async fn main() {
    simple_log::new(
        LogConfigBuilder::builder()
            .level(Level::Info).unwrap()
            .output_console()
            .build()
    ).unwrap();
    let light = Light::verify_new("192.168.1.61".parse().unwrap()).await.expect("failed to discover lights");
    // light.turn_on(RangedU8::new(100), RangedU16::new(3000)).await.expect("failed to turn on light");
    // light.turn_off().await.expect("failed to turn light off");
    let state = light.get_state().await.expect("failed to get state");
    println!("State: {state:?}");
}