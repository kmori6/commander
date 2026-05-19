use std::collections::HashSet;
use uuid::Uuid;

use crate::domain::error::llm_provider_error::LlmProviderError;
use crate::domain::model::message::{Message, MessageContent, Role};
use crate::domain::port::llm_provider::{LlmMessage, LlmProvider, LlmRequest};

const DEFAULT_COMPACT_AT_PERCENT: usize = 90;
const DEFAULT_KEEP_UNITS: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompactionConfig {
    pub trigger_tokens: usize,
    pub keep_units: usize,
}

impl CompactionConfig {
    pub fn new(trigger_tokens: usize, keep_units: usize) -> Self {
        Self {
            trigger_tokens,
            keep_units: keep_units.max(1),
        }
    }

    pub fn for_window(context_window: usize) -> Self {
        Self::new(
            context_window.saturating_mul(DEFAULT_COMPACT_AT_PERCENT) / 100,
            DEFAULT_KEEP_UNITS,
        )
    }
}

pub struct CompactionResult {
    pub summary: String,
    pub until: Uuid,
}

pub struct CompactionService {
    config: CompactionConfig,
}

pub struct CompactionPlan {
    pub compact: Vec<MessageUnit>,
    pub until: Uuid,
}

#[derive(Clone, Debug)]
pub struct MessageUnit {
    pub messages: Vec<Message>,
    pub complete: bool,
}

impl CompactionService {
    pub fn new(config: CompactionConfig) -> Self {
        Self { config }
    }

    pub async fn compact<L: LlmProvider>(
        &self,
        llm_provider: &L,
        model: &str,
        messages: Vec<Message>,
        previous_summary: Option<&str>,
    ) -> Result<Option<CompactionResult>, LlmProviderError> {
        let Some(plan) = self.plan(messages) else {
            return Ok(None);
        };

        let prompt = self.prompt(&plan, previous_summary);

        let response = llm_provider
            .respond(LlmRequest::new(
                model.to_string(),
                vec![LlmMessage::user_text(prompt)],
            ))
            .await?;

        Ok(Some(CompactionResult {
            summary: response.output_text("\n").trim().to_string(),
            until: plan.until,
        }))
    }

    pub fn plan(&self, messages: Vec<Message>) -> Option<CompactionPlan> {
        if estimate(&messages) < self.config.trigger_tokens {
            return None;
        }

        let units = units(messages);
        if units.len() <= self.config.keep_units {
            return None;
        }

        let mut split = units.len() - self.config.keep_units;
        if let Some(first_incomplete) = units.iter().position(|unit| !unit.complete)
            && first_incomplete < split
        {
            split = first_incomplete;
        }
        if split == 0 {
            return None;
        }

        let compact = units[..split].to_vec();
        let until = compact.last()?.messages.last()?.id;

        Some(CompactionPlan { compact, until })
    }

    pub fn prompt(&self, plan: &CompactionPlan, previous: Option<&str>) -> String {
        let mut text = String::new();

        text.push_str("Summarize the compacted conversation for future turns.\n");
        text.push_str("Preserve active tasks, decisions, constraints, file paths, tool results, and open questions.\n");
        text.push_str("Be concise, but keep enough detail to continue the work safely.\n\n");

        if let Some(previous) = previous {
            text.push_str("# Previous Summary\n");
            text.push_str(previous);
            text.push_str("\n\n");
        }

        text.push_str("# Conversation\n");
        text.push_str(&format_units(&plan.compact));

        text
    }
}

fn units(messages: Vec<Message>) -> Vec<MessageUnit> {
    let mut out = Vec::new();
    let mut i = 0;

    while i < messages.len() {
        let message = messages[i].clone();
        let calls = tool_call_ids(&message);

        if message.role != Role::Assistant || calls.is_empty() {
            out.push(MessageUnit {
                messages: vec![message],
                complete: true,
            });
            i += 1;
            continue;
        }

        let mut unit = vec![message];
        let mut pending = calls.into_iter().collect::<HashSet<_>>();
        i += 1;

        while i < messages.len() && !pending.is_empty() {
            let next = messages[i].clone();
            let outputs = tool_output_ids(&next);

            if outputs.is_empty() {
                break;
            }

            for output in outputs {
                pending.remove(&output);
            }
            unit.push(next);
            i += 1;
        }

        out.push(MessageUnit {
            messages: unit,
            complete: pending.is_empty(),
        });
    }

    out
}

fn tool_call_ids(message: &Message) -> Vec<String> {
    message
        .contents
        .iter()
        .filter_map(|content| match content {
            MessageContent::ToolCall { call_id, .. } => Some(call_id.clone()),
            _ => None,
        })
        .collect()
}

fn tool_output_ids(message: &Message) -> Vec<String> {
    message
        .contents
        .iter()
        .filter_map(|content| match content {
            MessageContent::ToolCallOutput { call_id, .. } => Some(call_id.clone()),
            _ => None,
        })
        .collect()
}

fn estimate(messages: &[Message]) -> usize {
    messages.iter().map(estimate_message).sum()
}

fn estimate_message(message: &Message) -> usize {
    message.contents.iter().map(estimate_content).sum()
}

fn estimate_content(content: &MessageContent) -> usize {
    match content {
        MessageContent::InputText { text } | MessageContent::OutputText { text } => {
            estimate_text(text)
        }
        MessageContent::InputImage { image_url } => estimate_text(image_url),
        MessageContent::InputFile {
            filename,
            file_data,
        } => estimate_text(filename) + estimate_text(file_data),
        MessageContent::ToolCall {
            call_id,
            tool_name,
            arguments,
        } => {
            estimate_text(call_id)
                + estimate_text(tool_name)
                + estimate_text(&arguments.to_string())
        }
        MessageContent::ToolCallOutput {
            call_id,
            output,
            status,
        } => {
            estimate_text(call_id)
                + estimate_text(status.as_str())
                + estimate_text(&output.to_string())
        }
    }
}

fn estimate_text(text: &str) -> usize {
    text.len().div_ceil(4).max(1)
}

fn format_units(units: &[MessageUnit]) -> String {
    let mut text = String::new();

    for unit in units {
        if unit.complete {
            text.push_str("<unit>\n");
        } else {
            text.push_str("<unit incomplete=\"true\">\n");
        }

        for message in &unit.messages {
            format_message(&mut text, message);
        }

        text.push_str("</unit>\n\n");
    }

    text
}

fn format_message(text: &mut String, message: &Message) {
    text.push_str(&format!(
        "[{} {}]\n",
        message.created_at.to_rfc3339(),
        message.role.as_str()
    ));

    for content in &message.contents {
        text.push_str(&format_content(content));
        text.push('\n');
    }
}

fn format_content(content: &MessageContent) -> String {
    match content {
        MessageContent::InputText { text } | MessageContent::OutputText { text } => text.clone(),
        MessageContent::InputImage { image_url } => format!("[image] {image_url}"),
        MessageContent::InputFile {
            filename,
            file_data,
        } => format!("[file] {filename}\n{file_data}"),
        MessageContent::ToolCall {
            call_id,
            tool_name,
            arguments,
        } => format!("[tool_call] id={call_id} name={tool_name} arguments={arguments}"),
        MessageContent::ToolCallOutput {
            call_id,
            output,
            status,
        } => format!(
            "[tool_output] id={} status={} output={}",
            call_id,
            status.as_str(),
            output
        ),
    }
}
