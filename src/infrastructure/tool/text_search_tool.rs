use std::path::{Path, PathBuf};

use async_trait::async_trait;
use glob::{MatchOptions, Pattern};
use ignore::WalkBuilder;
use regex::Regex;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::domain::error::tool_error::ToolError;
use crate::domain::model::tool_call::ToolPermissionMode;
use crate::domain::port::tool::Tool;

const MAX_MATCHES: usize = 200;
const MAX_FILE_BYTES: u64 = 1_000_000;

#[derive(Debug, Clone)]
pub struct TextSearchTool {
    workspace_root: PathBuf,
}

impl TextSearchTool {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self { workspace_root }
    }

    fn resolve_path(&self, path: &str) -> Result<(PathBuf, PathBuf), ToolError> {
        let path = path.trim();
        let requested = if path.is_empty() {
            Path::new(".")
        } else {
            Path::new(path)
        };

        let workspace_root = std::fs::canonicalize(&self.workspace_root)
            .map_err(|err| ToolError::ExecutionFailed(err.to_string()))?;

        let joined = if requested.is_absolute() {
            requested.to_path_buf()
        } else {
            workspace_root.join(requested)
        };

        let resolved = std::fs::canonicalize(joined)
            .map_err(|err| ToolError::ExecutionFailed(err.to_string()))?;

        if !resolved.starts_with(&workspace_root) {
            return Err(ToolError::ExecutionFailed(format!(
                "path is outside workspace: {path}"
            )));
        }

        Ok((workspace_root, resolved))
    }
}

#[derive(Debug, Deserialize)]
struct TextSearchArguments {
    query: String,
    path: Option<String>,
    include: Option<String>,
}

#[derive(Debug)]
struct TextSearchMatch {
    path: String,
    line: usize,
    text: String,
}

#[async_trait]
impl Tool for TextSearchTool {
    fn name(&self) -> &str {
        "text_search"
    }

    fn description(&self) -> &str {
        "Search UTF-8 workspace files for lines that match a regular expression."
    }

    fn default_permission(&self) -> ToolPermissionMode {
        ToolPermissionMode::Allow
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Regular expression to search for in file contents."
                },
                "path": {
                    "type": "string",
                    "description": "Directory or file path to search. Defaults to the workspace root."
                },
                "include": {
                    "type": "string",
                    "description": "Optional glob filter for matching file paths, such as **/*.rs or data/*.md."
                }
            },
            "required": ["query"],
            "additionalProperties": false
        })
    }

    async fn execute(&self, arguments: Value) -> Result<Value, ToolError> {
        let args: TextSearchArguments = serde_json::from_value(arguments)
            .map_err(|err| ToolError::InvalidArguments(err.to_string()))?;

        let query = args.query.trim();

        if query.is_empty() {
            return Err(ToolError::InvalidArguments(
                "query must not be empty".to_string(),
            ));
        }

        let regex =
            Regex::new(query).map_err(|err| ToolError::InvalidArguments(err.to_string()))?;

        let requested_path = args.path.unwrap_or_else(|| ".".to_string());
        let (workspace_root, search_path) = self.resolve_path(&requested_path)?;

        let include = match args
            .include
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            Some(pattern) => {
                let pattern_path = Path::new(pattern);

                if pattern_path.is_absolute() {
                    return Err(ToolError::InvalidArguments(
                        "include must be relative".to_string(),
                    ));
                }

                if pattern_path
                    .components()
                    .any(|component| matches!(component, std::path::Component::ParentDir))
                {
                    return Err(ToolError::InvalidArguments(
                        "include must not contain '..'".to_string(),
                    ));
                }

                Some(
                    Pattern::new(pattern)
                        .map_err(|err| ToolError::InvalidArguments(err.to_string()))?,
                )
            }
            None => None,
        };

        let mut walker = WalkBuilder::new(&search_path);
        walker.hidden(false);
        walker.git_ignore(true);
        walker.git_exclude(true);
        walker.ignore(true);
        walker.parents(true);
        walker.follow_links(false);

        let glob_options = MatchOptions {
            case_sensitive: true,
            require_literal_separator: false,
            require_literal_leading_dot: false,
        };

        let mut matches = Vec::new();
        let mut truncated = false;

        for entry in walker.build() {
            let Ok(entry) = entry else {
                continue;
            };

            let path = entry.path();

            let Ok(resolved_path) = std::fs::canonicalize(path) else {
                continue;
            };

            if !resolved_path.starts_with(&workspace_root) {
                continue;
            }

            if !resolved_path.is_file() {
                continue;
            }

            let relative = relative_path(&workspace_root, &resolved_path)?;

            if let Some(include) = &include
                && !include.matches_with(&relative, glob_options)
            {
                continue;
            }

            let Ok(metadata) = std::fs::metadata(&resolved_path) else {
                continue;
            };

            if metadata.len() > MAX_FILE_BYTES {
                continue;
            }

            let Ok(bytes) = std::fs::read(&resolved_path) else {
                continue;
            };

            if bytes.contains(&0) {
                continue;
            }

            let Ok(content) = String::from_utf8(bytes) else {
                continue;
            };

            for (line_index, line) in content.lines().enumerate() {
                if !regex.is_match(line) {
                    continue;
                }

                if matches.len() >= MAX_MATCHES {
                    truncated = true;
                    break;
                }

                matches.push(TextSearchMatch {
                    path: relative.clone(),
                    line: line_index + 1,
                    text: line.to_string(),
                });
            }

            if truncated {
                break;
            }
        }

        Ok(json!({
            "matches": matches
                .into_iter()
                .map(|item| {
                    json!({
                        "path": item.path,
                        "line": item.line,
                        "text": item.text
                    })
                })
                .collect::<Vec<_>>(),
            "truncated": truncated
        }))
    }
}

fn relative_path(workspace_root: &Path, path: &Path) -> Result<String, ToolError> {
    let relative = path.strip_prefix(workspace_root).map_err(|err| {
        ToolError::ExecutionFailed(format!("failed to build relative path: {err}"))
    })?;

    if relative.as_os_str().is_empty() {
        Ok(".".to_string())
    } else {
        Ok(relative.to_string_lossy().replace('\\', "/"))
    }
}
