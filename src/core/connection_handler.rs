use super::{serialization::ServerMessage, ClientMessage, Command};
use anyhow::{anyhow, Result};
use futures::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use std::sync::Arc;
use tokio::{
    net::TcpStream,
    sync::{mpsc, oneshot},
};
use tokio_tungstenite::{tungstenite::Message, WebSocketStream};

/// A WebSocket stream handler responsible for processing incoming messages.
///
/// The `WsStreamHandler` listens for WebSocket messages, validates their format,
/// and forwards correctly structured commands to the `CommandManager` for execution.
///
/// # Fields
/// - `client_id`: An identifier for the client, shared across threads.
/// - `cmd_tx`: A channel sender used to transmit validated commands to the `CommandManager`.
pub struct ConnectionHandler {
    msg_tx: mpsc::Sender<Message>,
}

type WsReceiver = SplitStream<WebSocketStream<TcpStream>>;
type WsSender = SplitSink<WebSocketStream<TcpStream>, Message>;
type MngrCmdSender = mpsc::Sender<Command>;
type ClientId = Arc<str>;

impl ConnectionHandler {
    pub fn spawn(
        ws_stream: WebSocketStream<TcpStream>,
        mngr_tx: MngrCmdSender,
        client_id: ClientId,
    ) -> Self {
        let (ws_tx, ws_rx) = ws_stream.split();
        let (msg_tx, msg_rx) = mpsc::channel(10);

        // spawn a thread that will listen for incoming messages
        tokio::spawn(Self::listen(ws_rx, mngr_tx, client_id, msg_tx.clone()));

        // spawn a thread that will wait for outgoing messages
        tokio::spawn(Self::wait_for_msgs(msg_rx, ws_tx));

        Self { msg_tx }
    }

    pub async fn send_message(&mut self, msg: Message) -> Result<()> {
        self.msg_tx.send(msg).await.map_err(|e| {
            anyhow!(
                "Error communicating with the connection handler thread: {:?}",
                e
            )
        })
    }

    // waits for incoming messages
    async fn listen(
        mut ws_rx: WsReceiver,
        mngr_tx: MngrCmdSender,
        client_id: ClientId,
        msg_tx: mpsc::Sender<Message>,
    ) {
        while let Some(msg) = ws_rx.next().await {
            let process_result = match msg {
                Ok(msg) => Self::process_msg(msg, &mngr_tx, &msg_tx, &client_id).await,
                Err(e) => {
                    eprintln!("An error occurred while receiving ws message: {}", e);
                    break;
                }
            };

            if let Err(e) = process_result {
                eprintln!("Error processing websocket message: {:?}", e);
                break;
            }
        }

        println!("closed ws listener");
    }

    /// waits for outgoing messages
    async fn wait_for_msgs(mut msg_rx: mpsc::Receiver<Message>, mut ws_tx: WsSender) {
        while let Some(msg) = msg_rx.recv().await {
            if let Err(e) = ws_tx.send(msg.clone()).await {
                eprintln!("Error sending websocket message: {:?}", e);
            }
        }
    }

    /// processes incoming messages
    async fn process_msg(
        msg: Message,
        mngr_tx: &MngrCmdSender,
        msg_tx: &mpsc::Sender<Message>,
        client_id: &Arc<str>,
    ) -> Result<()> {
        // deserialize message
        let msg = match ClientMessage::from(msg) {
            Ok(msg) => msg,
            Err(e) => {
                let err_msg = format!("Error deserializing message from {}: {:?}", client_id, e);
                eprintln!("{}", err_msg);
                let msg = ServerMessage::error(&err_msg);
                Self::send_ws_msg(msg_tx, msg).await?;
                return Ok(());
            }
        };

        match msg {
            ClientMessage::Message { recipient_id, msg } => {
                Self::relay_msg(recipient_id, msg, &client_id, &mngr_tx, &msg_tx).await?
            }
            ClientMessage::Disconnect => Self::disconnect(&mngr_tx, &client_id).await?,
            _ => (),
        };

        Ok(())
    }

    /// ask manager to relay the sent message to the target client
    async fn relay_msg(
        recipient_id: Box<str>,
        msg: Box<str>,
        client_id: &Arc<str>,
        mngr_tx: &MngrCmdSender,
        msg_tx: &mpsc::Sender<Message>,
    ) -> Result<()> {
        let (result_tx, result_rx) = oneshot::channel();

        let msg = ServerMessage::relayed_msg(client_id.to_string().into_boxed_str(), msg);
        let cmd = Command::RelayMessage {
            client_id: recipient_id.clone(),
            msg,
            result_tx,
        };

        Self::send_cmd_to_mngr(mngr_tx, cmd).await?;

        match result_rx.await {
            Ok(result) => {
                if let Err(e) = result {
                    // send an error message back to the sender
                    let msg = ServerMessage::error(format!("{:?}", e).as_str().into());
                    Self::send_ws_msg(msg_tx, msg).await?;
                }
            }
            Err(e) => Err(anyhow!(
                "Error receiving response from the manager: {:?}",
                e
            ))?,
        }

        Ok(())
    }

    /// unregisters the client on the connection manager
    async fn disconnect(mngr_tx: &MngrCmdSender, client_id: &Arc<str>) -> Result<()> {
        println!("{} disconnected", client_id);

        let cmd = Command::Unregister(client_id.clone().into());
        Self::send_cmd_to_mngr(mngr_tx, cmd).await?;

        Ok(())
    }

    async fn send_cmd_to_mngr(mngr_tx: &MngrCmdSender, cmd: Command) -> Result<()> {
        mngr_tx
            .send(cmd)
            .await
            .map_err(|e| anyhow!("Error communicating with the connection manager: {:?}", e))
    }

    async fn send_ws_msg(msg_tx: &mpsc::Sender<Message>, msg: Message) -> Result<()> {
        msg_tx
            .send(msg)
            .await
            .map_err(|e| anyhow!("Error communicating with the ws message sender: {:?}", e))
    }
}
