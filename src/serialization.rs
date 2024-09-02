use serde::{Deserialize, Serialize};
use tokio_tungstenite::tungstenite::Message;

#[derive(Serialize, Deserialize)]
pub enum ClientMessage {
    ClientId { client_id: u32 },
    Message { recvr_id: u32, msg: String },
    Error { msg: String },
    Disconnect,
}

impl ClientMessage {
    pub fn from(msg: Message) -> Self {
        match msg {
            Message::Text(msg) => serde_json::from_str::<Self>(&msg).unwrap(),
            Message::Close(_) => Self::Disconnect,
            _ => todo!(),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub enum ServerMessage {
    ClientId { client_id: u32 },
    Message { sender_id: u32, msg: String },
    Error { msg: String },
}

impl ServerMessage {
    pub fn client_id(client_id: u32) -> Message {
        let client_id = Self::ClientId { client_id };
        Message::text(serde_json::to_string(&client_id).unwrap())
    }

    pub fn relayed_msg(sender_id: u32, msg: String) -> Message {
        let msg = Self::Message { sender_id, msg };
        Message::text(serde_json::to_string(&msg).unwrap())
    }

    pub fn error(msg: String) -> Message {
        let msg = Self::Error { msg };
        Message::text(serde_json::to_string(&msg).unwrap())
    }
}
