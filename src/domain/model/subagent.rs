#[derive(Debug, Clone)]
pub struct Subagent {
    pub name: String,
    pub description: String,
    pub instruction: String,
    pub allowed_tools: Vec<String>,
}

impl Subagent {
    pub fn restore(
        name: impl Into<String>,
        description: impl Into<String>,
        instruction: impl Into<String>,
        allowed_tools: Vec<String>,
    ) -> Option<Self> {
        let name = name.into().trim().to_string();
        let instruction = instruction.into().trim().to_string();
        let allowed_tools = allowed_tools
            .into_iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();

        if name.is_empty() || instruction.is_empty() || allowed_tools.is_empty() {
            return None;
        }

        Some(Self {
            name,
            description: description.into().trim().to_string(),
            instruction,
            allowed_tools,
        })
    }

    pub fn allows_tool(&self, tool_name: &str) -> bool {
        self.allowed_tools.iter().any(|tool| tool == tool_name)
    }

    pub fn summary(&self) -> String {
        if self.description.is_empty() {
            self.name.clone()
        } else {
            format!("{}: {}", self.name, self.description)
        }
    }
}
