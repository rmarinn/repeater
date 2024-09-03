use std::sync::Arc;
use tokio::net::TcpStream;
use tokio_tungstenite::WebSocketStream;

/// Enum representing different commands that can be sent to the `ConnectionManager`.
pub enum Command {
    /// Register a new WebSocket connection with the `ConnectionManager`.
    ///
    /// # Arguments
    /// * `WebSocketStream<TcpStream>` - The WebSocket stream associated with the connection.
    Register(WebSocketStream<TcpStream>),

    /// Unregister an existing client connection from the `ConnectionManager`.
    ///
    /// # Arguments
    /// * `Arc<str>` - The unique identifier of the client to unregister.
    Unregister(Arc<str>),

    /// Relay a message from one client to another.
    ///
    /// # Arguments
    /// * `sender_id: Arc<str>` - The unique identifier of the sending client.
    /// * `recvr_id: Box<str>` - The unique identifier of the receiving client.
    /// * `msg: Box<str>` - The message to be relayed.
    RelayMessage {
        sender_id: Arc<str>,
        recvr_id: Box<str>,
        msg: Box<str>,
    },

    /// Send an error message to a specific client.
    ///
    /// # Arguments
    /// * `client_id: Arc<str>` - The unique identifier of the client to receive the error message.
    /// * `msg: Box<str>` - The error message content.
    ErrorMessage { client_id: Arc<str>, msg: Box<str> },
}
