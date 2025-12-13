use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub name: String,
    pub path: PathBuf,
    pub created: chrono::DateTime<chrono::Utc>,
    pub languages: Vec<LanguageConfig>,
    pub packages: Vec<String>,
    pub env: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageConfig {
    pub name: String,
    pub version: Option<String>,
    pub packages: Vec<String>,
}

impl Workspace {
    pub fn new(name: &str) -> Self {
        let base_path = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("~/.local/share"))
            .join("rvn/workspaces")
            .join(name);

        Self {
            name: name.to_string(),
            path: base_path,
            created: chrono::Utc::now(),
            languages: Vec::new(),
            packages: Vec::new(),
            env: HashMap::new(),
        }
    }

    pub fn add_language(&mut self, name: &str, version: Option<&str>) {
        self.languages.push(LanguageConfig {
            name: name.to_string(),
            version: version.map(String::from),
            packages: Vec::new(),
        });
    }

    pub fn generate_activation_script(&self) -> String {
        let mut script = String::new();

        script.push_str("# RavenLinux Workspace Activation Script\n");
        script.push_str(&format!("# Workspace: {}\n\n", self.name));

        // Set workspace-specific PATH
        script.push_str(&format!("export RAVEN_WORKSPACE=\"{}\"\n", self.name));
        script.push_str(&format!(
            "export RAVEN_WORKSPACE_PATH=\"{}\"\n",
            self.path.display()
        ));

        // Add workspace bin to PATH
        script.push_str(&format!(
            "export PATH=\"{}/bin:$PATH\"\n",
            self.path.display()
        ));

        // Set custom environment variables
        for (key, value) in &self.env {
            script.push_str(&format!("export {}=\"{}\"\n", key, value));
        }

        // Language-specific setup
        for lang in &self.languages {
            script.push_str(&format!("\n# {} setup\n", lang.name));
            match lang.name.as_str() {
                "rust" => {
                    if let Some(version) = &lang.version {
                        script.push_str(&format!("rustup override set {}\n", version));
                    }
                }
                "node" => {
                    if let Some(version) = &lang.version {
                        script.push_str(&format!("fnm use {}\n", version));
                    }
                }
                "python" => {
                    if let Some(version) = &lang.version {
                        script.push_str(&format!("pyenv local {}\n", version));
                    }
                }
                _ => {}
            }
        }

        script.push_str("\necho \"Workspace '");
        script.push_str(&self.name);
        script.push_str("' activated\"\n");

        script
    }

    pub fn save(&self) -> Result<()> {
        std::fs::create_dir_all(&self.path)?;
        let config_path = self.path.join("workspace.toml");
        let content = toml::to_string_pretty(self)?;
        std::fs::write(config_path, content)?;
        Ok(())
    }

    pub fn load(name: &str) -> Result<Self> {
        let base_path = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("~/.local/share"))
            .join("rvn/workspaces")
            .join(name);

        let config_path = base_path.join("workspace.toml");
        let content = std::fs::read_to_string(config_path)?;
        let workspace: Workspace = toml::from_str(&content)?;
        Ok(workspace)
    }
}

mod dirs {
    use std::path::PathBuf;

    pub fn data_local_dir() -> Option<PathBuf> {
        std::env::var("XDG_DATA_HOME")
            .ok()
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var("HOME")
                    .ok()
                    .map(|h| PathBuf::from(h).join(".local/share"))
            })
    }
}
