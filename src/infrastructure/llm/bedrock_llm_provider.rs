use crate::domain::error::llm_provider_error::LlmProviderError;
use crate::domain::model::attachment::Attachment;
use crate::domain::model::message::{Message, MessageContent};
use crate::domain::model::role::Role;
use crate::domain::model::token_usage::TokenUsage;
use crate::domain::model::tool::{ToolCall, ToolSpec};
use crate::domain::port::llm_provider::{LlmProvider, LlmResponse, StructuredOutputSchema};
use async_trait::async_trait;
use aws_config::BehaviorVersion;
use aws_sdk_bedrockruntime::types::{
    JsonSchemaDefinition, OutputConfig, OutputFormat, OutputFormatStructure, OutputFormatType,
};
use aws_sdk_bedrockruntime::{
    Client,
    types::{
        ContentBlock, ConversationRole, DocumentBlock, DocumentFormat, DocumentSource, ImageBlock,
        ImageFormat, ImageSource, Message as BedrockMessage, SystemContentBlock,
        TokenUsage as BedrockTokenUsage, Tool, ToolConfiguration, ToolInputSchema, ToolResultBlock,
        ToolResultContentBlock, ToolResultStatus, ToolSpecification, ToolUseBlock,
    },
};
use aws_smithy_types::Blob;
use aws_smithy_types::error::metadata::ProvideErrorMetadata;
use aws_smithy_types::{Document, Number};
use std::collections::HashMap;

struct ConverseOptions {
    tools: Option<Vec<ToolSpec>>,
    structured_output: Option<StructuredOutputSchema>,
}

struct ConverseResult {
    text_blocks: Vec<String>,
    tool_calls: Vec<ToolCall>,
    usage: TokenUsage,
}

impl ConverseResult {
    fn plain_text(&self) -> String {
        self.text_blocks.join("\n")
    }

    fn structured_text(&self) -> String {
        self.text_blocks.join("")
    }
}

#[derive(Clone)]
pub struct BedrockLlmProvider {
    client: Client,
}

impl BedrockLlmProvider {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    pub async fn from_default_config() -> Self {
        let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
        let client = Client::new(&config);
        Self::new(client)
    }

    async fn converse(
        &self,
        messages: Vec<Message>,
        model: &str,
        options: ConverseOptions,
    ) -> Result<ConverseResult, LlmProviderError> {
        if options.tools.is_some() && options.structured_output.is_some() {
            return Err(LlmProviderError::RequestBuild(
                "Combining tools and structured output is not supported yet".to_string(),
            ));
        }

        let system_blocks = build_system_content_blocks(&messages)?;

        let message_blocks = build_content_block(&messages)?;

        let mut req = self
            .client
            .converse()
            .model_id(model)
            .set_messages(Some(message_blocks));

        for block in system_blocks {
            req = req.system(block);
        }

        if let Some(tools) = options.tools.as_ref().filter(|tools| !tools.is_empty()) {
            req = req.tool_config(tool_configuration(tools)?);
        }

        if let Some(schema) = options.structured_output.as_ref() {
            req = req.output_config(structured_output_config(schema)?);
        }

        let output = req.send().await.map_err(|e| {
            let code = e.code().unwrap_or("unknown");
            let message = e.message().unwrap_or("no message");
            LlmProviderError::ApiCall(format!(
                "Bedrock converse error: code={code}, message={message}, debug={e:?}"
            ))
        })?;

        let usage = convert_token_usage(output.usage());

        let output_blocks = output
            .output()
            .ok_or_else(|| {
                LlmProviderError::ResponseParse("No output in Bedrock response".to_string())
            })?
            .as_message()
            .map_err(|_| {
                LlmProviderError::ResponseParse(
                    "Unsupported output type in Bedrock response".to_string(),
                )
            })?
            .content();

        // LLM response
        let mut text_blocks = Vec::new();
        let mut tool_calls = Vec::new();

        for block in output_blocks {
            if let Ok(text) = block.as_text() {
                text_blocks.push(text.to_string());
                continue;
            }

            if let Ok(tool_use) = block.as_tool_use() {
                tool_calls.push(ToolCall {
                    id: tool_use.tool_use_id().to_string(),
                    name: tool_use.name().to_string(),
                    arguments: document_to_json(tool_use.input())?,
                });
            }
        }

        Ok(ConverseResult {
            text_blocks,
            tool_calls,
            usage,
        })
    }
}

