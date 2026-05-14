use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const SESSION_TITLE_MAX_CHARS: usize = 80;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub id: Uuid,
    pub title: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Session {
    pub fn normalize_title(title: impl AsRef<str>) -> Option<String> {
        let normalized = title
            .as_ref()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");

        if normalized.is_empty() {
            return None;
        }

        Some(
            normalized
                .chars()
                .take(SESSION_TITLE_MAX_CHARS)
                .collect::<String>(),
        )
    }
}
