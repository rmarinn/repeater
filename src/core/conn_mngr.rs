use super::{Command, ServerMessage, WsStreamHandler};
use futures::{stream::SplitSink, SinkExt, StreamExt};
use rand::Rng;
use std::{collections::HashMap, iter, sync::Arc};
use tokio::{
    net::TcpStream,
    sync::{broadcast, mpsc},
    task::JoinHandle,
};
use tokio_tungstenite::{tungstenite::Message, WebSocketStream};

/// Represents a connected client, managing their WebSocket sender and task handle.
///
/// The `Client` struct holds the WebSocket sender (`ws_tx`) that is used to send
/// messages to the client, and a task handle (`task_handle`) that represents the
/// asynchronous task managing the WebSocket connection.
struct Client {
    /// The WebSocket sender for sending messages to the client.
    pub ws_tx: SplitSink<WebSocketStream<TcpStream>, Message>,

    /// The handle for the asynchronous task managing the WebSocket connection.
    pub task_handle: JoinHandle<()>,
}

impl Client {
    /// Sends a message to the client via the WebSocket connection.
    ///
    /// # Parameters
    /// - `msg`: The WebSocket `Message` to send.
    ///
    /// # Panics
    /// This function will panic if sending the message fails.
    pub async fn send(&mut self, msg: Message) {
        self.ws_tx.send(msg).await.unwrap();
    }
}

/// Manages active WebSocket connections and processes commands from clients.
///
/// The `ConnectionManager` is responsible for registering new clients, relaying messages
/// between clients, handling client disconnections, and shutting down all connections
/// when required.
pub struct ConnectionManager {
    /// A map storing active clients by their unique identifier.
    clients: HashMap<Arc<str>, Client>,

    /// The sender channel for dispatching commands to the connection manager.
    cmd_tx: mpsc::Sender<Command>,

    /// The receiver channel for receiving commands.
    cmd_rx: mpsc::Receiver<Command>,

    /// The receiver channel for receiving shutdown signals.
    shutdown_rx: broadcast::Receiver<()>,
}

impl ConnectionManager {
    /// Spawns a new `ConnectionManager` and returns its command sender and task handle.
    ///
    /// # Parameters
    /// - `shutdown_rx`: A broadcast receiver used to signal shutdown.
    ///
    /// # Returns
    /// A tuple containing the command sender and the handle to the connection manager task.
    pub fn spawn(shutdown_rx: broadcast::Receiver<()>) -> (mpsc::Sender<Command>, JoinHandle<()>) {
        let (cmd_tx, cmd_rx) = mpsc::channel::<Command>(100);
        let mut mngr = Self {
            clients: HashMap::new(),
            cmd_tx: cmd_tx.clone(),
            cmd_rx,
            shutdown_rx,
        };
        let handle = tokio::spawn(async move { mngr.run().await });
        (cmd_tx, handle)
    }

    /// The main loop that runs the `ConnectionManager`, processing commands and handling shutdown.
    ///
    /// This loop listens for incoming commands or shutdown signals and responds appropriately.
    async fn run(&mut self) {
        loop {
            tokio::select! {
                Some(cmd) = self.cmd_rx.recv() => self.handle_command(cmd).await,
                _ = self.shutdown_rx.recv() => {
                    println!("");
                    self.shutdown_clients().await;
                    break;
                }
            }
        }
    }

    /// Handles individual commands received by the connection manager.
    ///
    /// # Parameters
    /// - `cmd`: The command to process, which may involve relaying messages,
    /// registering/unregistering clients, or sending error messages.
    async fn handle_command(&mut self, cmd: Command) {
        match cmd {
            Command::RelayMessage {
                sender_id,
                recvr_id,
                msg,
            } => self.relay_msg(sender_id, recvr_id, msg).await,
            Command::Register(ws_stream) => self.register_connection(ws_stream).await,
            Command::Unregister(client_id) => self.unregister_connection(client_id),
            Command::ErrorMessage { client_id, msg } => self.send_err_msg(client_id, msg).await,
        }
    }

