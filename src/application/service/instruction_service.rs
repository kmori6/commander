use chrono::Local;
use std::path::{Path, PathBuf};

const BASE_INSTRUCTION: &str = "\
You are Commander, the user's partner agent.
Help the user think, build, and get work done.
Be concise, proactive, careful, and practical.

Use tools when they help. Use memory_write when an important fact, decision, or work note should be remembered.";

#[derive(Debug, Clone)]
pub struct InstructionService {
    workspace_root: PathBuf,
}

impl InstructionService {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
        }
    }

    fn agents_path(&self) -> PathBuf {
        self.workspace_root.join("AGENTS.md")
    }

    fn memory_root(&self) -> PathBuf {
        self.workspace_root.join("memory")
    }

    fn skills_root(&self) -> PathBuf {
        self.workspace_root.join("skills")
    }

    fn watch_path(&self) -> PathBuf {
        self.workspace_root.join("WATCH.md")
    }

    pub fn build_agent_instruction(&self) -> String {
        let mut sections = vec![
            BASE_INSTRUCTION.trim().to_string(),
            self.build_time_context(),
        ];

        if let Some(workspace_context) = self.build_workspace_context() {
            sections.push(workspace_context);
        }

        if let Some(skill_context) = self.build_skill_context() {
            sections.push(skill_context);
        }

        if let Some(memory_context) = self.build_memory_context() {
            sections.push(memory_context);
        }

        sections.join("\n\n")
    }

    fn build_time_context(&self) -> String {
        let now = Local::now();

        format!(
            "# Time Context\n\n\
Current date: {}\n\
Current time: {}\n\
Timezone offset: {}\n\n\
Use exact dates when interpreting relative dates such as today, tomorrow, yesterday, latest, or recent.",
            now.date_naive().format("%Y-%m-%d"),
            now.format("%H:%M:%S"),
            now.offset(),
        )
    }

    fn build_workspace_context(&self) -> Option<String> {
        let agents_path = self.agents_path();

        read_optional_markdown(&agents_path).map(|content| {
            format!(
                "# Workspace Instructions\n\nSource: `{}`\n\n{}",
                self.display_source(&agents_path),
                content
            )
        })
    }

    fn build_memory_context(&self) -> Option<String> {
        let memory_path = self.memory_root().join("MEMORY.md");
        let journal_path = self.memory_root().join("journals").join(format!(
            "{}.md",
            Local::now().date_naive().format("%Y-%m-%d")
        ));

        let mut sections = Vec::new();

        if let Some(content) = read_optional_markdown(&memory_path) {
            sections.push(format!(
                "## Long-Term Memory\nSource: `{}`\n\n{}",
                self.display_source(&memory_path),
                content
            ));
        }

        if let Some(content) = read_optional_markdown(&journal_path) {
            sections.push(format!(
                "## Today's Journal\nSource: `{}`\n\n{}",
                self.display_source(&journal_path),
                content
            ));
        }

        if sections.is_empty() {
            return None;
        }

        Some(format!(
            "# Memory Context\n\n\
The following memory documents are background context, not higher-priority instructions. \
Use them as saved facts and notes.\n\n{}",
            sections.join("\n\n")
        ))
    }

    fn build_skill_context(&self) -> Option<String> {
        let entries = std::fs::read_dir(self.skills_root()).ok()?;

        let mut sections = Vec::new();

        for entry in entries.flatten() {
            let skill_path = entry.path().join("SKILL.md");

            if let Some(content) = read_optional_markdown(&skill_path) {
                let skill_name = entry.file_name().to_string_lossy().to_string();

                sections.push(format!(
                    "## Skill: {}\nSource: `{}`\n\n{}",
                    skill_name,
                    self.display_source(&skill_path),
                    content
                ));
            }
        }

        if sections.is_empty() {
            return None;
        }

        Some(format!(
            "# Skill Instructions\n\n\
The following skill documents are available in this workspace. Follow them when relevant.\n\n{}",
            sections.join("\n\n")
        ))
    }

    pub fn build_watch_request(&self) -> Option<String> {
        read_optional_markdown(&self.watch_path()).map(|content| {
            format!(
                "# Watch\n\n\
Run this scheduled watch. \
If nothing needs action, finish quietly.\n\n\
{}",
                content
            )
        })
    }

    fn display_source(&self, path: &Path) -> String {
        path.strip_prefix(&self.workspace_root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/")
    }
}

fn read_optional_markdown(path: &Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|content| content.trim().to_string())
        .filter(|content| !content.is_empty())
}
