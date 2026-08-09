//! Discord Gateway WebSocket connection.
//!
//! The [`GatewayClient`] is a thin handle that spawns a background
//! runner task (the private `GatewayRunner`). The runner owns the full
//! connection lifecycle: it opens the WebSocket, drives the read loop,
//! spawns a per-connection heartbeat task, and reconnects (resuming when
//! possible) on `Reconnect` / `InvalidSession` / socket close. This
//! keeps the bot's event loop free of blocking reconnects (see `bot.rs`).

use anyhow::Result;
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message as WsMessage};

/// Default Gateway URL (Discord v10, JSON encoding). Used for the initial
/// connection; the resume URL returned in `READY` is used thereafter.
const DEFAULT_GATEWAY_URL: &str = "wss://gateway.discord.gg/?v=10&encoding=json";

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
    #[serde(default)]
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
    #[serde(default, rename = "mentions")]
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

/// Gateway event types delivered to the bot.
#[derive(Debug)]
pub enum GatewayEvent {
    Ready(ReadyData),
    MessageCreate(MessageCreate),
    InteractionCreate(InteractionCreate),
    /// Gateway requested a reconnect (handled internally; emitted for
    /// observability only — the runner reconnects on its own).
    Reconnect,
    /// Invalid session (handled internally; emitted for observability).
    InvalidSession(bool),
    HeartbeatAck,
    Unknown(String),
}

/// Control flow returned by per-message handlers to the read loop.
enum LoopControl {
    Continue,
    /// Break the read loop and reconnect. `resumable` controls whether
    /// the runner sends Resume (true) or a fresh Identify (false).
    Reconnect {
        resumable: bool,
    },
}

/// Public handle to the gateway background runner.
pub struct GatewayClient {
    bot_token: String,
    intents: u64,
    event_sender: mpsc::UnboundedSender<GatewayEvent>,
    run_handle: Option<JoinHandle<()>>,
}

impl GatewayClient {
    /// Create a new gateway client.
    ///
    /// Default intents: `GUILDS | GUILD_MESSAGES | MESSAGE_CONTENT`.
    /// `GUILD_MESSAGE_REACTIONS` is intentionally not requested (the bot
    /// does not handle reactions).
    pub fn new(bot_token: String, event_sender: mpsc::UnboundedSender<GatewayEvent>) -> Self {
        // GUILDS (1<<0) | GUILD_MESSAGES (1<<9) | MESSAGE_CONTENT (1<<15)
        let intents = (1 << 0) | (1 << 9) | (1 << 15);
        Self {
            bot_token,
            intents,
            event_sender,
            run_handle: None,
        }
    }

    /// Start the background runner. Opens the first connection and fails
    /// fast (returning an error) if the initial socket cannot be opened;
    /// thereafter the runner reconnects on its own.
    pub async fn start(&mut self) -> Result<()> {
        let (init_tx, init_rx) = tokio::sync::oneshot::channel::<Result<()>>();
        let bot_token = self.bot_token.clone();
        let intents = self.intents;
        let event_sender = self.event_sender.clone();
        let handle = tokio::spawn(async move {
            let mut runner = GatewayRunner::new(bot_token, intents, event_sender);
            runner.run(init_tx).await;
        });
        self.run_handle = Some(handle);
        init_rx
            .await
            .map_err(|_| anyhow::anyhow!("gateway runner dropped before initial connect"))?
    }

    /// Stop the gateway, aborting its background task.
    pub fn shutdown(&mut self) {
        if let Some(h) = self.run_handle.take() {
            h.abort();
        }
    }

    /// Whether the gateway runner is currently active.
    pub fn is_running(&self) -> bool {
        self.run_handle
            .as_ref()
            .map(|h| !h.is_finished())
            .unwrap_or(false)
    }
}

impl Drop for GatewayClient {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Owns the mutable gateway session state and runs the reconnect loop.
struct GatewayRunner {
    bot_token: String,
    intents: u64,
    session_id: Option<String>,
    /// Last received sequence number, shared with the heartbeat task.
    sequence: Arc<Mutex<Option<u64>>>,
    resume_gateway_url: Option<String>,
    heartbeat_interval: u64,
    ws_sender: Option<mpsc::UnboundedSender<WsMessage>>,
    event_sender: mpsc::UnboundedSender<GatewayEvent>,
    heartbeat_handle: Option<JoinHandle<()>>,
    writer_handle: Option<JoinHandle<()>>,
}

impl GatewayRunner {
    fn new(
        bot_token: String,
        intents: u64,
        event_sender: mpsc::UnboundedSender<GatewayEvent>,
    ) -> Self {
        Self {
            bot_token,
            intents,
            session_id: None,
            sequence: Arc::new(Mutex::new(None)),
            resume_gateway_url: None,
            heartbeat_interval: 41250,
            ws_sender: None,
            event_sender,
            heartbeat_handle: None,
            writer_handle: None,
        }
    }

