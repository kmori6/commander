use super::{AgentClient, SessionResolver};
use futures::{SinkExt, StreamExt};
use reqwest::Client;
use serde_json::{Value, json};
use std::{env, io, sync::Arc};
use tokio::time::{Duration, sleep};
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

    // Optionally watch proactive events (schedule/watch)
    if let Ok(channel) = env::var("SLACK_PROACTIVE_CHANNEL") {
        let http = http.clone();
        let bot_token = bot_token.clone();
        let agent = agent.clone();

        tokio::spawn(async move {
            watch_proactive_events(http, bot_token, agent, channel).await;
        });
    }

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
    // 3. message + thread_ts: follow-up in an active channel thread
    let channel_type = event.get("channel_type").and_then(Value::as_str);
    let is_message = event_type == "message";
    let is_dm = is_message && channel_type == Some("im");
    let is_mention = event_type == "app_mention";
    let is_thread_message = is_message && !is_dm && event.get("thread_ts").is_some();

    if !is_dm && !is_mention && !is_thread_message {
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

    if is_thread_message && !resolver.has_session(&conversation_id).await {
        return Ok(());
    }

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

async fn watch_proactive_events(
    http: Client,
    bot_token: String,
    agent: AgentClient,
    channel: String,
) {
    // commander serve -> SSE /v1/events -> commander slack -> proactive channel
    loop {
        let mut response = match agent.connect_events(None).await {
            Ok(response) => response,
            Err(err) => {
                log::warn!("failed to connect proactive events: {err}");
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                continue;
            }
        };

        let mut buffer = String::new();

        loop {
            // SSE chunk -> buffer -> event
            let chunk = match response.chunk().await {
                Ok(Some(chunk)) => chunk,
                Ok(None) => break,
                Err(err) => {
                    log::warn!("failed to read proactive events: {err}");
                    break;
                }
            };

            buffer.push_str(&String::from_utf8_lossy(&chunk));

            while let Some(index) = buffer.find("\n\n") {
                let raw_event = buffer[..index].to_string();
                buffer = buffer[index + 2..].to_string();

                let mut event_name = "";
                let mut event_data = String::new();

                for line in raw_event.lines() {
                    let line = line.trim_end_matches('\r');

                    if let Some(value) = line.strip_prefix("event:") {
                        event_name = value.trim();
                    } else if let Some(value) = line.strip_prefix("data:") {
                        event_data.push_str(value.trim());
                    }
                }

                if event_name != "task_completed" && event_name != "task_failed" {
                    continue;
                }

                // event -> task_id -> /v1/tasks/{task_id} -> schedule/watch check
                let Ok(data) = serde_json::from_str::<Value>(&event_data) else {
                    continue;
                };

                let Some(task_id) = data
                    .get("task_id")
                    .and_then(Value::as_str)
                    .and_then(|id| Uuid::parse_str(id).ok())
                else {
                    continue;
                };

                let Ok(task) = agent.get_task(task_id).await else {
                    continue;
                };

                let is_proactive = task.get("schedule_id").and_then(Value::as_str).is_some()
                    || task.get("scheduled_at").and_then(Value::as_str).is_some();

                if !is_proactive {
                    continue;
                }

                let payload = data.get("payload").unwrap_or(&Value::Null);
                // task_completed/task_failed -> text -> Slack proactive channel
                let text = match event_name {
                    "task_completed" => payload
                        .get("output")
                        .and_then(Value::as_str)
                        .filter(|text| !text.trim().is_empty())
                        .unwrap_or("done")
                        .to_string(),
                    "task_failed" => format!(
                        "task failed: {}",
                        payload
                            .get("error")
                            .and_then(Value::as_str)
                            .filter(|text| !text.trim().is_empty())
                            .unwrap_or("unknown error")
                    ),
                    _ => continue,
                };

                if let Err(err) = post_message(&http, &bot_token, &channel, None, &text).await {
                    log::warn!("failed to post proactive message: {err}");
                }
            }
        }

        sleep(Duration::from_secs(5)).await;
    }
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
