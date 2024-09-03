use std::{collections::HashMap, iter, sync::Arc};

use futures::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use rand::Rng;
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
    Unregister(Arc<str>),
    RelayMessage {
        sender_id: Arc<str>,
        recvr_id: Box<str>,
        msg: Box<str>,
    },
    ErrorMessage {
        client_id: Arc<str>,
        msg: Box<str>,
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
        let mut clients: HashMap<Arc<str>, SplitSink<WebSocketStream<TcpStream>, Message>> =
            HashMap::new();

        loop {
            tokio::select! {
                Some(cmd) = cmd_rx.recv() => Self::handle_command(cmd, &mut clients, &cmd_tx).await,
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
        clients: &mut HashMap<Arc<str>, SplitSink<WebSocketStream<TcpStream>, Message>>,
        cmd_tx: &mpsc::Sender<Command>,
    ) {
        match cmd {
            Command::RelayMessage {
                sender_id,
                recvr_id,
                msg,
            } => {
                if let Some(recvr_tx) = clients.get_mut(&*recvr_id) {
                    // if there is a recipient, relay the message
                    recvr_tx
                        .send(ServerMessage::relayed_msg(
                            sender_id.to_string().into_boxed_str(),
                            msg,
                        ))
                        .await
                        .unwrap();
                } else if let Some(sender_tx) = clients.get_mut(&sender_id) {
                    // if there is no recipient, send an error message
                    sender_tx
                        .send(ServerMessage::error("Recipient not found"))
                        .await
                        .unwrap();
                }
            }
            Command::Register(ws_stream) => {
                let (mut ws_sink, ws_stream) = ws_stream.split();

                // find a free client_id
                let mut client_id = Self::generate_rand_id();
                while clients.contains_key(&client_id) {
                    client_id = Self::generate_rand_id();
                }

                // send the client id
                ws_sink
                    .send(ServerMessage::client_id(
                        client_id.clone().to_string().into_boxed_str(),
                    ))
                    .await
                    .unwrap();

                // store the client id
                clients.insert(client_id.clone(), ws_sink);
                println!("registered new connection as {client_id}");

                // start a process to handle the connection
                Self::handle_ws_stream(client_id, ws_stream, cmd_tx.clone());
            }
            Command::Unregister(client_id) => {
                clients.remove(&client_id);
                println!("{client_id} unregistered");
            }
            Command::ErrorMessage { client_id, msg } => {
                if let Some(ws_tx) = clients.get_mut(&client_id) {
                    ws_tx.send(ServerMessage::error(&msg)).await.unwrap();
                }
            }
        }
    }

    fn handle_ws_stream(
        client_id: Arc<str>,
        stream: SplitStream<WebSocketStream<TcpStream>>,
        sender: mpsc::Sender<Command>,
    ) {
        tokio::spawn(async move {
            let mut stream = stream;
            while let Some(msg) = stream.next().await {
                match msg {
                    Ok(msg) => {
                        let client_msg = ClientMessage::from(msg); // deserialize message
                        match client_msg {
                            Ok(msg) => match msg {
                                ClientMessage::Message { recvr_id, msg } => {
                                    if sender
                                        .send(Command::RelayMessage {
                                            sender_id: client_id.clone(),
                                            recvr_id: recvr_id.clone(),
                                            msg,
                                        })
                                        .await
                                        .is_err()
                                    {
                                        break;
                                    }
                                }
                                ClientMessage::Disconnect => {
                                    println!("{client_id} disconnected");
                                    if sender
                                        .send(Command::Unregister(client_id.clone()))
                                        .await
                                        .is_err()
                                    {
                                        break;
                                    }
                                }
                                _ => (),
                            },
                            Err(e) => sender
                                .send(Command::ErrorMessage {
                                    client_id: client_id.clone(),
                                    msg: format!("Malformed request: {e}").into(),
                                })
                                .await
                                .unwrap(),
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
        clients: &mut HashMap<Arc<str>, SplitSink<WebSocketStream<TcpStream>, Message>>,
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

    fn generate_rand_id() -> Arc<str> {
        const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";
        const CLIENT_ID_LEN: usize = 8;
        let rand_char = || CHARSET[rand::thread_rng().gen_range(0..CHARSET.len())] as char;
        let id: String = iter::repeat_with(rand_char).take(CLIENT_ID_LEN).collect();
        id.into()
    }
}
