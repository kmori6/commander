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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_title_collapses_whitespace() {
        assert_eq!(
            Some("hello world".to_string()),
            Session::normalize_title("  hello   world  ")
        );
    }

    #[test]
    fn normalize_title_returns_none_for_blank() {
        assert_eq!(None, Session::normalize_title("   \n\t  "));
    }

    #[test]
    fn normalize_title_truncates_to_max_chars() {
        let title = "a".repeat(100);

        assert_eq!(Some("a".repeat(80)), Session::normalize_title(title));
    }
}
