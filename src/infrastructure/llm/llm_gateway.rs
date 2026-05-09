use std::path::PathBuf;

use async_trait::async_trait;
use tokio::fs;
use tokio::sync::RwLock;

use crate::domain::error::llm_provider_error::LlmProviderError;
use crate::domain::model::llm::{Catalog, ModelSpec, ProviderKind};
use crate::domain::port::llm_provider::{LlmProvider, LlmRequest, LlmResponse};
use crate::infrastructure::llm::bedrock_llm_provider::BedrockLlmProvider;
use crate::infrastructure::llm::openai_llm_provider::OpenaiLlmProvider;

pub struct LlmGateway {
    catalog: RwLock<Catalog>,
    paths: ModelConfigPaths,
    bedrock: BedrockLlmProvider,
    openai: OpenaiLlmProvider,
}

impl LlmGateway {
    pub async fn from_default_config() -> Result<Self, LlmProviderError> {
        let paths = ModelConfigPaths::resolve();
        let catalog = load_catalog(&paths).await?;
        validate_catalog(&catalog)?;

        Ok(Self {
            catalog: RwLock::new(catalog),
            paths,
            bedrock: BedrockLlmProvider::from_default_config().await,
            openai: OpenaiLlmProvider::from_default_client(),
        })
    }

    pub async fn list_models(&self) -> Vec<ModelSpec> {
        self.catalog.read().await.models.clone()
    }

    pub async fn default_model_id(&self) -> String {
        self.catalog.read().await.default_model.clone()
    }

    pub async fn select_model(&self, id: &str) -> Result<ModelSpec, LlmProviderError> {
        let (next_catalog, selected_model) = {
            let catalog = self.catalog.read().await;
            let mut next_catalog = catalog.clone();

            if !next_catalog.set_default_model(id.to_string()) {
                return Err(LlmProviderError::RequestBuild(format!(
                    "unknown model id: {id}"
                )));
            }

            let selected_model = next_catalog.find_model(id).cloned().ok_or_else(|| {
                LlmProviderError::Unexpected(format!("selected model disappeared: {id}"))
            })?;

            (next_catalog, selected_model)
        };

        save_catalog(&self.paths, &next_catalog).await?;
        *self.catalog.write().await = next_catalog;

        Ok(selected_model)
    }

    pub async fn context_window(&self, id: &str) -> Option<i64> {
        self.catalog
            .read()
            .await
            .find_model(id)
            .map(|model| model.context_window)
    }
}

#[async_trait]
impl LlmProvider for LlmGateway {
    async fn respond(&self, mut request: LlmRequest) -> Result<LlmResponse, LlmProviderError> {
        let (model, provider) = {
            let catalog = self.catalog.read().await;

            let model = catalog.find_model(&request.model).cloned().ok_or_else(|| {
                LlmProviderError::RequestBuild(format!("unknown model id: {}", request.model))
            })?;

            let provider = catalog
                .find_provider(&model.provider)
                .cloned()
                .ok_or_else(|| {
                    LlmProviderError::RequestBuild(format!(
                        "unknown provider id: {}",
                        model.provider
                    ))
                })?;

            (model, provider)
        };

        request.model = model.model.clone();

        match provider.provider_type {
            ProviderKind::Bedrock => self.bedrock.respond(request).await,
            ProviderKind::Openai => {
                self.openai
                    .respond_with_provider(&provider, &model, request)
                    .await
            }
        }
    }
}

#[derive(Debug, Clone)]
struct ModelConfigPaths {
    path: PathBuf,
}

impl ModelConfigPaths {
    fn resolve() -> Self {
        Self {
            path: PathBuf::from("config/models.json"),
        }
    }

    fn path(&self) -> PathBuf {
        self.path.clone()
    }
}

async fn load_catalog(paths: &ModelConfigPaths) -> Result<Catalog, LlmProviderError> {
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

async fn save_catalog(paths: &ModelConfigPaths, catalog: &Catalog) -> Result<(), LlmProviderError> {
    let path = paths.path();

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await.map_err(|err| {
            LlmProviderError::Unexpected(format!(
                "failed to create model config directory {}: {err}",
                parent.display()
            ))
        })?;
    }

    let content = serde_json::to_string_pretty(catalog).map_err(|err| {
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

fn validate_catalog(catalog: &Catalog) -> Result<(), LlmProviderError> {
    if catalog.find_model(&catalog.default_model).is_none() {
        return Err(LlmProviderError::Unexpected(format!(
            "default_model is not defined in models: {}",
            catalog.default_model
        )));
    }

    for model in &catalog.models {
        if catalog.find_provider(&model.provider).is_none() {
            return Err(LlmProviderError::Unexpected(format!(
                "model {} references unknown provider {}",
                model.id, model.provider
            )));
        }
    }

    Ok(())
}