    /// Registers a new client and assigns them a unique identifier.
    ///
    /// This function splits the WebSocket stream, generates a unique client ID,
    /// and spawns a new task to handle the WebSocket connection for the client.
    ///
    /// # Parameters
    /// - `ws_stream`: The WebSocket stream associated with the new client.
    async fn register_connection(&mut self, ws_stream: WebSocketStream<TcpStream>) {
        let (ws_tx, ws_rx) = ws_stream.split();

        // find a free client_id
        let mut client_id = Self::generate_rand_id();
        while self.clients.contains_key(&client_id) {
            client_id = Self::generate_rand_id();
        }

        // start a process to handle the ws connection
        let task_handle = WsStreamHandler::spawn(ws_rx, client_id.clone(), self.cmd_tx.clone());

        let mut client = Client { ws_tx, task_handle };

        // send the client id
        client
            .send(ServerMessage::client_id(
                client_id.clone().to_string().into_boxed_str(),
            ))
            .await;

        // store the client info
        self.clients.insert(client_id.clone(), client);

        println!("{} connected", client_id);
    }

    /// Relays a message from one client to another.
    ///
    /// If the recipient exists, the message is forwarded; otherwise, an error message
    /// is sent back to the sender.
    ///
    /// # Parameters
    /// - `sender_id`: The ID of the client sending the message.
    /// - `recvr_id`: The ID of the client intended to receive the message.
    /// - `msg`: The message content.
    async fn relay_msg(&mut self, sender_id: Arc<str>, recvr_id: Box<str>, msg: Box<str>) {
        if let Some(recvr) = self.clients.get_mut(&*recvr_id) {
            // if there is a recipient, relay the message
            recvr
                .send(ServerMessage::relayed_msg(
                    sender_id.to_string().into_boxed_str(),
                    msg,
                ))
                .await;
        } else if let Some(sender) = self.clients.get_mut(&sender_id) {
            // if there is no recipient, send an error message
            sender
                .send(ServerMessage::error("Recipient not found"))
                .await;
        }
    }

    /// Unregisters a client and removes their connection.
    ///
    /// # Parameters
    /// - `client_id`: The ID of the client to be unregistered.
    fn unregister_connection(&mut self, client_id: Arc<str>) {
        self.clients.remove(&client_id);
        println!("{client_id} unregistered");
    }

    /// Sends an error message to a specific client.
    ///
    /// # Parameters
    /// - `client_id`: The ID of the client to receive the error message.
    /// - `msg`: The error message content.
    async fn send_err_msg(&mut self, client_id: Arc<str>, msg: Box<str>) {
        if let Some(client) = self.clients.get_mut(&client_id) {
            client.send(ServerMessage::error(&msg)).await;
        }
    }

    /// Shuts down all active client connections gracefully.
    ///
    /// This function sends a shutdown message to each client and then aborts
    /// the tasks managing their WebSocket connections.
    async fn shutdown_clients(&mut self) {
        for (client_id, mut client) in self.clients.drain() {
            println!("Disconnecting client {}", client_id);
            let _ = client
                .send(Message::Text("Server is closing".to_string()))
                .await;
            let _ = client.send(Message::Close(None)).await;
            client.task_handle.abort();
        }
        println!("All clients disconnected");
    }

    /// Generates a random client identifier.
    ///
    /// The ID is an 8-character string consisting of uppercase letters.
    ///
    /// # Returns
    /// A unique `Arc<str>` representing the client ID.
    fn generate_rand_id() -> Arc<str> {
        const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";
        const CLIENT_ID_LEN: usize = 8;
        let rand_char = || CHARSET[rand::thread_rng().gen_range(0..CHARSET.len())] as char;
        let id: String = iter::repeat_with(rand_char).take(CLIENT_ID_LEN).collect();
        id.into()
    }
}
