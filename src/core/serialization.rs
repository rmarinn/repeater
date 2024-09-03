use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio_tungstenite::tungstenite::Message;

/// Represents a message sent from the client to the server.
#[derive(Serialize, Deserialize)]
pub enum ClientMessage {
    /// Contains the client's identifier.
    ClientId { client_id: Box<str> },

    /// Represents a message intended for another client.
    /// Includes the receiver's ID and the message content.
    Message { recvr_id: Box<str>, msg: Box<str> },

    /// Reports an error with an accompanying message.
    Error { msg: Box<str> },

    /// Indicates that the client is disconnecting from the server.
    Disconnect,
}

impl ClientMessage {
    /// Converts a WebSocket `Message` into a `ClientMessage`.
    ///
    /// This function attempts to deserialize a text-based WebSocket message into a
    /// `ClientMessage` variant.
    ///
    /// # Errors
    /// Returns an error if deserialization fails.
    ///
    /// # Parameters
    /// - `msg`: The WebSocket `Message` to be converted.
    ///
    /// # Returns
    /// A `Result` containing the `ClientMessage` if successful, or an error otherwise.
    pub fn from(msg: Message) -> Result<Self> {
        let msg = match msg {
            Message::Text(msg) => serde_json::from_str::<Self>(&msg)?,
            Message::Close(_) => Self::Disconnect,
            _ => todo!(),
        };
        Ok(msg)
    }
}

/// Represents a message sent from the server to the client.
#[derive(Serialize, Deserialize)]
pub enum ServerMessage {
    /// Contains the client's identifier assigned by the server.
    ClientId { client_id: Box<str> },

    /// Represents a message relayed from one client to another.
    Message { sender_id: Box<str>, msg: Box<str> },

    /// Reports an error with an accompanying message.
    Error { msg: Box<str> },
}

impl ServerMessage {
    /// Creates a WebSocket `Message` containing the client's identifier.
    ///
    /// This function serializes a `ClientId` variant into a JSON string and
    /// encapsulates it in a WebSocket text message.
    ///
    /// # Parameters
    /// - `client_id`: The identifier of the client.
    ///
    /// # Returns
    /// A WebSocket `Message` containing the serialized client ID.
    pub fn client_id(client_id: Box<str>) -> Message {
        let client_id = Self::ClientId { client_id };
        Message::text(serde_json::to_string(&client_id).unwrap())
    }

    /// Creates a WebSocket `Message` containing a relayed message from another client.
    ///
    /// This function serializes a `Message` variant into a JSON string and
    /// encapsulates it in a WebSocket text message.
    ///
    /// # Parameters
    /// - `sender_id`: The identifier of the client who sent the original message.
    /// - `msg`: The content of the message.
    ///
    /// # Returns
    /// A WebSocket `Message` containing the serialized relayed message.
    pub fn relayed_msg(sender_id: Box<str>, msg: Box<str>) -> Message {
        let msg = Self::Message { sender_id, msg };
        Message::text(serde_json::to_string(&msg).unwrap())
    }

    /// Creates a WebSocket `Message` containing an error message.
    ///
    /// This function serializes an `Error` variant into a JSON string and
    /// encapsulates it in a WebSocket text message.
    ///
    /// # Parameters
    /// - `msg`: The error message to be sent.
    ///
    /// # Returns
    /// A WebSocket `Message` containing the serialized error message.
    pub fn error(msg: &str) -> Message {
        let msg = Self::Error { msg: msg.into() };
        Message::text(serde_json::to_string(&msg).unwrap())
    }
}
