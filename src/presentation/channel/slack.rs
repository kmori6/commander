use super::{AgentClient, SessionResolver};
use futures::{SinkExt, StreamExt};
use reqwest::Client;
use serde_json::{Value, json};
use std::{env, io, sync::Arc};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use uuid::Uuid;

pub async fn run(base_url: String) -> io::Result<()> {
    let app_token = env::var("SLACK_APP_TOKEN")
        .map_err(|_| io::Error::new(io::ErrorKind::NotFound, "SLACK_APP_TOKEN is missing"))?;
    let bot_token = env::var("SLACK_BOT_TOKEN")
        .map_err(|_| io::Error::new(io::ErrorKind::NotFound, "SLACK_BOT_TOKEN is missing"))?;

    let http = Client::new();
    let socket_url = open_socket(&http, &app_token).await?;

    let (socket, _) = connect_async(socket_url).await.map_err(io::Error::other)?;
    let (mut writer, mut reader) = socket.split();

    let agent = AgentClient::new(base_url);
    let resolver = Arc::new(SessionResolver::new(agent.clone()));

    // Slack Socket Mode -> WebSocket -> reader.next()
    while let Some(message) = reader.next().await {
        let message = message.map_err(io::Error::other)?;
        match message {
            Message::Text(text) => {
                // event example:
                // {
                //     "envelope_id": "dummy-envelope-id",
                //     "type": "events_api",
                //     "payload": {
                //         "event": {
                //         "type": "app_mention",
                //         "text": "<@U_BOT> hello"
                //         }
                //     }
                // }
                let Ok(envelope) = serde_json::from_str::<Value>(&text) else {
                    continue;
                };

                // ACK: reader -> envelope_id -> writer -> WebSocket -> Slack
                if let Some(envelope_id) = envelope.get("envelope_id").and_then(Value::as_str) {
                    writer
                        .send(Message::Text(
                            json!({ "envelope_id": envelope_id }).to_string().into(),
                        ))
                        .await
                        .map_err(io::Error::other)?;
                }

                let agent = agent.clone();
                let resolver = resolver.clone();
                let http = http.clone();
                let bot_token = bot_token.clone();

                tokio::spawn(async move {
                    if let Err(err) =
                        handle_event(&http, &bot_token, &agent, resolver, envelope).await
                    {
                        log::warn!("failed to handle slack event: {err}");
                    }
                });
            }
            // Ping/Pong: Slack -> WebSocket -> reader(Ping) -> writer(Pong) -> WebSocket -> Slack
            Message::Ping(payload) => {
                writer
                    .send(Message::Pong(payload))
                    .await
                    .map_err(io::Error::other)?;
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    Ok(())
}

// call slack endpoint to get websocket URL
// https://docs.slack.dev/apis/events-api/using-socket-mode/#call
async fn open_socket(http: &Client, app_token: &str) -> io::Result<String> {
    // {
    //     "ok": true,
    //     "url": "wss:\/\/wss.slack.com\/link\/?ticket=dummy-ticket"
    // }
    let value = http
        .post("https://slack.com/api/apps.connections.open")
        .bearer_auth(app_token)
        .send()
        .await
        .map_err(io::Error::other)?
        .error_for_status()
        .map_err(io::Error::other)?
        .json::<Value>()
        .await
        .map_err(io::Error::other)?;

    if !value.get("ok").and_then(Value::as_bool).unwrap_or(false) {
        return Err(io::Error::other(format!(
            "failed to open slack socket: {}",
            value
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("unknown_error")
        )));
    }

    value
        .get("url")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| io::Error::other("slack socket url is missing"))
}

async fn handle_event(
    http: &Client,
    bot_token: &str,
    agent: &AgentClient,
    resolver: Arc<SessionResolver>,
    envelope: Value,
) -> io::Result<()> {
    // filter only `events_api` type
    if envelope.get("type").and_then(Value::as_str) != Some("events_api") {
        return Ok(());
    }

    // event example:
    // {
    //     "type": "events_api",
    //     "payload": {
    //         "event": {
    //         "type": "app_mention",
    //         "text": "<@U_BOT> hello"
    //         }
    //     }
    // }
    let payload = envelope.get("payload").unwrap_or(&Value::Null);
    let event = payload.get("event").unwrap_or(&Value::Null);
    let event_type = event.get("type").and_then(Value::as_str).unwrap_or("");

    // NOTE: we only handle types:
    // 1. app_mention: @commander in a channel
    // 2. message + channel_type: im: direct message to commander
    let is_dm =
        event_type == "message" && event.get("channel_type").and_then(Value::as_str) == Some("im");
    let is_mention = event_type == "app_mention";

    if !is_dm && !is_mention {
        return Ok(());
    }

    if event.get("bot_id").is_some() || event.get("subtype").is_some() {
        return Ok(());
    }

    let Some(channel) = event.get("channel").and_then(Value::as_str) else {
        return Ok(());
    };
    let Some(raw_text) = event.get("text").and_then(Value::as_str) else {
        return Ok(());
    };

    // text example: "<@U_BOT> hello" -> "hello"
    let text = strip_mentions(raw_text);

    if text.is_empty() {
        return Ok(());
    }

    // For channel mentions, start or continue a thread.
    // For DMs, preserve an existing thread and otherwise post a normal DM reply.
    let thread_ts = if is_mention {
        event
            .get("thread_ts")
            .or_else(|| event.get("ts"))
            .and_then(Value::as_str)
            .map(ToString::to_string)
    } else {
        event
            .get("thread_ts")
            .and_then(Value::as_str)
            .map(ToString::to_string)
    };

    // channel mention: workspace + channel + thread timestamp = conversation id
    // example: "slack:T_WORKSPACE:channel:C_CHANNEL:thread:1234567890.000000"
    // DM: workspace + DM channel = conversation id
    // example: "slack:T_WORKSPACE:dm:D_CHANNEL"
    let team_id = payload
        .get("team_id")
        .or_else(|| envelope.get("team_id"))
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let conversation_id = if is_dm {
        format!("slack:{team_id}:dm:{channel}")
    } else {
        let conversation_ts = thread_ts.as_deref().unwrap_or("default");
        format!("slack:{team_id}:channel:{channel}:thread:{conversation_ts}")
    };

    let reply = if let Some(id) = text.strip_prefix("!approve ") {
        match Uuid::parse_str(id.trim()) {
            Ok(approval_id) => agent.approve(approval_id).await?,
            Err(_) => "usage: !approve <approval_id>".to_string(),
        }
    } else if let Some(id) = text.strip_prefix("!reject ") {
        match Uuid::parse_str(id.trim()) {
            Ok(approval_id) => agent.reject(approval_id).await?,
            Err(_) => "usage: !reject <approval_id>".to_string(),
        }
    } else {
        // 1. POST /v1/sessions/{session_id}/messages
        // 2. GET /v1/events?task_id=... and while loop until get the task_completed event
        let session_id = resolver.resolve(&conversation_id).await?;
        agent.ask(session_id, &text).await?
    };

    post_message(http, bot_token, channel, thread_ts.as_deref(), &reply).await
}

// text -> slack
async fn post_message(
    http: &Client,
    bot_token: &str,
    channel: &str,
    thread_ts: Option<&str>,
    text: &str,
) -> io::Result<()> {
    let mut body = json!({
        "channel": channel,
        "text": text,
    });

    if let Some(thread_ts) = thread_ts {
        body["thread_ts"] = json!(thread_ts);
    }

    let value = http
        .post("https://slack.com/api/chat.postMessage")
        .bearer_auth(bot_token)
        .json(&body)
        .send()
        .await
        .map_err(io::Error::other)?
        .error_for_status()
        .map_err(io::Error::other)?
        .json::<Value>()
        .await
        .map_err(io::Error::other)?;

    if value.get("ok").and_then(Value::as_bool).unwrap_or(false) {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "failed to post slack message: {}",
            value
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("unknown_error")
        )))
    }
}

fn strip_mentions(text: &str) -> String {
    let mut rest = text.trim();

    loop {
        let Some(after_prefix) = rest.strip_prefix("<@") else {
            return rest.trim().to_string();
        };

        let Some(index) = after_prefix.find('>') else {
            return rest.trim().to_string();
        };

        rest = after_prefix[index + 1..].trim_start();
    }
}
