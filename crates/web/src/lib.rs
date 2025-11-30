#![doc = include_str!("../README.md")]

use anyhow::Context;
pub use axum;
use axum::Router;
use bon::Builder;
use control::Service;

/// A Web Server for serving HTTP frontends/APIs/etc
#[derive(Debug, Builder)]
pub struct WebServer {
    #[builder(into)]
    #[builder(default = "0.0.0.0")]
    bind_address: String,
    port: u16,
    router: Router,
}

impl Service<'static> for WebServer {
    fn name(&self) -> String {
        "web-server".to_string()
    }
    async fn start(self) -> anyhow::Result<()> {
        let listener = tokio::net::TcpListener::bind((self.bind_address, self.port))
            .await
            .context("failed to bind address")?;
        axum::serve(listener, self.router)
            .await
            .context("http server failed")
    }
}
