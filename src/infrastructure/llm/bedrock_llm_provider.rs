use async_trait::async_trait;
use aws_config::BehaviorVersion;
use aws_sdk_bedrockruntime::Client;
use aws_sdk_bedrockruntime::types::{
    ContentBlock, ConversationRole, DocumentBlock, DocumentFormat, DocumentSource, ImageBlock,
    ImageFormat, ImageSource, JsonSchemaDefinition, Message as BedrockMessage, OutputConfig,
    OutputFormat, OutputFormatStructure, OutputFormatType, SystemContentBlock, TokenUsage, Tool,
    ToolConfiguration, ToolInputSchema, ToolResultBlock, ToolResultContentBlock, ToolResultStatus,
    ToolSpecification, ToolUseBlock,
};
use aws_smithy_types::error::metadata::ProvideErrorMetadata;
use aws_smithy_types::{Blob, Document, Number};
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

use crate::domain::error::llm_provider_error::LlmProviderError;
use crate::domain::model::message::{MessageContent, Role, ToolCallOutputStatus};
use crate::domain::model::token_usage::TokenUsageCounts;
use crate::domain::model::tool_call::ToolSpec;
use crate::domain::port::llm_provider::{
    LlmMessage, LlmProvider, LlmRequest, LlmResponse, StructuredOutputSchema,
};
use crate::domain::util::data_uri::decode_data_uri;

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
}

#[async_trait]
impl LlmProvider for BedrockLlmProvider {
    async fn respond(&self, request: LlmRequest) -> Result<LlmResponse, LlmProviderError> {
        if !request.tools.is_empty() && request.structured_output.is_some() {
            return Err(LlmProviderError::RequestBuild(
                "combining tools and structured output is not supported yet".to_string(),
            ));
        }

        let system_blocks = build_system_content_blocks(&request.messages)?;
        let message_blocks = build_content_blocks(&request.messages)?;

        let mut req = self
            .client
            .converse()
            .model_id(request.model)
            .set_messages(Some(message_blocks));

        for block in system_blocks {
            req = req.system(block);
        }

        if !request.tools.is_empty() {
            req = req.tool_config(tool_configuration(&request.tools)?);
        }

        if let Some(schema) = request.structured_output.as_ref() {
            req = req.output_config(structured_output_config(schema)?);
        }

        let output = req.send().await.map_err(|err| {
            let code = err.code().unwrap_or("unknown");
            let message = err.message().unwrap_or("no message");
            LlmProviderError::ApiCall(format!(
                "Bedrock converse error: code={code}, message={message}, debug={err:?}"
            ))
        })?;

        let usage = convert_token_usage(output.usage());

        let output_blocks = output
            .output()
            .ok_or_else(|| {
                LlmProviderError::ResponseParse("no output in Bedrock response".to_string())
            })?
            .as_message()
            .map_err(|_| {
                LlmProviderError::ResponseParse(
                    "unsupported output type in Bedrock response".to_string(),
                )
            })?
            .content();

        let mut contents = Vec::new();

        for block in output_blocks {
            if let Ok(text) = block.as_text() {
                contents.push(MessageContent::OutputText {
                    text: text.to_string(),
                });
                continue;
            }

            if let Ok(tool_use) = block.as_tool_use() {
                contents.push(MessageContent::ToolCall {
                    call_id: tool_use.tool_use_id().to_string(),
                    tool_name: tool_use.name().to_string(),
                    arguments: document_to_json(tool_use.input())?,
                });
            }
        }

        if contents.is_empty() {
            contents.push(MessageContent::output_text(""));
        }

        Ok(LlmResponse {
            message: LlmMessage::new(Role::Assistant, contents),
            usage,
        })
    }
}

fn build_system_content_blocks(
    messages: &[LlmMessage],
) -> Result<Vec<SystemContentBlock>, LlmProviderError> {
    let mut blocks = Vec::new();

    for message in messages
        .iter()
        .filter(|message| message.role == Role::System)
    {
        for content in &message.contents {
            match content {
                MessageContent::InputText { text } => {
                    blocks.push(SystemContentBlock::Text(text.clone()));
                }
                _ => {
                    return Err(LlmProviderError::RequestBuild(
                        "system messages can only contain input_text".to_string(),
                    ));
                }
            }
        }
    }

    Ok(blocks)
}

fn build_content_blocks(messages: &[LlmMessage]) -> Result<Vec<BedrockMessage>, LlmProviderError> {
    let mut message_blocks = Vec::new();
    let mut current_role: Option<Role> = None;
    let mut current_contents: Vec<ContentBlock> = Vec::new();

    for message in messages
        .iter()
        .filter(|message| message.role != Role::System)
    {
        let blocks = message
            .contents
            .iter()
            .map(|content| message_content_to_content_block(message.role, content))
            .collect::<Result<Vec<_>, _>>()?;

        if current_role == Some(message.role) {
            current_contents.extend(blocks);
        } else {
            push_bedrock_message(&mut message_blocks, current_role, &mut current_contents)?;
            current_role = Some(message.role);
            current_contents = blocks;
        }
    }

    push_bedrock_message(&mut message_blocks, current_role, &mut current_contents)?;

    Ok(message_blocks)
}

