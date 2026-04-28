use async_trait::async_trait;
use aws_config::BehaviorVersion;
use aws_sdk_bedrockruntime::Client;
use aws_smithy_types::Blob;
use aws_smithy_types::error::metadata::ProvideErrorMetadata;
use serde::{Deserialize, Serialize};

use crate::domain::error::embedding_provider_error::EmbeddingProviderError;
use crate::domain::port::embedding_provider::EmbeddingProvider;

const DEFAULT_MODEL_ID: &str = "amazon.titan-embed-text-v2:0";
const DEFAULT_DIMENSIONS: usize = 1024;

#[derive(Serialize)]
struct EmbeddingRequest<'a> {
    #[serde(rename = "inputText")]
    input_text: &'a str,
    dimensions: usize,
    normalize: bool,
}

#[derive(Deserialize)]
struct EmbeddingResponse {
    embedding: Vec<f32>,
}

#[derive(Clone)]
pub struct BedrockEmbeddingProvider {
    client: Client,
    model_id: String,
    dimensions: usize,
    normalize: bool,
}

impl BedrockEmbeddingProvider {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            model_id: DEFAULT_MODEL_ID.to_string(),
            dimensions: DEFAULT_DIMENSIONS,
            normalize: true,
        }
    }

    pub async fn from_default_config() -> Self {
        let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
        let client = Client::new(&config);
        Self::new(client)
    }
}

#[async_trait]
impl EmbeddingProvider for BedrockEmbeddingProvider {
    fn model(&self) -> &str {
        &self.model_id
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }

    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingProviderError> {
        let text = text.trim();
        if text.is_empty() {
            return Err(EmbeddingProviderError::RequestBuild(
                "embedding input must not be empty".into(),
            ));
        }

        let request = EmbeddingRequest {
            input_text: text,
            dimensions: self.dimensions,
            normalize: self.normalize,
        };

        let body = serde_json::to_vec(&request)
            .map_err(|err| EmbeddingProviderError::RequestBuild(err.to_string()))?;

        let output = self
            .client
            .invoke_model()
            .model_id(&self.model_id)
            .content_type("application/json")
            .accept("application/json")
            .body(Blob::new(body))
            .send()
            .await
            .map_err(|err| {
                let code = err.code().unwrap_or("unknown");
                let message = err.message().unwrap_or("no message");
                EmbeddingProviderError::ApiCall(format!(
                    "Bedrock invoke_model error: code={code}, message={message}, debug={err:?}"
                ))
            })?;

        let response: EmbeddingResponse =
            serde_json::from_slice(output.body.as_ref()).map_err(|err| {
                EmbeddingProviderError::ResponseParse(format!(
                    "invalid Titan embedding response: {err}"
                ))
            })?;

        if response.embedding.len() != self.dimensions {
            return Err(EmbeddingProviderError::ResponseParse(format!(
                "embedding dimension mismatch: expected {}, got {}",
                self.dimensions,
                response.embedding.len()
            )));
        }

        Ok(response.embedding)
    }
}
