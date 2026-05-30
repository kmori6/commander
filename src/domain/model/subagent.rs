#[derive(Debug, Clone)]
pub struct Subagent {
    pub name: String,
    pub description: String,
    pub instruction: String,
    pub allowed_tools: Vec<String>,
}
