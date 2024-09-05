use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio_tungstenite::tungstenite::Message;

/// Represents a message sent from the client to the server.
///
/// This enum encapsulates different types of messages that the client may send to the server,
/// such as client identification, text messages for other clients, error reports, or
/// disconnect notifications.
#[derive(Serialize, Deserialize)]
pub enum ClientMessage {
    /// Contains the client's identifier.
    ClientId { client_id: Box<str> },

    /// A message intended for another client.
    ///
    /// # Parameters
    /// - `recipient_id`: The unique identifier of the message recipient.
    /// - `msg`: The message content.
    Message {
        recipient_id: Box<str>,
        msg: Box<str>,
    },

    /// Reports an error with an accompanying message.
    Error { msg: Box<str> },

    /// Indicates that the client is disconnecting from the server.
    Disconnect,
}

impl ClientMessage {
    /// Converts a WebSocket `Message` into a `ClientMessage`.
    ///
    /// This function deserializes a text-based WebSocket message into the appropriate
    /// `ClientMessage` variant.
    ///
    /// # Errors
    /// Returns an error if deserialization fails or if the message format is unsupported.
    ///
    /// # Parameters
    /// - `msg`: The WebSocket `Message` to be converted.
    ///
    /// # Returns
    /// A `Result` containing the deserialized `ClientMessage` or an error if conversion fails.
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
///
/// This enum encapsulates the different types of messages that the server may send to
/// the client, such as client identification, relayed messages from other clients, or
/// error reports.
#[derive(Serialize, Deserialize)]
pub enum ServerMessage {
    /// Contains the client's identifier assigned by the server.
    ClientId { client_id: Box<str> },

    /// Represents a message relayed from one client to another.
    ///
    /// # Parameters
    /// - `sender_id`: The unique identifier of the client who sent the original message.
    /// - `msg`: The content of the relayed message.
    Message { sender_id: Box<str>, msg: Box<str> },

    /// Reports an error with an accompanying message.
    Error { msg: Box<str> },
}

impl ServerMessage {
    /// Creates a WebSocket `Message` containing the client's identifier.
    ///
    /// This function serializes the `ClientId` variant and encapsulates it in a WebSocket
    /// text message for transmission to the client.
    ///
    /// # Parameters
    /// - `client_id`: The identifier of the client assigned by the server.
    ///
    /// # Returns
    /// A WebSocket `Message` containing the serialized client ID.
    pub fn client_id(client_id: Box<str>) -> Message {
        let client_id = Self::ClientId { client_id };
        Message::text(serde_json::to_string(&client_id).unwrap())
    }

    /// Creates a WebSocket `Message` containing a relayed message from another client.
    ///
    /// This function serializes the `Message` variant and encapsulates it in a WebSocket
    /// text message for transmission.
    ///
    /// # Parameters
    /// - `sender_id`: The identifier of the client who sent the original message.
    /// - `msg`: The content of the message being relayed.
    ///
    /// # Returns
    /// A WebSocket `Message` containing the serialized relayed message.
    pub fn relayed_msg(sender_id: Box<str>, msg: Box<str>) -> Message {
        let msg = Self::Message { sender_id, msg };
        Message::text(serde_json::to_string(&msg).unwrap())
    }

    /// Creates a WebSocket `Message` containing an error message.
    ///
    /// This function serializes the `Error` variant and encapsulates it in a WebSocket
    /// text message for transmission to the client.
    ///
    /// # Parameters
    /// - `msg`: The error message to be sent to the client.
    ///
    /// # Returns
    /// A WebSocket `Message` containing the serialized error message.
    pub fn error(msg: &str) -> Message {
        let msg = Self::Error { msg: msg.into() };
        Message::text(serde_json::to_string(&msg).unwrap())
    }
}
