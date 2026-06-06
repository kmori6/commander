pub mod slack;

use serde_json::{Value, json};
use std::{collections::HashMap, io};
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Clone)]
struct AgentClient {
    base_url: String,
    http: reqwest::Client,
}

impl AgentClient {
    fn new(base_url: String) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            http: reqwest::Client::new(),
        }
    }

    async fn create_session(&self) -> io::Result<Uuid> {
        let value = self
            .http
            .post(format!("{}/v1/sessions", self.base_url))
            .json(&json!({}))
            .send()
            .await
            .map_err(io::Error::other)?
            .error_for_status()
            .map_err(io::Error::other)?
            .json::<Value>()
            .await
            .map_err(io::Error::other)?;

        value
            .get("id")
            .and_then(Value::as_str)
            .and_then(|id| Uuid::parse_str(id).ok())
            .ok_or_else(|| io::Error::other("session id is missing"))
    }

    async fn ask(&self, session_id: Uuid, text: &str) -> io::Result<String> {
        let value = self
            .http
            .post(format!(
                "{}/v1/sessions/{}/messages",
                self.base_url, session_id
            ))
            .json(&json!({ "text": text }))
            .send()
            .await
            .map_err(io::Error::other)?
            .error_for_status()
            .map_err(io::Error::other)?
            .json::<Value>()
            .await
            .map_err(io::Error::other)?;

        let task_id = value
            .get("task_id")
            .and_then(Value::as_str)
            .and_then(|id| Uuid::parse_str(id).ok())
            .ok_or_else(|| io::Error::other("task id is missing"))?;

        self.wait_task(task_id).await
    }

    async fn approve(&self, approval_id: Uuid) -> io::Result<String> {
        let value = self
            .http
            .post(format!(
                "{}/v1/tools/approvals/{}/approve",
                self.base_url, approval_id
            ))
            .send()
            .await
            .map_err(io::Error::other)?
            .error_for_status()
            .map_err(io::Error::other)?
            .json::<Value>()
            .await
            .map_err(io::Error::other)?;

        let task_id = value
            .get("task_id")
            .and_then(Value::as_str)
            .and_then(|id| Uuid::parse_str(id).ok())
            .ok_or_else(|| io::Error::other("approval task id is missing"))?;

        self.wait_task(task_id).await
    }

    async fn reject(&self, approval_id: Uuid) -> io::Result<String> {
        let value = self
            .http
            .post(format!(
                "{}/v1/tools/approvals/{}/reject",
                self.base_url, approval_id
            ))
            .send()
            .await
            .map_err(io::Error::other)?
            .error_for_status()
            .map_err(io::Error::other)?
            .json::<Value>()
            .await
            .map_err(io::Error::other)?;

        let task_id = value
            .get("task_id")
            .and_then(Value::as_str)
            .and_then(|id| Uuid::parse_str(id).ok())
            .ok_or_else(|| io::Error::other("approval task id is missing"))?;

        self.wait_task(task_id).await
    }

    async fn wait_task(&self, task_id: Uuid) -> io::Result<String> {
        let mut response = self.connect_events(Some(task_id)).await?;

        let mut buffer = String::new();

        while let Some(chunk) = response.chunk().await.map_err(io::Error::other)? {
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

                if event_name.is_empty() || event_data.is_empty() {
                    continue;
                }

                let Ok(data) = serde_json::from_str::<Value>(&event_data) else {
                    continue;
                };

                let payload = data.get("payload").unwrap_or(&Value::Null);

                match event_name {
                    "task_completed" => {
                        return Ok(payload
                            .get("output")
                            .and_then(Value::as_str)
                            .filter(|text| !text.trim().is_empty())
                            .unwrap_or("done")
                            .to_string());
                    }
                    "task_failed" => {
                        return Ok(payload
                            .get("error")
                            .and_then(Value::as_str)
                            .filter(|text| !text.trim().is_empty())
                            .unwrap_or("task failed")
                            .to_string());
                    }
                    "tool_approval_requested" => {
                        let approval_id = payload
                            .get("approval_id")
                            .and_then(Value::as_str)
                            .unwrap_or("-");
                        let tool_name = payload
                            .get("tool_name")
                            .and_then(Value::as_str)
                            .unwrap_or("tool");

                        return Ok(format!(
                            "承認が必要です: {tool_name}\n`!approve {approval_id}` または `!reject {approval_id}`"
                        ));
                    }
                    _ => {}
                }
            }
        }

        Ok("task stream closed".to_string())
    }

    async fn connect_events(&self, task_id: Option<Uuid>) -> io::Result<reqwest::Response> {
        let url = match task_id {
            Some(task_id) => format!("{}/v1/events?task_id={}", self.base_url, task_id),
            None => format!("{}/v1/events", self.base_url),
        };

        self.http
            .get(url)
            .send()
            .await
            .map_err(io::Error::other)?
            .error_for_status()
            .map_err(io::Error::other)
    }

    async fn get_task(&self, task_id: Uuid) -> io::Result<Value> {
        self.http
            .get(format!("{}/v1/tasks/{}", self.base_url, task_id))
            .send()
            .await
            .map_err(io::Error::other)?
            .error_for_status()
            .map_err(io::Error::other)?
            .json::<Value>()
            .await
            .map_err(io::Error::other)
    }
}

struct SessionResolver {
    client: AgentClient,
    sessions: Mutex<HashMap<String, Uuid>>,
}

impl SessionResolver {
    fn new(client: AgentClient) -> Self {
        Self {
            client,
            sessions: Mutex::new(HashMap::new()),
        }
    }

    async fn resolve(&self, conversation_id: &str) -> io::Result<Uuid> {
        let mut sessions = self.sessions.lock().await;

        if let Some(session_id) = sessions.get(conversation_id) {
            return Ok(*session_id);
        }

        let created = self.client.create_session().await?;
        sessions.insert(conversation_id.to_string(), created);

        Ok(created)
    }

    async fn has_session(&self, conversation_id: &str) -> bool {
        self.sessions.lock().await.contains_key(conversation_id)
    }
}
