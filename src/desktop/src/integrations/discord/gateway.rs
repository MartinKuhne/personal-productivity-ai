//! Discord Gateway WebSocket connection.

use anyhow::Result;
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message as WsMessage};

/// Gateway opcodes.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[repr(u8)]
pub enum OpCode {
    Dispatch = 0,
    Heartbeat = 1,
    Identify = 2,
    PresenceUpdate = 3,
    VoiceStateUpdate = 4,
    Resume = 6,
    Reconnect = 7,
    RequestGuildMembers = 8,
    InvalidSession = 9,
    Hello = 10,
    HeartbeatAck = 11,
}

/// Gateway payload.
#[derive(Debug, Serialize, Deserialize)]
pub struct GatewayPayload {
    pub op: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub d: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub s: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub t: Option<String>,
}

/// Gateway Hello payload.
#[derive(Debug, Deserialize)]
pub struct HelloData {
    pub heartbeat_interval: u64,
}

/// Gateway Ready payload.
#[derive(Debug, Deserialize)]
pub struct ReadyData {
    pub v: u32,
    pub user: User,
    pub session_id: String,
    pub resume_gateway_url: String,
}

/// Discord User object.
#[derive(Debug, Deserialize)]
pub struct User {
    pub id: String,
    pub username: String,
    pub discriminator: String,
    pub avatar: Option<String>,
    pub bot: Option<bool>,
}

/// Discord Message Create event.
#[derive(Debug, Deserialize)]
pub struct MessageCreate {
    pub id: String,
    pub channel_id: String,
    pub guild_id: Option<String>,
    pub author: User,
    pub content: String,
    pub timestamp: String,
    pub mentioned_users: Option<Vec<User>>,
    pub mention_everyone: Option<bool>,
}

/// Discord Interaction Create event.
#[derive(Debug, Deserialize)]
pub struct InteractionCreate {
    pub id: String,
    pub application_id: String,
    #[serde(rename = "type")]
    pub interaction_type: u8,
    pub data: Option<InteractionData>,
    pub guild_id: Option<String>,
    pub channel_id: String,
    pub token: String,
    pub version: u8,
}

/// Interaction data for slash commands.
#[derive(Debug, Deserialize)]
pub struct InteractionData {
    pub id: String,
    pub name: String,
    pub options: Option<Vec<InteractionOption>>,
}

/// Interaction option for slash commands.
#[derive(Debug, Deserialize)]
pub struct InteractionOption {
    pub name: String,
    #[serde(rename = "type")]
    pub option_type: u8,
    pub value: Option<serde_json::Value>,
}

/// Gateway event types.
#[derive(Debug)]
pub enum GatewayEvent {
    Ready(ReadyData),
    MessageCreate(MessageCreate),
    InteractionCreate(InteractionCreate),
    HeartbeatAck,
    InvalidSession(bool),
    Reconnect,
    Unknown(String),
}

/// Gateway client.
pub struct GatewayClient {
    bot_token: String,
    intents: u64,
    session_id: Option<String>,
    sequence: Option<u64>,
    heartbeat_interval: u64,
    ws_sender: Option<mpsc::UnboundedSender<WsMessage>>,
    event_sender: mpsc::UnboundedSender<GatewayEvent>,
}

impl GatewayClient {
    pub fn new(bot_token: String, event_sender: mpsc::UnboundedSender<GatewayEvent>) -> Self {
        // Default intents: GUILDS | GUILD_MESSAGES | MESSAGE_CONTENT | GUILD_MESSAGE_REACTIONS
        let intents = (1 << 0) | (1 << 9) | (1 << 15) | (1 << 10);
        Self {
            bot_token,
            intents,
            session_id: None,
            sequence: None,
            heartbeat_interval: 41250,
            ws_sender: None,
            event_sender,
        }
    }