#[async_trait]
impl LlmProvider for BedrockLlmProvider {
    async fn response(
        &self,
        messages: Vec<Message>,
        model: &str,
    ) -> Result<String, LlmProviderError> {
        Ok(self
            .converse(
                messages,
                model,
                ConverseOptions {
                    tools: None,
                    structured_output: None,
                },
            )
            .await?
            .plain_text())
    }

    async fn response_with_tool(
        &self,
        messages: Vec<Message>,
        tools: Vec<ToolSpec>,
        model: &str,
    ) -> Result<LlmResponse, LlmProviderError> {
        let result = self
            .converse(
                messages,
                model,
                ConverseOptions {
                    tools: Some(tools),
                    structured_output: None,
                },
            )
            .await?;
        Ok(LlmResponse {
            text: result.plain_text(),
            tool_calls: result.tool_calls,
            usage: result.usage,
        })
    }

    async fn response_with_structure(
        &self,
        messages: Vec<Message>,
        schema: StructuredOutputSchema,
        model: &str,
    ) -> Result<serde_json::Value, LlmProviderError> {
        let result = self
            .converse(
                messages,
                model,
                ConverseOptions {
                    tools: None,
                    structured_output: Some(schema),
                },
            )
            .await?;
        serde_json::from_str(result.structured_text().trim()).map_err(|e| {
            LlmProviderError::ResponseParse(format!("Failed to parse structured output JSON: {e}"))
        })
    }
}

/// Converts system messages to Bedrock SystemContentBlocks.
fn build_system_content_blocks(
    messages: &[Message],
) -> Result<Vec<SystemContentBlock>, LlmProviderError> {
    messages
        .iter()
        .filter(|m| m.role == Role::System)
        .map(|m| match &m.content {
            MessageContent::Text(text) => Ok(SystemContentBlock::Text(text.clone())),
            MessageContent::Multimodal { .. } => Err(LlmProviderError::RequestBuild(
                "System messages cannot contain attachments".to_string(),
            )),
            MessageContent::ToolCall { .. } => Err(LlmProviderError::RequestBuild(
                "System messages cannot contain tool calls".to_string(),
            )),
            MessageContent::ToolResults(_) => Err(LlmProviderError::RequestBuild(
                "System messages cannot contain tool results".to_string(),
            )),
        })
        .collect()
}

/// Converts domain messages to Bedrock messages.
fn build_content_block(messages: &[Message]) -> Result<Vec<BedrockMessage>, LlmProviderError> {
    let mut message_blocks: Vec<BedrockMessage> = vec![];
    for m in messages.iter().filter(|m| m.role != Role::System) {
        let role = match m.role {
            Role::Assistant => ConversationRole::Assistant,
            _ => ConversationRole::User,
        };
        let msg = match &m.content {
            MessageContent::Text(text) => BedrockMessage::builder()
                .role(role.clone())
                .content(ContentBlock::Text(text.clone()))
                .build()
                .map_err(|e| {
                    LlmProviderError::RequestBuild(format!("Error building Bedrock message: {}", e))
                })?,
            MessageContent::Multimodal { text, attachments } => {
                let mut builder = BedrockMessage::builder().role(role.clone());

                if !text.is_empty() {
                    builder = builder.content(ContentBlock::Text(text.clone()));
                }

                for attachment in attachments {
                    let block = attachment_to_content_block(attachment)?;
                    builder = builder.content(block);
                }

                builder.build().map_err(|e| {
                    LlmProviderError::RequestBuild(format!("Error building Bedrock message: {}", e))
                })?
            }
            MessageContent::ToolCall { text, tool_calls } => {
                let mut builder = BedrockMessage::builder().role(ConversationRole::Assistant);

                if let Some(text) = text.as_ref().filter(|text| !text.is_empty()) {
                    builder = builder.content(ContentBlock::Text(text.clone()));
                }

                for tool_call in tool_calls {
                    let tool_use = ToolUseBlock::builder()
                        .tool_use_id(tool_call.id.clone())
                        .name(tool_call.name.clone())
                        .input(json_to_document(&tool_call.arguments)?)
                        .build()
                        .map_err(|e| {
                            LlmProviderError::RequestBuild(format!(
                                "Error building Bedrock tool use block: {}",
                                e
                            ))
                        })?;

                    builder = builder.content(ContentBlock::ToolUse(tool_use));
                }

                builder.build().map_err(|e| {
                    LlmProviderError::RequestBuild(format!("Error building Bedrock message: {}", e))
                })?
            }
            MessageContent::ToolResults(tool_results) => {
                let mut builder = BedrockMessage::builder().role(ConversationRole::User);

                for tool_result in tool_results {
                    let status = if tool_result.is_error {
                        ToolResultStatus::Error
                    } else {
                        ToolResultStatus::Success
                    };

                    let result_content =
                        ToolResultContentBlock::Json(json_to_document(&tool_result.output)?);

                    let block = ToolResultBlock::builder()
                        .tool_use_id(tool_result.tool_call_id.clone())
                        .content(result_content)
                        .status(status)
                        .build()
                        .map_err(|e| {
                            LlmProviderError::RequestBuild(format!(
                                "Error building Bedrock tool result block: {}",
                                e
                            ))
                        })?;

                    builder = builder.content(ContentBlock::ToolResult(block));
                }

                builder.build().map_err(|e| {
                    LlmProviderError::RequestBuild(format!("Error building Bedrock message: {}", e))
                })?
            }
        };
        message_blocks.push(msg);
    }

    Ok(message_blocks)
}

