use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Llm {
    pub model: String,
    pub context_window: i64,
    pub base_url: String,
}
