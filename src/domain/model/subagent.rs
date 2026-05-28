#[derive(Debug, Clone)]
pub struct Subagent {
    pub name: String,
    pub description: String,
    pub instruction: String,
    pub allowed_tools: Vec<String>,
}

impl Subagent {
    pub fn try_new(
        name: impl Into<String>,
        description: impl Into<String>,
        instruction: impl Into<String>,
        allowed_tools: Vec<String>,
    ) -> Result<Self, String> {
        let name = name.into().trim().to_string();
        let instruction = instruction.into().trim().to_string();
        let mut tools = Vec::new();

        for tool in allowed_tools {
            let tool = tool.trim().to_string();

            if !tool.is_empty() && !tools.contains(&tool) {
                tools.push(tool);
            }
        }

        if name.is_empty() {
            return Err("name must not be empty".to_string());
        }

        if instruction.is_empty() {
            return Err("instruction must not be empty".to_string());
        }

        Ok(Self {
            name,
            description: description.into().trim().to_string(),
            instruction,
            allowed_tools: tools,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_new_trims_fields_and_tools() {
        let subagent = Subagent::try_new(
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
    fn try_new_rejects_empty_required_fields() {
        assert_eq!(
            "name must not be empty",
            Subagent::try_new("", "desc", "instruction", vec!["shell".to_string()]).unwrap_err()
        );
        assert_eq!(
            "instruction must not be empty",
            Subagent::try_new("name", "desc", "", vec!["shell".to_string()]).unwrap_err()
        );
    }

    #[test]
    fn try_new_allows_no_tools() {
        let subagent =
            Subagent::try_new("planner", "", "plan only", vec![" ".to_string()]).unwrap();

        assert!(subagent.allowed_tools.is_empty());
    }
}
