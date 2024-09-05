use anyhow::Result;
use std::sync::Arc;
use tokio::{net::TcpStream, sync::oneshot};
use tokio_tungstenite::{tungstenite::Message, WebSocketStream};

/// Represents commands sent to the `ConnectionManager` for managing WebSocket connections.
///
/// This enum defines various operations that the `ConnectionManager` can handle, such as
/// registering new connections, unregistering clients, or relaying messages between clients.
pub enum Command {
    /// Register a new WebSocket connection with the `ConnectionManager`.
    ///
    /// # Parameters
    /// - `WebSocketStream<TcpStream>`: The WebSocket stream associated with the new connection.
    Register(WebSocketStream<TcpStream>),

    /// Unregister a client connection from the `ConnectionManager`.
    ///
    /// # Parameters
    /// - `Arc<str>`: The unique identifier of the client to be unregistered.
    Unregister(Arc<str>),

    /// Relay a message from one client to another.
    ///
    /// # Parameters
    /// - `client_id: Box<str>`: The unique identifier of the sending client.
    /// - `msg: Message`: The message to be relayed to the receiving client.
    /// - `result_tx: oneshot::Sender<Result<()>>`: A one-shot channel for signaling the result
    ///   of the message relay, indicating success or error.
    RelayMessage {
        client_id: Box<str>,
        msg: Message,
        result_tx: oneshot::Sender<Result<()>>,
    },
}
