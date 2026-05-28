use std::{collections::HashMap, path::PathBuf};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::fs;
use tokio::sync::RwLock;

use crate::domain::error::llm_provider_error::LlmProviderError;
use crate::domain::model::llm::{Llm, LlmProviderKind};
use crate::domain::port::llm_provider::{LlmProvider, LlmRequest, LlmResponse};
use crate::infrastructure::llm::bedrock_llm_provider::BedrockLlmProvider;
use crate::infrastructure::llm::openai_llm_provider::OpenaiLlmProvider;

const EXAMPLE_MODEL_CONFIG_PATH: &str = "config/models.json";

#[derive(Debug, Clone)]
struct ModelConfigPaths {
    example_path: PathBuf,
    user_path: PathBuf,
}

impl ModelConfigPaths {
    fn new(user_path: PathBuf) -> Self {
        Self {
            example_path: PathBuf::from(EXAMPLE_MODEL_CONFIG_PATH),
            user_path,
        }
    }

    fn path(&self) -> PathBuf {
        self.user_path.clone()
    }

    async fn ensure_user_config(&self) -> Result<(), LlmProviderError> {
        if fs::try_exists(&self.user_path).await.map_err(|err| {
            LlmProviderError::Unexpected(format!(
                "failed to check model config {}: {err}",
                self.user_path.display()
            ))
        })? {
            return Ok(());
        }

        if let Some(parent) = self.user_path.parent() {
            fs::create_dir_all(parent).await.map_err(|err| {
                LlmProviderError::Unexpected(format!(
                    "failed to create model config directory {}: {err}",
                    parent.display()
                ))
            })?;
        }

        fs::copy(&self.example_path, &self.user_path)
            .await
            .map_err(|err| {
                LlmProviderError::Unexpected(format!(
                    "failed to initialize model config from {} to {}: {err}",
                    self.example_path.display(),
                    self.user_path.display()
                ))
            })?;

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LlmConfig {
    default_model: String,
    providers: HashMap<String, ProviderConfig>,
    models: Vec<ModelConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModelConfig {
    id: String,
    provider: String,
    model: String,
    context_window: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProviderConfig {
    #[serde(rename = "type")]
    kind: LlmProviderKind,
    base_url: Option<String>,
    api_key_env: Option<String>,
}

impl LlmConfig {
    fn resolve(&self, id: &str) -> Result<Llm, String> {
        let id = id.trim();

        if id.is_empty() {
            return Err("llm id must not be empty".to_string());
        }

        let model = self
            .models
            .iter()
            .find(|model| model.id == id)
            .ok_or_else(|| format!("unknown model id: {id}"))?;

        let provider = self.providers.get(&model.provider).ok_or_else(|| {
            format!(
                "model {} references unknown provider {}",
                model.id, model.provider
            )
        })?;

        let model_id = model.id.trim().to_string();
        if model_id.is_empty() {
            return Err("llm id must not be empty".to_string());
        }

        let model_name = model.model.trim().to_string();
        if model_name.is_empty() {
            return Err("llm model must not be empty".to_string());
        }

        if model.context_window <= 0 {
            return Err("llm context_window must be positive".to_string());
        }

        Ok(Llm {
            id: model_id,
            provider: provider.kind,
            model: model_name,
            context_window: model.context_window,
            base_url: provider.base_url.clone(),
            api_key_env: provider.api_key_env.clone(),
        })
    }

    fn list_llms(&self) -> Result<Vec<Llm>, String> {
        self.models
            .iter()
            .map(|model| self.resolve(&model.id))
            .collect()
    }

    fn select_default_model(&mut self, id: &str) -> Result<Llm, String> {
        let llm = self.resolve(id)?;
        self.default_model = llm.id.clone();
        Ok(llm)
    }

    fn validate(&self) -> Result<(), String> {
        self.resolve(&self.default_model)?;

        for model in &self.models {
            self.resolve(&model.id)?;
        }

        Ok(())
    }
}

pub struct LlmGateway {
    config: RwLock<LlmConfig>,
    paths: ModelConfigPaths,
    bedrock: BedrockLlmProvider,
    openai: OpenaiLlmProvider,
}

impl LlmGateway {
    pub async fn from_config_path(user_path: impl Into<PathBuf>) -> Result<Self, LlmProviderError> {
        let paths = ModelConfigPaths::new(user_path.into());
        paths.ensure_user_config().await?;

        let config = load_config(&paths).await?;
        config.validate().map_err(LlmProviderError::Unexpected)?;

        Ok(Self {
            config: RwLock::new(config),
            paths,
            bedrock: BedrockLlmProvider::from_default_config().await,
            openai: OpenaiLlmProvider::from_default_client(),
        })
    }

    pub async fn list_models(&self) -> Vec<Llm> {
        self.config
            .read()
            .await
            .list_llms()
            .expect("llm config is validated at load time")
    }

    pub async fn select_model(&self, id: &str) -> Result<Llm, LlmProviderError> {
        let (next_config, selected_model) = {
            let config = self.config.read().await;
            let mut next_config = config.clone();

            let selected_model = next_config
                .select_default_model(id)
                .map_err(LlmProviderError::RequestBuild)?;

            (next_config, selected_model)
        };

        save_config(&self.paths, &next_config).await?;
        *self.config.write().await = next_config;

        Ok(selected_model)
    }
}

#[async_trait]
impl LlmProvider for LlmGateway {
    async fn respond(&self, mut request: LlmRequest) -> Result<LlmResponse, LlmProviderError> {
        let llm = {
            let config = self.config.read().await;

            config
                .resolve(&request.model)
                .map_err(LlmProviderError::RequestBuild)?
        };

        request.model = llm.model.clone();

        match llm.provider {
            LlmProviderKind::Bedrock => self.bedrock.respond(request).await,
            LlmProviderKind::Openai => self.openai.respond_with_llm(&llm, request).await,
        }
    }

    async fn context_window(&self, model: &str) -> i64 {
        self.config
            .read()
            .await
            .resolve(model)
            .map(|llm| llm.context_window)
            .unwrap_or(256_000)
    }

    async fn current_model_id(&self) -> Result<String, LlmProviderError> {
        Ok(self.config.read().await.default_model.clone())
    }
}

async fn load_config(paths: &ModelConfigPaths) -> Result<LlmConfig, LlmProviderError> {
    let path = paths.path();
    let content = fs::read_to_string(&path).await.map_err(|err| {
        LlmProviderError::Unexpected(format!(
            "failed to read model config {}: {err}",
            path.display()
        ))
    })?;

    serde_json::from_str(&content).map_err(|err| {
        LlmProviderError::Unexpected(format!(
            "failed to parse model config {}: {err}",
            path.display()
        ))
    })
}

async fn save_config(paths: &ModelConfigPaths, config: &LlmConfig) -> Result<(), LlmProviderError> {
    let path = paths.path();

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await.map_err(|err| {
            LlmProviderError::Unexpected(format!(
                "failed to create model config directory {}: {err}",
                parent.display()
            ))
        })?;
    }

    let content = serde_json::to_string_pretty(config).map_err(|err| {
        LlmProviderError::Unexpected(format!("failed to serialize model config: {err}"))
    })?;

    fs::write(&path, format!("{content}\n"))
        .await
        .map_err(|err| {
            LlmProviderError::Unexpected(format!(
                "failed to write model config {}: {err}",
                path.display()
            ))
        })
}