/// Converts a Vec of ToolCall to a Bedrock ToolConfiguration.
fn tool_configuration(tools: &[ToolSpec]) -> Result<ToolConfiguration, LlmProviderError> {
    let mut builder = ToolConfiguration::builder();

    for tool in tools {
        let spec = ToolSpecification::builder()
            .name(tool.name.clone())
            .description(tool.description.clone())
            .input_schema(ToolInputSchema::Json(json_to_document(&tool.parameters)?))
            .build()
            .map_err(|e| {
                LlmProviderError::RequestBuild(format!(
                    "Error building Bedrock tool specification: {}",
                    e
                ))
            })?;

        builder = builder.tools(Tool::ToolSpec(spec));
    }

    builder.build().map_err(|e| {
        LlmProviderError::RequestBuild(format!("Error building Bedrock tool configuration: {}", e))
    })
}

/// Converts a Bedrock Document to a serde_json::Value.
fn document_to_json(document: &Document) -> Result<serde_json::Value, LlmProviderError> {
    match document {
        Document::Object(object) => {
            let mut map = serde_json::Map::new();
            for (key, value) in object {
                map.insert(key.clone(), document_to_json(value)?);
            }
            Ok(serde_json::Value::Object(map))
        }
        Document::Array(array) => Ok(serde_json::Value::Array(
            array
                .iter()
                .map(document_to_json)
                .collect::<Result<Vec<_>, _>>()?,
        )),
        Document::Number(number) => match number {
            Number::PosInt(value) => Ok(serde_json::Value::Number((*value).into())),
            Number::NegInt(value) => Ok(serde_json::Value::Number((*value).into())),
            Number::Float(value) => serde_json::Number::from_f64(*value)
                .map(serde_json::Value::Number)
                .ok_or_else(|| {
                    LlmProviderError::ResponseParse(format!(
                        "Bedrock tool input contains non-finite float: {}",
                        value
                    ))
                }),
        },
        Document::String(value) => Ok(serde_json::Value::String(value.clone())),
        Document::Bool(value) => Ok(serde_json::Value::Bool(*value)),
        Document::Null => Ok(serde_json::Value::Null),
    }
}