fn message_content_to_content_block(
    role: Role,
    content: &MessageContent,
) -> Result<ContentBlock, LlmProviderError> {
    match content {
        MessageContent::InputText { text } | MessageContent::OutputText { text } => {
            Ok(ContentBlock::Text(text.clone()))
        }
        MessageContent::InputImage { image_url } => {
            if role != Role::User {
                return Err(LlmProviderError::RequestBuild(
                    "images must be in user messages for Bedrock Converse".to_string(),
                ));
            }

            input_image_to_content_block(image_url)
        }
        MessageContent::InputFile { file_data, .. } => {
            if role != Role::User {
                return Err(LlmProviderError::RequestBuild(
                    "documents must be in user messages for Bedrock Converse".to_string(),
                ));
            }

            input_file_to_content_block(file_data)
        }
        MessageContent::ToolCall {
            call_id,
            tool_name,
            arguments,
        } => {
            if role != Role::Assistant {
                return Err(LlmProviderError::RequestBuild(
                    "tool calls must be in assistant messages".to_string(),
                ));
            }

            let tool_use = ToolUseBlock::builder()
                .tool_use_id(call_id.clone())
                .name(tool_name.clone())
                .input(json_to_document(arguments)?)
                .build()
                .map_err(|err| {
                    LlmProviderError::RequestBuild(format!(
                        "failed to build Bedrock tool use block: {err}"
                    ))
                })?;

            Ok(ContentBlock::ToolUse(tool_use))
        }
        MessageContent::ToolCallOutput {
            call_id,
            output,
            status,
        } => {
            if role != Role::User {
                return Err(LlmProviderError::RequestBuild(
                    "tool call outputs must be in user messages".to_string(),
                ));
            }

            let status = match status {
                ToolCallOutputStatus::Success => ToolResultStatus::Success,
                ToolCallOutputStatus::Error => ToolResultStatus::Error,
            };

            let block = ToolResultBlock::builder()
                .tool_use_id(call_id.clone())
                .content(ToolResultContentBlock::Json(json_to_document(output)?))
                .status(status)
                .build()
                .map_err(|err| {
                    LlmProviderError::RequestBuild(format!(
                        "failed to build Bedrock tool result block: {err}"
                    ))
                })?;

            Ok(ContentBlock::ToolResult(block))
        }
    }
}

fn push_bedrock_message(
    message_blocks: &mut Vec<BedrockMessage>,
    role: Option<Role>,
    contents: &mut Vec<ContentBlock>,
) -> Result<(), LlmProviderError> {
    let Some(role) = role else {
        return Ok(());
    };

    if contents.is_empty() {
        return Ok(());
    }

    let conversation_role = match role {
        Role::User => ConversationRole::User,
        Role::Assistant => ConversationRole::Assistant,
        Role::System => {
            return Err(LlmProviderError::RequestBuild(
                "system messages must be converted to Bedrock system blocks".to_string(),
            ));
        }
    };

    let mut builder = BedrockMessage::builder().role(conversation_role);
    for content in contents.drain(..) {
        builder = builder.content(content);
    }

    let message = builder.build().map_err(|err| {
        LlmProviderError::RequestBuild(format!("failed to build Bedrock message: {err}"))
    })?;

    message_blocks.push(message);

    Ok(())
}

fn tool_configuration(tools: &[ToolSpec]) -> Result<ToolConfiguration, LlmProviderError> {
    let mut builder = ToolConfiguration::builder();

    for tool in tools {
        let spec = ToolSpecification::builder()
            .name(tool.name.clone())
            .description(tool.description.clone())
            .input_schema(ToolInputSchema::Json(json_to_document(&tool.parameters)?))
            .build()
            .map_err(|err| {
                LlmProviderError::RequestBuild(format!(
                    "failed to build Bedrock tool specification: {err}"
                ))
            })?;

        builder = builder.tools(Tool::ToolSpec(spec));
    }

    builder.build().map_err(|err| {
        LlmProviderError::RequestBuild(format!("failed to build Bedrock tool configuration: {err}"))
    })
}

