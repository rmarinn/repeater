use std::net::SocketAddr;

use anyhow::Result;
use connection_manager::{Command, ConnectionManager};
use tokio::{
    net::TcpListener,
    sync::{
        broadcast,
        mpsc::{self},
    },
};
use tokio_tungstenite::accept_async;

mod connection_manager;
mod serialization;

#[tokio::main]
async fn main() -> Result<()> {
    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));

    let (shutdown_tx, _) = broadcast::channel(1);

    let mngr = ConnectionManager::start(shutdown_tx.subscribe());

    // accept new connections
    let accept_handle = tokio::spawn(accept_connections(
        addr,
        mngr.cmd_tx.clone(),
        shutdown_tx.subscribe(),
    ));

    tokio::signal::ctrl_c()
        .await
        .expect("Failed to listen for Ctrl+C");

    let _ = shutdown_tx.send(());

    accept_handle.await.unwrap();
    mngr.handle.await.unwrap();

    println!("Server shut down gracefully");
    Ok(())
}

async fn accept_connections(
    addr: SocketAddr,
    mngr_tx: mpsc::Sender<Command>,
    mut shutdown_rx: broadcast::Receiver<()>,
) {
    let listener = TcpListener::bind(addr).await.unwrap();
    println!("Listening on: {}", addr);

    loop {
        tokio::select! {
            Ok((stream, _socket_addr)) = listener.accept() => {
                let ws_stream = accept_async(stream).await.unwrap();

                if mngr_tx
                    .send(Command::Register(ws_stream))
                    .await
                    .is_err()
                {
                    // Manager is likely shutting down so stop accepting new connections
                    break;
                }
            },
            _ = shutdown_rx.recv() => {
                break;
            }
        }
    }
}
