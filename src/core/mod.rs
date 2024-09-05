mod command;
mod connection_handler;
mod connection_manager;
mod serialization;

use anyhow::Result;
use command::Command;
use connection_handler::ConnectionHandler;
use connection_manager::ConnectionManager;
use serialization::{ClientMessage, ServerMessage};
use std::net::SocketAddr;
use tokio::{net::TcpListener, sync::mpsc};
use tokio_tungstenite::accept_async;
use tokio_util::sync::CancellationToken;

/// Starts the WebSocket server at the specified address.
///
/// This function initializes the WebSocket server, starts listening for new connections,
/// and gracefully shuts down when a shutdown signal (e.g., Ctrl+C) is received.
///
/// # Arguments
/// * `addr` - The `SocketAddr` on which the server should listen for incoming connections.
///
/// # Returns
/// This function runs the server indefinitely until a shutdown signal is received.
/// It returns a `anyhow::Result<()>` to handle potential errors during runtime.
pub async fn start_server(addr: SocketAddr) -> Result<()> {
    // Create a token to manage graceful shutdown across tasks
    let cancellation_token = CancellationToken::new();

    // Spawn the connection manager, responsible for handling WebSocket connections
    let (cmd_tx, mngr_handle) = ConnectionManager::spawn(cancellation_token.clone());

    // Start listening for incoming connections
    let listener_handle = tokio::spawn(start_listener(addr, cmd_tx));

    // Wait for Ctrl+C signal to initiate shutdown
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to listen for Ctrl+C");

    // Signal cancellation to all active tasks
    cancellation_token.cancel();

    // Await the graceful shutdown of the manager
    if let Err(e) = mngr_handle.await {
        eprintln!("Error in ConnectionManager: {:?}", e);
    }

    // stop listening
    listener_handle.abort();

    println!("Server shut down gracefully");
    Ok(())
}

/// Listens for incoming TCP connections, upgrades them to WebSocket connections,
/// and registers them with the `ConnectionManager`.
///
/// This function runs the listener loop, where it accepts incoming TCP connections,
/// upgrades them to WebSocket using `tokio-tungstenite`, and sends registration
/// commands to the `ConnectionManager` via the provided `cmd_tx` channel.
///
/// # Arguments
/// * `addr` - The address on which the `TcpListener` should listen for connections.
/// * `cmd_tx` - A `mpsc::Sender<Command>` channel for sending commands to the `ConnectionManager`.
///
/// # Errors
/// Returns an error if the listener fails to bind to the specified address or if there is
/// an issue accepting or upgrading TCP connections to WebSocket.
///
/// # Panics
/// This function will panic if it fails to bind to the specified address.
async fn start_listener(addr: SocketAddr, cmd_tx: mpsc::Sender<Command>) {
    let listener = match TcpListener::bind(addr).await {
        Ok(listener) => listener,
        Err(e) => {
            eprintln!("Failed to bind to address {}: {:?}", addr, e);
            return;
        }
    };

    println!("Listening on: {}", addr);

    loop {
        match listener.accept().await {
            Ok((stream, socket_addr)) => {
                println!("New connection from: {}", socket_addr);

                // Attempt to upgrade the TCP stream to a WebSocket connection
                match accept_async(stream).await {
                    Ok(ws_stream) => {
                        // Send the WebSocket stream to the ConnectionManager for registration
                        if cmd_tx.send(Command::Register(ws_stream)).await.is_err() {
                            eprintln!("Failed to send register command, shutting down listener");
                            break; // Shutdown likely triggered, exit loop
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to upgrade connection to WebSocket: {:?}", e);
                    }
                }
            }
            Err(e) => {
                eprintln!("Failed to accept connection: {:?}", e);
                break;
            }
        }
    }
}