    /// Run the reconnect loop forever. `init` receives the result of the
    /// first connection attempt (for fail-fast semantics); subsequent
    /// failures are logged and retried with a backoff.
    async fn run(&mut self, init: tokio::sync::oneshot::Sender<Result<()>>) {
        let mut init = Some(init);
        let mut first = true;
        loop {
            let url = self
                .resume_gateway_url
                .clone()
                .unwrap_or_else(|| DEFAULT_GATEWAY_URL.to_string());
            let resumable = self.session_id.is_some() && self.sequence.lock().unwrap().is_some();
            let result = self.establish(&url, resumable).await;

            if first {
                if let Some(init) = init.take() {
                    let _ = init.send(
                        result
                            .as_ref()
                            .map(|_| ())
                            .map_err(|e| anyhow::anyhow!(e.to_string())),
                    );
                }
                first = false;
            }
            match &result {
                Ok(()) => tracing::info!("discord.gateway.session_ended",),
                Err(e) => tracing::error!(error = %e, "discord.gateway.session_error"),
            }
            // Backoff before reconnecting to avoid hot-looping on a persistent failure.
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }

    /// Open a WebSocket to `url`, send Identify or Resume, and drive the
    /// read loop until the connection closes or a reconnect is requested.
    async fn establish(&mut self, url: &str, resume: bool) -> Result<()> {
        let (ws, _) = connect_async(url).await?;
        let (mut write, mut read) = ws.split();

        let (tx, mut rx) = mpsc::unbounded_channel::<WsMessage>();
        self.ws_sender = Some(tx);

        let writer_handle = tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if write.send(msg).await.is_err() {
                    break;
                }
            }
        });
        self.writer_handle = Some(writer_handle);

        if resume {
            self.send_resume()?;
        } else {
            self.send_identify()?;
        }

