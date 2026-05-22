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
        let mut tools = Vec::new();

        for tool in allowed_tools {
            let tool = tool.trim().to_string();

            if !tool.is_empty() && !tools.contains(&tool) {
                tools.push(tool);
            }
        }

        if name.is_empty() || instruction.is_empty() || tools.is_empty() {
            return None;
        }

        Some(Self {
            name,
            description: description.into().trim().to_string(),
            instruction,
            allowed_tools: tools,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restore_trims_fields_and_tools() {
        let subagent = Subagent::restore(
            " reviewer ",
            " code review ",
            " review carefully ",
            vec![
                " shell ".to_string(),
                "".to_string(),
                "shell".to_string(),
                " read ".to_string(),
            ],
        )
        .unwrap();

        assert_eq!("reviewer", subagent.name);
        assert_eq!("code review", subagent.description);
        assert_eq!("review carefully", subagent.instruction);
        assert_eq!(
            vec!["shell".to_string(), "read".to_string()],
            subagent.allowed_tools
        );
    }

    #[test]
    fn restore_rejects_empty_required_fields() {
        assert!(Subagent::restore("", "desc", "instruction", vec!["shell".to_string()]).is_none());
        assert!(Subagent::restore("name", "desc", "", vec!["shell".to_string()]).is_none());
        assert!(Subagent::restore("name", "desc", "instruction", vec![" ".to_string()]).is_none());
    }

    #[test]
    fn allows_configured_tool_only() {
        let subagent =
            Subagent::restore("reviewer", "", "review", vec!["shell".to_string()]).unwrap();

        assert!(subagent.allows_tool("shell"));
        assert!(!subagent.allows_tool("write"));
    }

    #[test]
    fn summary_uses_description_when_present() {
        let subagent = Subagent::restore(
            "reviewer",
            "code review",
            "review",
            vec!["shell".to_string()],
        )
        .unwrap();

        assert_eq!("reviewer: code review", subagent.summary());
    }

    #[test]
    fn summary_uses_name_without_description() {
        let subagent =
            Subagent::restore("reviewer", "", "review", vec!["shell".to_string()]).unwrap();

        assert_eq!("reviewer", subagent.summary());
    }
}
