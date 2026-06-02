use std::env;
use std::io;
use std::path::{Path, PathBuf};

const DEFAULT_COMMANDER_DIR: &str = ".commander";
const DEFAULT_WORKSPACE_DIR: &str = "workspace";

#[derive(Debug, Clone)]
pub struct CommanderPaths {
    home_path: PathBuf,
    workspace_path: PathBuf,
}

impl CommanderPaths {
    pub fn resolve() -> io::Result<Self> {
        let home_path = user_home_dir()?.join(DEFAULT_COMMANDER_DIR);
        let workspace_path = home_path.join(DEFAULT_WORKSPACE_DIR);

        Ok(Self {
            home_path,
            workspace_path,
        })
    }

    pub async fn ensure_dirs(&self) -> io::Result<()> {
        tokio::fs::create_dir_all(self.config_dir()).await?;
        tokio::fs::create_dir_all(self.tools_dir()).await?;
        tokio::fs::create_dir_all(self.schedules_dir()).await?;
        tokio::fs::create_dir_all(&self.workspace_path).await?;
        Ok(())
    }

    pub fn workspace_path(&self) -> &Path {
        &self.workspace_path
    }

    fn config_dir(&self) -> PathBuf {
        self.home_path.join("config")
    }

    pub fn model_config_path(&self) -> PathBuf {
        self.config_dir().join("models.json")
    }

    fn tools_dir(&self) -> PathBuf {
        self.home_path.join("tools")
    }

    pub fn tool_permissions_path(&self) -> PathBuf {
        self.tools_dir().join("permissions.json")
    }

    fn schedules_dir(&self) -> PathBuf {
        self.home_path.join("schedules")
    }

    pub fn schedules_path(&self) -> PathBuf {
        self.schedules_dir().join("crons.json")
    }

    pub fn watch_config_path(&self) -> PathBuf {
        self.config_dir().join("watch.json")
    }

    pub fn sandbox_env_path(&self) -> PathBuf {
        self.config_dir().join(".env")
    }

    pub fn mcp_config_path(&self) -> PathBuf {
        self.config_dir().join("mcp.json")
    }
}

fn user_home_dir() -> io::Result<PathBuf> {
    env::var_os("HOME").map(PathBuf::from).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "failed to resolve home directory from HOME",
        )
    })
}
