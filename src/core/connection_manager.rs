use super::{Command, ConnectionHandler, ServerMessage};
use anyhow::{anyhow, Result};
use rand::Rng;
use std::{borrow::Cow, collections::HashMap, iter, sync::Arc};
use tokio::{
    net::TcpStream,
    sync::{mpsc, oneshot},
    task::JoinHandle,
};
use tokio_tungstenite::{
    tungstenite::{
        protocol::{frame::coding::CloseCode, CloseFrame},
        Message,
    },
    WebSocketStream,
};
use tokio_util::sync::CancellationToken;

/// Manages active WebSocket connections and processes client commands.
///
/// The `ConnectionManager` is responsible for registering new clients, relaying messages
/// between clients, handling client disconnections, and shutting down all connections
/// when required.
pub struct ConnectionManager {
    /// Stores active clients by their unique identifier.
    clients: HashMap<Arc<str>, ConnectionHandler>,

    /// Channel for dispatching commands to the connection manager.
    cmd_tx: mpsc::Sender<Command>,

    /// Channel for receiving commands from clients.
    cmd_rx: mpsc::Receiver<Command>,

    /// Token for managing graceful shutdowns.
    cancellation_token: CancellationToken,
}

impl ConnectionManager {
    /// Spawns a new `ConnectionManager` and returns its command sender and task handle.
    ///
    /// # Parameters
    /// - `cancellation_token`: Token to signal the shutdown of the connection manager.
    ///
    /// # Returns
    /// A tuple containing the command sender and the handle to the connection manager task.
    pub fn spawn(cancellation_token: CancellationToken) -> (mpsc::Sender<Command>, JoinHandle<()>) {
        let (cmd_tx, cmd_rx) = mpsc::channel::<Command>(100);
        let mut manager = Self {
            clients: HashMap::new(),
            cmd_tx: cmd_tx.clone(),
            cmd_rx,
            cancellation_token,
        };

        // Spawn the task that runs the connection manager
        let handle = tokio::spawn(async move { manager.run().await });

        (cmd_tx, handle)
    }

    /// The main loop of the `ConnectionManager`, processing commands and handling shutdown.
    ///
    /// This loop listens for incoming commands or cancellation signals and handles them accordingly.
    async fn run(&mut self) {
        loop {
            tokio::select! {
                Some(cmd) = self.cmd_rx.recv() => {
                    if let Err(e) = self.handle_command(cmd).await {
                        eprintln!("Error occured: {:?}", e);
                        break;
                    }
                },
                _ = self.cancellation_token.cancelled() => {
                    println!("Shutting down server...");
                    if let Err(e) = self.disconnect_clients().await {
                        eprintln!("Error during client disconnection: {:?}", e)
                    }
                    break;
                }
            }
        }
    }

    /// Processes individual commands received by the connection manager.
    ///
    /// # Parameters
    /// - `cmd`: A command to process, which can involve registering, unregistering, or relaying messages.
    async fn handle_command(&mut self, cmd: Command) -> Result<()> {
        match cmd {
            Command::RelayMessage {
                client_id,
                msg,
                result_tx,
            } => self.relay_msg(client_id, msg, result_tx).await,
            Command::Register(ws_stream) => self.register_connection(ws_stream).await,
            Command::Unregister(client_id) => self.unregister_connection(client_id),
        }
    }

    /// Registers a new client and assigns them a unique identifier.
    ///
    /// This function generates a unique client ID and spawns a new task to handle the WebSocket connection.
    ///
    /// # Parameters
    /// - `ws_stream`: The WebSocket stream of the new client.
    async fn register_connection(&mut self, ws_stream: WebSocketStream<TcpStream>) -> Result<()> {
        // Generate a unique client ID
        let mut client_id = Self::generate_rand_id();
        while self.clients.contains_key(&client_id) {
            client_id = Self::generate_rand_id();
        }

        // Spawn a task to handle the WebSocket connection
        let mut conn = ConnectionHandler::spawn(ws_stream, self.cmd_tx.clone(), client_id.clone());

        // Send the client ID to the client
        conn.send_message(ServerMessage::client_id(
            client_id.clone().to_string().into_boxed_str(),
        ))
        .await?;

        // Store the connection handler's info
        self.clients.insert(client_id.clone(), conn);

        println!("{} connected", client_id);
        Ok(())
    }

    /// Relays a message from one client to another.
    ///
    /// If the recipient exists, the message is forwarded; otherwise, an error message
    /// is sent back to the sender.
    ///
    /// # Parameters
    /// - `client_id`: The ID of the client receiving the message.
    /// - `msg`: The message content to be relayed.
    /// - `result_tx`: A channel to communicate the result of the relay operation.
    async fn relay_msg(
        &mut self,
        client_id: Box<str>,
        msg: Message,
        result_tx: oneshot::Sender<Result<()>>,
    ) -> Result<()> {
        let result = match self.clients.get_mut(&*client_id) {
            Some(client) => {
                // if there is a recipient, relay the message
                client.send_message(msg).await?;
                Ok(())
            }
            None => Err(anyhow!("Failed to send message: recipient not found")),
        };

        result_tx
            .send(result)
            .map_err(|e| anyhow!("Error communicating with the connection handler: {:?}", e))?;

        Ok(())
    }

    /// Unregisters a client and removes their connection.
    ///
    /// # Parameters
    /// - `client_id`: The ID of the client to be unregistered.
    fn unregister_connection(&mut self, client_id: Arc<str>) -> Result<()> {
        self.clients.remove(&client_id);
        println!("{} unregistered", client_id);
        Ok(())
    }

    /// Disconnects all active clients and shuts down their WebSocket connections.
    ///
    /// Sends a close frame to each client and removes them from the active client list.
    async fn disconnect_clients(&mut self) -> Result<()> {
        for (client_id, mut client) in self.clients.drain() {
            println!("Disconnecting {}", client_id);

            let close_frame = CloseFrame {
                code: CloseCode::Away,
                reason: Cow::Owned("Server is closing".to_string()),
            };
            let _ = client.send_message(Message::Close(Some(close_frame))).await;
        }
        println!("All clients disconnected");
        Ok(())
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