fn structured_output_config(
    schema: &StructuredOutputSchema,
) -> Result<OutputConfig, LlmProviderError> {
    let schema_string = serde_json::to_string(&schema.schema)
        .map_err(|err| LlmProviderError::RequestBuild(format!("invalid JSON schema: {err}")))?;

    let json_schema = JsonSchemaDefinition::builder()
        .name(schema.name.clone())
        .set_description(schema.description.clone())
        .schema(schema_string)
        .build()
        .map_err(|err| {
            LlmProviderError::RequestBuild(format!("failed to build JSON schema: {err}"))
        })?;

    let text_format = OutputFormat::builder()
        .r#type(OutputFormatType::JsonSchema)
        .structure(OutputFormatStructure::JsonSchema(json_schema))
        .build()
        .map_err(|err| {
            LlmProviderError::RequestBuild(format!("failed to build output format: {err}"))
        })?;

    Ok(OutputConfig::builder().text_format(text_format).build())
}

fn document_to_json(document: &Document) -> Result<Value, LlmProviderError> {
    match document {
        Document::Object(object) => {
            let mut map = serde_json::Map::new();
            for (key, value) in object {
                map.insert(key.clone(), document_to_json(value)?);
            }
            Ok(Value::Object(map))
        }
        Document::Array(array) => Ok(Value::Array(
            array
                .iter()
                .map(document_to_json)
                .collect::<Result<Vec<_>, _>>()?,
        )),
        Document::Number(number) => match number {
            Number::PosInt(value) => Ok(Value::Number((*value).into())),
            Number::NegInt(value) => Ok(Value::Number((*value).into())),
            Number::Float(value) => serde_json::Number::from_f64(*value)
                .map(Value::Number)
                .ok_or_else(|| {
                    LlmProviderError::ResponseParse(format!(
                        "Bedrock document contains non-finite float: {value}"
                    ))
                }),
        },
        Document::String(value) => Ok(Value::String(value.clone())),
        Document::Bool(value) => Ok(Value::Bool(*value)),
        Document::Null => Ok(Value::Null),
    }
}

fn json_to_document(value: &Value) -> Result<Document, LlmProviderError> {
    match value {
        Value::Object(object) => {
            let mut map = HashMap::new();
            for (key, value) in object {
                map.insert(key.clone(), json_to_document(value)?);
            }
            Ok(Document::Object(map))
        }
        Value::Array(array) => Ok(Document::Array(
            array
                .iter()
                .map(json_to_document)
                .collect::<Result<Vec<_>, _>>()?,
        )),
        Value::Number(number) => {
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
                    "unsupported JSON number for Bedrock document: {number}"
                )))
            }
        }
        Value::String(value) => Ok(Document::String(value.clone())),
        Value::Bool(value) => Ok(Document::Bool(*value)),
        Value::Null => Ok(Document::Null),
    }
}

fn input_image_to_content_block(image_url: &str) -> Result<ContentBlock, LlmProviderError> {
    let decoded = decode_data_uri(image_url)
        .map_err(|err| LlmProviderError::RequestBuild(format!("invalid image data URI: {err}")))?;

    let format = match decoded.mime_type.as_str() {
        "image/png" => ImageFormat::Png,
        "image/jpeg" | "image/jpg" => ImageFormat::Jpeg,
        "image/gif" => ImageFormat::Gif,
        "image/webp" => ImageFormat::Webp,
        other => {
            return Err(LlmProviderError::RequestBuild(format!(
                "unsupported image format: {other}"
            )));
        }
    };

    let image_block = ImageBlock::builder()
        .format(format)
        .source(ImageSource::Bytes(Blob::new(decoded.data)))
        .build()
        .map_err(|err| {
            LlmProviderError::RequestBuild(format!("failed to build Bedrock image block: {err}"))
        })?;

    Ok(ContentBlock::Image(image_block))
}

fn input_file_to_content_block(file_data: &str) -> Result<ContentBlock, LlmProviderError> {
    let decoded = decode_data_uri(file_data)
        .map_err(|err| LlmProviderError::RequestBuild(format!("invalid file data URI: {err}")))?;

    let format = match decoded.mime_type.as_str() {
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
                "unsupported document format: {other}"
            )));
        }
    };

    let document_block = DocumentBlock::builder()
        .format(format)
        .name(bedrock_document_name())
        .source(DocumentSource::Bytes(Blob::new(decoded.data)))
        .build()
        .map_err(|err| {
            LlmProviderError::RequestBuild(format!("failed to build Bedrock document block: {err}"))
        })?;

    Ok(ContentBlock::Document(document_block))
}

fn bedrock_document_name() -> String {
    format!("document-{}", Uuid::new_v4())
}

fn convert_token_usage(usage: Option<&TokenUsage>) -> TokenUsageCounts {
    let Some(usage) = usage else {
        return TokenUsageCounts::default();
    };

    TokenUsageCounts {
        input_tokens: i64::from(usage.input_tokens().max(0)),
        output_tokens: i64::from(usage.output_tokens().max(0)),
        cache_read_tokens: i64::from(usage.cache_read_input_tokens().unwrap_or_default().max(0)),
        cache_write_tokens: i64::from(usage.cache_write_input_tokens().unwrap_or_default().max(0)),
    }
}
