//! Package definition parsing and loading

use anyhow::{Context, Result};
use std::path::Path;

use super::PackageDefinition;

impl PackageDefinition {
    /// Load a package definition from a TOML file
    pub fn from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read package definition: {}", path.display()))?;
        Self::from_str(&content)
    }

    /// Parse a package definition from a TOML string
    pub fn from_str(content: &str) -> Result<Self> {
        toml::from_str(content).context("Failed to parse package definition")
    }

    /// Serialize to TOML string
    pub fn to_toml(&self) -> Result<String> {
        toml::to_string_pretty(self).context("Failed to serialize package definition")
    }

    /// Save to a file
    pub fn save(&self, path: &Path) -> Result<()> {
        let content = self.to_toml()?;
        std::fs::write(path, content)
            .with_context(|| format!("Failed to write package definition: {}", path.display()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_package_definition() {
        let toml = r#"
[package]
name = "example"
version = "1.0.0"
description = "An example package"
license = "MIT"
maintainers = ["Test User"]
categories = ["utilities"]

[source]
type = "tarball"
url = "https://example.com/example-1.0.0.tar.gz"
sha256 = "abc123"

[build]
system = "make"

[dependencies]
runtime = []
build = []
"#;
        let def = PackageDefinition::from_str(toml).unwrap();
        assert_eq!(def.package.name, "example");
        assert_eq!(def.package.version.to_string(), "1.0.0");
    }
}
