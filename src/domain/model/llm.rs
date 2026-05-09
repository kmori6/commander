use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Catalog {
    pub default_model: String,
    pub providers: HashMap<String, ProviderSpec>,
    pub models: Vec<ModelSpec>,
}

impl Catalog {
    pub fn default_model(&self) -> Option<&ModelSpec> {
        self.find_model(&self.default_model)
    }

    pub fn find_model(&self, id: &str) -> Option<&ModelSpec> {
        self.models.iter().find(|model| model.id == id)
    }

    pub fn find_provider(&self, id: &str) -> Option<&ProviderSpec> {
        self.providers.get(id)
    }

    pub fn set_default_model(&mut self, id: impl Into<String>) -> bool {
        let id = id.into();

        if self.find_model(&id).is_none() {
            return false;
        }

        self.default_model = id;
        true
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelSpec {
    pub id: String,
    pub provider: String,
    pub model: String,
    pub context_window: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderSpec {
    #[serde(rename = "type")]
    pub provider_type: ProviderKind,

    pub base_url: Option<String>,
    pub api_key_env: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    Bedrock,
    Openai,
}

impl ProviderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bedrock => "bedrock",
            Self::Openai => "openai",
        }
    }
}