/// Converts a serde_json::Value to a Bedrock Document.
fn json_to_document(value: &serde_json::Value) -> Result<Document, LlmProviderError> {
    match value {
        serde_json::Value::Object(object) => {
            let mut map = HashMap::new();
            for (key, value) in object {
                map.insert(key.clone(), json_to_document(value)?);
            }
            Ok(Document::Object(map))
        }
        serde_json::Value::Array(array) => Ok(Document::Array(
            array
                .iter()
                .map(json_to_document)
                .collect::<Result<Vec<_>, _>>()?,
        )),
        serde_json::Value::Number(number) => {
            if let Some(value) = number.as_u64() {
                Ok(Document::Number(Number::PosInt(value)))
            } else if let Some(value) = number.as_i64() {
                if value < 0 {
                    Ok(Document::Number(Number::NegInt(value)))
                } else {
                    Ok(Document::Number(Number::PosInt(value as u64)))
                }
            } else if let Some(value) = number.as_f64() {
                Ok(Document::Number(Number::Float(value)))
            } else {
                Err(LlmProviderError::RequestBuild(format!(
                    "Unsupported JSON number for Bedrock tool schema: {}",
                    number
                )))
            }
        }
        serde_json::Value::String(value) => Ok(Document::String(value.clone())),
        serde_json::Value::Bool(value) => Ok(Document::Bool(*value)),
        serde_json::Value::Null => Ok(Document::Null),
    }
}

/// Converts a StructuredOutputSchema to a Bedrock OutputConfig.
fn structured_output_config(
    schema: &StructuredOutputSchema,
) -> Result<OutputConfig, LlmProviderError> {
    let schema_string = serde_json::to_string(&schema.schema)
        .map_err(|e| LlmProviderError::RequestBuild(format!("Invalid JSON schema: {e}")))?;

    let json_schema = JsonSchemaDefinition::builder()
        .name(schema.name.clone())
        .set_description(schema.description.clone())
        .schema(schema_string)
        .build()
        .map_err(|e| LlmProviderError::RequestBuild(format!("Failed to build JSON schema: {e}")))?;

    let text_format = OutputFormat::builder()
        .r#type(OutputFormatType::JsonSchema)
        .structure(OutputFormatStructure::JsonSchema(json_schema))
        .build()
        .map_err(|e| {
            LlmProviderError::RequestBuild(format!("Failed to build output format: {e}"))
        })?;

    Ok(OutputConfig::builder().text_format(text_format).build())
}

/// Converts Attachment to a Bedrock ContentBlock.
fn attachment_to_content_block(attachment: &Attachment) -> Result<ContentBlock, LlmProviderError> {
    if attachment.is_image() {
        let format = match attachment.mime_type.as_str() {
            "image/png" => ImageFormat::Png,
            "image/jpeg" | "image/jpg" => ImageFormat::Jpeg,
            "image/gif" => ImageFormat::Gif,
            "image/webp" => ImageFormat::Webp,
            other => {
                return Err(LlmProviderError::RequestBuild(format!(
                    "Unsupported image format: {other}"
                )));
            }
        };
        let image_block = ImageBlock::builder()
            .format(format)
            .source(ImageSource::Bytes(Blob::new(attachment.data.clone())))
            .build()
            .map_err(|e| {
                LlmProviderError::RequestBuild(format!("Error building ImageBlock: {e}"))
            })?;
        Ok(ContentBlock::Image(image_block))
    } else {
        let format = match attachment.mime_type.as_str() {
            "application/pdf" => DocumentFormat::Pdf,
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
            | "application/msword" => DocumentFormat::Docx,
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
            | "application/vnd.ms-excel" => DocumentFormat::Xlsx,
            "text/html" => DocumentFormat::Html,
            "text/markdown" | "text/x-markdown" => DocumentFormat::Md,
            "text/plain" => DocumentFormat::Txt,
            "text/csv" => DocumentFormat::Csv,
            other => {
                return Err(LlmProviderError::RequestBuild(format!(
                    "Unsupported document format: {other}"
                )));
            }
        };
        let doc_block = DocumentBlock::builder()
            .format(format)
            .name("attachment")
            .source(DocumentSource::Bytes(Blob::new(attachment.data.clone())))
            .build()
            .map_err(|e| {
                LlmProviderError::RequestBuild(format!("Error building DocumentBlock: {e}"))
            })?;
        Ok(ContentBlock::Document(doc_block))
    }
}

fn convert_token_usage(usage: Option<&BedrockTokenUsage>) -> TokenUsage {
    let Some(usage) = usage else {
        return TokenUsage::default();
    };

    TokenUsage {
        input_tokens: usage.input_tokens().max(0) as u64,
        output_tokens: usage.output_tokens().max(0) as u64,
        cache_read_tokens: usage.cache_read_input_tokens().unwrap_or_default().max(0) as u64,
        cache_write_tokens: usage.cache_write_input_tokens().unwrap_or_default().max(0) as u64,
    }
}