        while let Some(msg) = read.next().await {
            match msg {
                Ok(WsMessage::Text(text)) => match self.handle_message(&text).await {
                    Ok(LoopControl::Continue) => {}
                    Ok(LoopControl::Reconnect { resumable }) => {
                        if !resumable {
                            self.session_id = None;
                            self.sequence.lock().unwrap().take();
                        }
                        break;
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "discord.gateway.message_error");
                    }
                },
                Ok(WsMessage::Close(_)) => {
                    tracing::info!("discord.gateway.close");
                    break;
                }
                Err(e) => {
                    tracing::error!(error = %e, "discord.gateway.read_error");
                    break;
                }
                _ => {}
            }
        }

        // Tear down this connection's helper tasks so a reconnect starts clean.
        if let Some(h) = self.heartbeat_handle.take() {
            h.abort();
        }
        if let Some(h) = self.writer_handle.take() {
            h.abort();
        }
        self.ws_sender = None;
        Ok(())
    }

    fn send_identify(&self) -> Result<()> {
        let identify = serde_json::json!({
            "op": OpCode::Identify as u8,
            "d": {
                "token": self.bot_token,
                "intents": self.intents,
                "properties": {
                    "os": std::env::consts::OS,
                    "browser": "fastmd",
                    "device": "fastmd"
                }
            }
        });
        self.send(identify)
    }

    fn send_resume(&self) -> Result<()> {
        let (Some(session_id), seq) = (&self.session_id, *self.sequence.lock().unwrap()) else {
            // Nothing to resume — fall back to a fresh identify.
            return self.send_identify();
        };
        let resume = serde_json::json!({
            "op": OpCode::Resume as u8,
            "d": {
                "token": self.bot_token,
                "session_id": session_id,
                "seq": seq
            }
        });
        self.send(resume)
    }

    fn send(&self, payload: serde_json::Value) -> Result<()> {
        let Some(sender) = &self.ws_sender else {
            return Err(anyhow::anyhow!("gateway not connected"));
        };
        let text = serde_json::to_string(&payload)?;
        if sender.send(WsMessage::Text(text.into())).is_err() {
            return Err(anyhow::anyhow!("gateway writer channel closed"));
        }
        Ok(())
    }

    async fn handle_message(&mut self, text: &str) -> Result<LoopControl> {
        let payload: GatewayPayload = serde_json::from_str(text)?;

        // Update the shared sequence number for heartbeats.
        if let Some(s) = payload.s {
            *self.sequence.lock().unwrap() = Some(s);
        }

        match payload.op {
            0 => self.handle_dispatch(payload).await,
            1 => self.handle_heartbeat().await,
            7 => self.handle_reconnect().await,
            9 => self.handle_invalid_session(payload).await,
            10 => self.handle_hello(payload).await,
            11 => self.handle_heartbeat_ack().await,
            _ => Ok(LoopControl::Continue),
        }
    }

    async fn handle_dispatch(&mut self, payload: GatewayPayload) -> Result<LoopControl> {
        let event_type = payload.t.unwrap_or_default();
        let data = payload.d.unwrap_or_default();

        match event_type.as_str() {
            "READY" => {
                let ready: ReadyData = serde_json::from_value(data)?;
                self.session_id = Some(ready.session_id.clone());
                self.resume_gateway_url = Some(ready.resume_gateway_url.clone());
                let _ = self.event_sender.send(GatewayEvent::Ready(ready));
            }
            "MESSAGE_CREATE" => {
                if let Ok(msg) = serde_json::from_value::<MessageCreate>(data) {
                    let _ = self.event_sender.send(GatewayEvent::MessageCreate(msg));
                } else {
                    tracing::warn!("discord.gateway.message_create_decode_failed");
                }
            }
            "INTERACTION_CREATE" => {
                if let Ok(interaction) = serde_json::from_value::<InteractionCreate>(data) {
                    let _ = self
                        .event_sender
                        .send(GatewayEvent::InteractionCreate(interaction));
                } else {
                    tracing::warn!("discord.gateway.interaction_create_decode_failed");
                }
            }
            _ => {}
        }
        Ok(LoopControl::Continue)
    }

    async fn handle_hello(&mut self, payload: GatewayPayload) -> Result<LoopControl> {
        let hello: HelloData = serde_json::from_value(payload.d.unwrap_or_default())?;
        self.heartbeat_interval = hello.heartbeat_interval;

        // Spawn a single heartbeat task for this connection. It reads the
        // *current* sequence from the shared cell each tick (so it never
        // sends a stale value) and is aborted when the connection ends
        // (so reconnects never accumulate duplicate heartbeats).
        let interval = self.heartbeat_interval;
        let sender = self.ws_sender.clone();
        let seq = self.sequence.clone();
        let handle = tokio::spawn(async move {
            let mut timer = tokio::time::interval(Duration::from_millis(interval));
            // Discord expects the first heartbeat after one interval.
            timer.tick().await;
            loop {
                timer.tick().await;
                let Some(sender) = sender.as_ref() else {
                    break;
                };
                let d = match &*seq.lock().unwrap() {
                    Some(s) => serde_json::Value::from(*s),
                    None => serde_json::Value::Null,
                };
                let heartbeat = serde_json::json!({
                    "op": OpCode::Heartbeat as u8,
                    "d": d
                });
                let Ok(text) = serde_json::to_string(&heartbeat) else {
                    continue;
                };
                if sender.send(WsMessage::Text(text.into())).is_err() {
                    break;
                }
            }
        });
        self.heartbeat_handle = Some(handle);
        Ok(LoopControl::Continue)
    }

    async fn handle_heartbeat(&self) -> Result<LoopControl> {
        // Discord requested an immediate heartbeat; respond with the
        // current sequence number.
        let d = match &*self.sequence.lock().unwrap() {
            Some(s) => serde_json::Value::from(*s),
            None => serde_json::Value::Null,
        };
        self.send(serde_json::json!({ "op": OpCode::Heartbeat as u8, "d": d }))
            .map(|_| LoopControl::Continue)
    }

    async fn handle_heartbeat_ack(&self) -> Result<LoopControl> {
        Ok(LoopControl::Continue)
    }

    async fn handle_reconnect(&mut self) -> Result<LoopControl> {
        let _ = self.event_sender.send(GatewayEvent::Reconnect);
        Ok(LoopControl::Reconnect { resumable: true })
    }

    async fn handle_invalid_session(&mut self, payload: GatewayPayload) -> Result<LoopControl> {
        let resumable = payload
            .d
            .as_ref()
            .and_then(|d| d.as_bool())
            .unwrap_or(false);
        let _ = self
            .event_sender
            .send(GatewayEvent::InvalidSession(resumable));
        Ok(LoopControl::Reconnect { resumable })
    }
}