    /// Connect to the Gateway and start processing events.
    pub async fn connect(&mut self) -> Result<()> {
        let url = "wss://gateway.discord.gg/?v=10&encoding=json";
        let (ws_stream, _) = connect_async(url).await?;
        let (mut write, mut read) = ws_stream.split();

        let (tx, mut rx) = mpsc::unbounded_channel::<WsMessage>();
        self.ws_sender = Some(tx.clone());

        // Spawn writer task
        let write_task = tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if write.send(msg).await.is_err() {
                    break;
                }
            }
        });

        // Send Identify
        self.send_identify().await?;

        // Process incoming messages
        while let Some(msg) = read.next().await {
            match msg {
                Ok(WsMessage::Text(text)) => {
                    if let Err(e) = self.handle_message(&text).await {
                        tracing::error!(name = "discord.gateway.error", error = %e, "Gateway message handling error");
                    }
                }
                Ok(WsMessage::Close(_)) => {
                    tracing::info!(name = "discord.gateway.close", "Gateway connection closed");
                    break;
                }
                Err(e) => {
                    tracing::error!(name = "discord.gateway.error", error = %e, "Gateway read error");
                    break;
                }
                _ => {}
            }
        }

        write_task.abort();
        Ok(())
    }

    async fn send_identify(&mut self) -> Result<()> {
        let identify = serde_json::json!({
            "op": OpCode::Identify as u8,
            "d": {
                "token": self.bot_token,
                "intents": self.intents,
                "properties": {
                    "os": "linux",
                    "browser": "fastmd",
                    "device": "fastmd"
                }
            }
        });
        self.send(identify).await
    }

    async fn send(&self, payload: serde_json::Value) -> Result<()> {
        if let Some(sender) = &self.ws_sender {
            let text = serde_json::to_string(&payload)?;
            sender.send(WsMessage::Text(text)).ok();
        }
        Ok(())
    }

    async fn handle_message(&mut self, text: &str) -> Result<()> {
        let payload: GatewayPayload = serde_json::from_str(text)?;

        // Update sequence number
        if let Some(s) = payload.s {
            self.sequence = Some(s);
        }

        match payload.op {
            0 => self.handle_dispatch(payload).await?,
            1 => self.handle_heartbeat().await?,
            7 => self.handle_reconnect().await?,
            9 => self.handle_invalid_session(payload).await?,
            10 => self.handle_hello(payload).await?,
            11 => self.handle_heartbeat_ack().await?,
            _ => {}
        }
        Ok(())
    }

    async fn handle_dispatch(&mut self, payload: GatewayPayload) -> Result<()> {
        let event_type = payload.t.unwrap_or_default();
        let data = payload.d.unwrap_or_default();

        match event_type.as_str() {
            "READY" => {
                let ready: ReadyData = serde_json::from_value(data)?;
                self.session_id = Some(ready.session_id.clone());
                self.heartbeat_interval = 41250; // Will be updated from Hello
                self.event_sender.send(GatewayEvent::Ready(ready)).ok();
            }
            "MESSAGE_CREATE" => {
                let msg: MessageCreate = serde_json::from_value(data)?;
                self.event_sender
                    .send(GatewayEvent::MessageCreate(msg))
                    .ok();
            }
            "INTERACTION_CREATE" => {
                let interaction: InteractionCreate = serde_json::from_value(data)?;
                self.event_sender
                    .send(GatewayEvent::InteractionCreate(interaction))
                    .ok();
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_hello(&mut self, payload: GatewayPayload) -> Result<()> {
        let hello: HelloData = serde_json::from_value(payload.d.unwrap_or_default())?;
        self.heartbeat_interval = hello.heartbeat_interval;

        // Start heartbeat loop
        let interval = self.heartbeat_interval;
        let sender = self.ws_sender.clone();
        let seq = self.sequence;

        tokio::spawn(async move {
            let mut interval_timer =
                tokio::time::interval(tokio::time::Duration::from_millis(interval));
            loop {
                interval_timer.tick().await;
                if let Some(sender) = &sender {
                    let heartbeat = serde_json::json!({
                        "op": OpCode::Heartbeat as u8,
                        "d": seq
                    });
                    let text = serde_json::to_string(&heartbeat).ok();
                    if let Some(text) = text {
                        sender.send(WsMessage::Text(text)).ok();
                    }
                }
            }
        });
        Ok(())
    }

    async fn handle_heartbeat(&self) -> Result<()> {
        // Discord sent us a heartbeat request, respond with ACK
        let ack = serde_json::json!({
            "op": OpCode::HeartbeatAck as u8
        });
        self.send(ack).await
    }

    async fn handle_heartbeat_ack(&self) -> Result<()> {
        // Heartbeat acknowledged
        Ok(())
    }

    async fn handle_reconnect(&mut self) -> Result<()> {
        self.event_sender.send(GatewayEvent::Reconnect).ok();
        Ok(())
    }

    async fn handle_invalid_session(&mut self, payload: GatewayPayload) -> Result<()> {
        let resumable = payload
            .d
            .as_ref()
            .and_then(|d| d.as_bool())
            .unwrap_or(false);
        self.event_sender
            .send(GatewayEvent::InvalidSession(resumable))
            .ok();
        if !resumable {
            self.session_id = None;
            self.sequence = None;
        }
        Ok(())
    }

    /// Resume a previous session.
    pub async fn resume(&mut self) -> Result<()> {
        if let (Some(session_id), Some(seq)) = (&self.session_id, self.sequence) {
            let resume = serde_json::json!({
                "op": OpCode::Resume as u8,
                "d": {
                    "token": self.bot_token,
                    "session_id": session_id,
                    "seq": seq
                }
            });
            self.send(resume).await?;
        }
        Ok(())
    }
}
