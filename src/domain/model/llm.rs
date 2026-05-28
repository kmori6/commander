use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LlmProviderKind {
    Bedrock,
    Openai,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Llm {
    pub id: String,
    pub provider: LlmProviderKind,
    pub model: String,
    pub context_window: i64,
    pub base_url: Option<String>,
    pub api_key_env: Option<String>,
}
