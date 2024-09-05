use anyhow::Result;
use core::start_server;
use std::net::SocketAddr;

mod core;

#[tokio::main]
async fn main() -> Result<()> {
    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    start_server(addr).await.unwrap();
    Ok(())
}
