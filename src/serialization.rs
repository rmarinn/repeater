use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio_tungstenite::tungstenite::Message;

#[derive(Serialize, Deserialize)]
pub enum ClientMessage {
    ClientId { client_id: Box<str> },
    Message { recvr_id: Box<str>, msg: Box<str> },
    Error { msg: Box<str> },
    Disconnect,
}

impl ClientMessage {
    pub fn from(msg: Message) -> Result<Self> {
        let msg = match msg {
            Message::Text(msg) => serde_json::from_str::<Self>(&msg)?,
            Message::Close(_) => Self::Disconnect,
            _ => todo!(),
        };
        Ok(msg)
    }
}

#[derive(Serialize, Deserialize)]
pub enum ServerMessage {
    ClientId { client_id: Box<str> },
    Message { sender_id: Box<str>, msg: Box<str> },
    Error { msg: Box<str> },
}

impl ServerMessage {
    pub fn client_id(client_id: Box<str>) -> Message {
        let client_id = Self::ClientId { client_id };
        Message::text(serde_json::to_string(&client_id).unwrap())
    }

    pub fn relayed_msg(sender_id: Box<str>, msg: Box<str>) -> Message {
        let msg = Self::Message { sender_id, msg };
        Message::text(serde_json::to_string(&msg).unwrap())
    }

    pub fn error(msg: &str) -> Message {
        let msg = Self::Error { msg: msg.into() };
        Message::text(serde_json::to_string(&msg).unwrap())
    }
}
