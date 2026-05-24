use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LlmProviderKind {
    Bedrock,
    Openai,
}

impl LlmProviderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bedrock => "bedrock",
            Self::Openai => "openai",
        }
    }
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

impl Llm {
    pub fn restore(
        id: impl Into<String>,
        provider: LlmProviderKind,
        model: impl Into<String>,
        context_window: i64,
        base_url: Option<String>,
        api_key_env: Option<String>,
    ) -> Result<Self, String> {
        let id = id.into().trim().to_string();
        let model = model.into().trim().to_string();

        if id.is_empty() {
            return Err("llm id must not be empty".to_string());
        }

        if model.is_empty() {
            return Err("llm model must not be empty".to_string());
        }

        if context_window <= 0 {
            return Err("llm context_window must be positive".to_string());
        }

        Ok(Self {
            id,
            provider,
            model,
            context_window,
            base_url: normalize_optional(base_url),
            api_key_env: normalize_optional(api_key_env),
        })
    }
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}
