mod command;
mod connection_handler;
mod connection_manager;
mod serialization;

use command::Command;
use connection_handler::ConnectionHandler;
use connection_manager::ConnectionManager;
use serialization::{ClientMessage, ServerMessage};
use std::net::SocketAddr;
use tokio::{
    net::TcpListener,
    sync::{broadcast, mpsc},
};
use tokio_tungstenite::accept_async;

/// Starts the WebSocket server at the specified address.
///
/// This function sets up the server, starts listening for new connections,
/// and handles shutdown signals gracefully.
///
/// # Arguments
/// * `addr: SocketAddr` - The address at which the server should listen for connections.
pub async fn start_server(addr: SocketAddr) {
    // setup a channel where threads will receive the shutdown signal
    let (shutdown_tx, _) = broadcast::channel::<()>(1);

    // setup the manager that handles ws messages
    let (cmd_tx, mngr_handler) = ConnectionManager::spawn(shutdown_tx.subscribe());

    // start accepting connections
    let listener_handle = tokio::spawn(start_listener(addr, cmd_tx, shutdown_tx.subscribe()));

    // send shutdown signal when CTRL+C is pressed
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to listen for Ctrl+C");
    let _ = shutdown_tx.send(());

    listener_handle.await.unwrap();
    mngr_handler.await.unwrap();

    println!("Server shut down gracefully");
}

/// Opens a `TcpListener` and registers new WebSocket connections with the `ConnectionManager`.
///
/// This function listens for incoming TCP connections, upgrades them to WebSocket connections,
/// and registers them with the `ConnectionManager`. It also listens for shutdown signals to
/// terminate the listener loop.
///
/// # Arguments
/// * `addr: SocketAddr` - The address at which the `TcpListener` should listen for connections.
/// * `cmd_tx: mpsc::Sender<Command>` - The channel to send registration commands to the `ConnectionManager`.
/// * `shutdown_rx: broadcast::Receiver<()>` - The receiver to listen for shutdown signals.
async fn start_listener(
    addr: SocketAddr,
    cmd_tx: mpsc::Sender<Command>,
    mut shutdown_rx: broadcast::Receiver<()>,
) {
    let listener = TcpListener::bind(addr).await.unwrap();
    println!("Listening on: {}", addr);

    loop {
        tokio::select! {
            Ok((stream, _socket_addr)) = listener.accept() => {
                let ws_stream = accept_async(stream).await.unwrap();

                // register new connections on the manager
                if cmd_tx.send(Command::Register(ws_stream)).await.is_err() {
                    break; // break since server is likely shutting down
                }
            },
            _ = shutdown_rx.recv() => break,
        }
    }
}
