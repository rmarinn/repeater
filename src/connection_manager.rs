use std::collections::HashMap;

use futures::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use tokio::{
    net::TcpStream,
    sync::{
        broadcast,
        mpsc::{self},
    },
    task::JoinHandle,
};
use tokio_tungstenite::{tungstenite::Message, WebSocketStream};

use crate::serialization::{ClientMessage, ServerMessage};

pub enum Command {
    Register(WebSocketStream<TcpStream>),
    Unregister(u32),
    RelayMessage {
        sender_id: u32,
        recvr_id: u32,
        msg: String,
    },
}

/// Manages WebSocket connections and handles commands like message relaying.
pub struct ConnectionManager {
    pub cmd_tx: mpsc::Sender<Command>,
    pub handle: JoinHandle<()>,
}

impl ConnectionManager {
    /// Starts a background process that handles commands
    pub fn start(shutdown_rx: broadcast::Receiver<()>) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel::<Command>(100);
        let handle = tokio::spawn(Self::run(cmd_rx, cmd_tx.clone(), shutdown_rx));
        Self { cmd_tx, handle }
    }

    async fn run(
        mut cmd_rx: mpsc::Receiver<Command>,
        cmd_tx: mpsc::Sender<Command>,
        mut shutdown_rx: broadcast::Receiver<()>,
    ) {
        let mut id_counter = 1u32;
        let mut clients: HashMap<u32, SplitSink<WebSocketStream<TcpStream>, Message>> =
            HashMap::new();

        loop {
            tokio::select! {
                Some(cmd) = cmd_rx.recv() => Self::handle_command(cmd, &mut clients, &mut id_counter, &cmd_tx).await,
                _ = shutdown_rx.recv() => {
                    println!("Received shutdown signal");
                    Self::shutdown_clients(&mut clients).await;
                    break;
                }
            }
        }
    }

    async fn handle_command(
        cmd: Command,
        clients: &mut HashMap<u32, SplitSink<WebSocketStream<TcpStream>, Message>>,
        id_counter: &mut u32,
        cmd_tx: &mpsc::Sender<Command>,
    ) {
        match cmd {
            Command::RelayMessage {
                sender_id,
                recvr_id,
                msg,
            } => {
                if let Some(recvr_tx) = clients.get_mut(&recvr_id) {
                    recvr_tx
                        .send(ServerMessage::relayed_msg(sender_id, msg))
                        .await
                        .unwrap();
                } else if let Some(sender_tx) = clients.get_mut(&sender_id) {
                    sender_tx
                        .send(ServerMessage::error("Recipient not found".to_string()))
                        .await
                        .unwrap();
                }
            }
            Command::Register(ws_stream) => {
                let (mut ws_sink, ws_stream) = ws_stream.split();
                let client_id = *id_counter;
                ws_sink
                    .send(ServerMessage::client_id(client_id))
                    .await
                    .unwrap();
                clients.insert(client_id, ws_sink);
                *id_counter += 1;
                Self::handle_ws_stream(client_id, ws_stream, cmd_tx.clone());
            }
            Command::Unregister(client_id) => {
                clients.remove(&client_id);
            }
        }
    }

    fn handle_ws_stream(
        client_id: u32,
        stream: SplitStream<WebSocketStream<TcpStream>>,
        sender: mpsc::Sender<Command>,
    ) {
        tokio::spawn(async move {
            let mut stream = stream;
            while let Some(msg) = stream.next().await {
                match msg {
                    Ok(msg) => {
                        let client_msg = ClientMessage::from(msg);
                        if let ClientMessage::Message { recvr_id, msg } = client_msg {
                            if sender
                                .send(Command::RelayMessage {
                                    sender_id: client_id,
                                    recvr_id,
                                    msg,
                                })
                                .await
                                .is_err()
                            {
                                break;
                            }
                        } else if let ClientMessage::Disconnect = client_msg {
                            if sender.send(Command::Unregister(client_id)).await.is_err() {
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        println!("An error occurred: {}", e);
                        break;
                    }
                }
            }
        });
    }

    async fn shutdown_clients(
        clients: &mut HashMap<u32, SplitSink<WebSocketStream<TcpStream>, Message>>,
    ) {
        for (client_id, mut ws_sink) in clients.drain() {
            println!("Disconnecting client {}", client_id);
            let _ = ws_sink
                .send(Message::Text("Server is closing".to_string()))
                .await;
            let _ = ws_sink.send(Message::Close(None)).await;
        }
        println!("All clients disconnected");
    }
}
