use super::{ClientMessage, Command};
use futures::{stream::SplitStream, StreamExt};
use std::sync::Arc;
use tokio::{net::TcpStream, sync::mpsc, task::JoinHandle};
use tokio_tungstenite::WebSocketStream;

/// A WebSocket stream handler responsible for processing incoming messages.
///
/// The `WsStreamHandler` listens for WebSocket messages, validates their format,
/// and forwards correctly structured commands to the `CommandManager` for execution.
///
/// # Fields
/// - `client_id`: An identifier for the client, shared across threads.
/// - `cmd_tx`: A channel sender used to transmit validated commands to the `CommandManager`.
pub struct WsStreamHandler {
    client_id: Arc<str>,
    cmd_tx: mpsc::Sender<Command>,
}

impl WsStreamHandler {
    pub fn spawn(
        mut ws_rx: SplitStream<WebSocketStream<TcpStream>>,
        client_id: Arc<str>,
        cmd_tx: mpsc::Sender<Command>,
    ) -> JoinHandle<()> {
        let handler = Self { client_id, cmd_tx };
        tokio::spawn(async move {
            while let Some(msg) = ws_rx.next().await {
                match msg {
                    Ok(msg) => {
                        // deserialize then process message
                        match ClientMessage::from(msg) {
                            Ok(msg) => handler.process_msg(msg).await,
                            Err(e) => {
                                handler
                                    .error(format!("Error deserializing client's message: {}", e))
                                    .await
                            }
                        }
                    }
                    Err(e) => {
                        println!("An error occurred while receiving ws message: {}", e);
                        break;
                    }
                }
            }
        })
    }

    async fn process_msg(&self, msg: ClientMessage) {
        match msg {
            ClientMessage::Message { recvr_id, msg } => self.relay_msg(recvr_id, msg).await,
            ClientMessage::Disconnect => self.disconnect().await,
            _ => (),
        };
    }

    async fn relay_msg(&self, recvr_id: Box<str>, msg: Box<str>) {
        self.cmd_tx
            .send(Command::RelayMessage {
                sender_id: self.client_id.clone(),
                recvr_id: recvr_id.clone(),
                msg,
            })
            .await
            .unwrap();
    }

    async fn disconnect(&self) {
        println!("{} disconnected", self.client_id);
        self.cmd_tx
            .send(Command::Unregister(self.client_id.clone()))
            .await
            .unwrap();
    }

    async fn error(&self, msg: String) {
        self.cmd_tx
            .send(Command::ErrorMessage {
                client_id: self.client_id.clone(),
                msg: msg.into(),
            })
            .await
            .unwrap();
    }
}
