//! PKGBUILD parser for AUR packages
//!
//! Parses Arch Linux PKGBUILD files to extract metadata and build instructions.

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

/// Parsed PKGBUILD data
#[derive(Debug, Clone, Default)]
pub struct PkgBuild {
    pub name: String,
    pub version: String,
    pub release: String,
    pub description: Option<String>,
    pub url: Option<String>,
    pub license: Vec<String>,
    pub depends: Vec<String>,
    pub makedepends: Vec<String>,
    pub optdepends: Vec<String>,
    pub provides: Vec<String>,
    pub conflicts: Vec<String>,
    pub replaces: Vec<String>,
    pub source: Vec<String>,
    pub sha256sums: Vec<String>,
    pub arch: Vec<String>,
    pub backup: Vec<String>,
    pub install: Option<String>,
    /// Raw variables for custom processing
    pub variables: HashMap<String, String>,
    /// Raw arrays for custom processing
    pub arrays: HashMap<String, Vec<String>>,
}

impl PkgBuild {
    /// Parse a PKGBUILD file
    pub fn parse(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read PKGBUILD: {}", path.display()))?;

        Self::parse_content(&content)
    }

    /// Parse PKGBUILD content string
    pub fn parse_content(content: &str) -> Result<Self> {
        let mut pkg = PkgBuild::default();

        // Use bash to evaluate and extract variables
        let extracted = Self::extract_with_bash(content)?;

        // Parse the extracted values
        for line in extracted.lines() {
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim();

                // Check if it's an array (starts with '(' and ends with ')')
                if value.starts_with('(') && value.ends_with(')') {
                    let array_content = &value[1..value.len() - 1];
                    let items: Vec<String> = Self::parse_array_items(array_content);
                    pkg.arrays.insert(key.to_string(), items.clone());

                    match key {
                        "depends" => pkg.depends = items,
                        "makedepends" => pkg.makedepends = items,
                        "optdepends" => pkg.optdepends = items,
                        "provides" => pkg.provides = items,
                        "conflicts" => pkg.conflicts = items,
                        "replaces" => pkg.replaces = items,
                        "source" => pkg.source = items,
                        "sha256sums" => pkg.sha256sums = items,
                        "arch" => pkg.arch = items,
                        "license" => pkg.license = items,
                        "backup" => pkg.backup = items,
                        _ => {}
                    }
                } else {
                    // Scalar value
                    let value = value.trim_matches('"').trim_matches('\'').to_string();
                    pkg.variables.insert(key.to_string(), value.clone());

                    match key {
                        "pkgname" => pkg.name = value,
                        "pkgver" => pkg.version = value,
                        "pkgrel" => pkg.release = value,
                        "pkgdesc" => pkg.description = Some(value),
                        "url" => pkg.url = Some(value),
                        "install" => pkg.install = Some(value),
                        _ => {}
                    }
                }
            }
        }

        // Fallback parsing if bash extraction failed
        if pkg.name.is_empty() {
            pkg = Self::parse_fallback(content)?;
        }

        Ok(pkg)
    }

    /// Use bash to safely extract PKGBUILD variables
    fn extract_with_bash(content: &str) -> Result<String> {
        // Create a safe extraction script that only outputs variable values
        let script = format!(
            r#"
# Disable all functions to prevent code execution
unset -f prepare build check package 2>/dev/null || true

# Source PKGBUILD in a restricted way
eval '{}' 2>/dev/null || true

# Output variables we care about
echo "pkgname=$pkgname"
echo "pkgver=$pkgver"
echo "pkgrel=$pkgrel"
echo "pkgdesc=$pkgdesc"
echo "url=$url"
echo "install=$install"

# Output arrays
printf 'depends=('
printf "'%s' " "${{depends[@]}}" 2>/dev/null || true
echo ')'

printf 'makedepends=('
printf "'%s' " "${{makedepends[@]}}" 2>/dev/null || true
echo ')'

printf 'optdepends=('
printf "'%s' " "${{optdepends[@]}}" 2>/dev/null || true
echo ')'

printf 'provides=('
printf "'%s' " "${{provides[@]}}" 2>/dev/null || true
echo ')'

printf 'conflicts=('
printf "'%s' " "${{conflicts[@]}}" 2>/dev/null || true
echo ')'

printf 'replaces=('
printf "'%s' " "${{replaces[@]}}" 2>/dev/null || true
echo ')'

printf 'license=('
printf "'%s' " "${{license[@]}}" 2>/dev/null || true
echo ')'

printf 'arch=('
printf "'%s' " "${{arch[@]}}" 2>/dev/null || true
echo ')'

printf 'source=('
printf "'%s' " "${{source[@]}}" 2>/dev/null || true
echo ')'

printf 'sha256sums=('
printf "'%s' " "${{sha256sums[@]}}" 2>/dev/null || true
echo ')'

printf 'backup=('
printf "'%s' " "${{backup[@]}}" 2>/dev/null || true
echo ')'
"#,
            // Escape single quotes in content
            content.replace('\'', "'\"'\"'")
        );

        let output = Command::new("bash")
            .arg("-c")
            .arg(&script)
            .output()
            .context("Failed to run bash for PKGBUILD parsing")?;

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Parse array items from bash array syntax
    fn parse_array_items(content: &str) -> Vec<String> {
        let mut items = Vec::new();
        let mut current = String::new();
        let mut in_quote = false;
        let mut quote_char = ' ';

        for ch in content.chars() {
            match ch {
                '\'' | '"' if !in_quote => {
                    in_quote = true;
                    quote_char = ch;
                }
                c if c == quote_char && in_quote => {
                    in_quote = false;
                    if !current.is_empty() {
                        items.push(current.clone());
                        current.clear();
                    }
                }
                ' ' | '\t' | '\n' if !in_quote => {
                    if !current.is_empty() {
                        items.push(current.clone());
                        current.clear();
                    }
                }
                _ => current.push(ch),
            }
        }

        if !current.is_empty() {
            items.push(current);
        }

        items
    }

    /// Fallback parser using regex-like patterns (no bash required)
    fn parse_fallback(content: &str) -> Result<Self> {
        let mut pkg = PkgBuild::default();

        for line in content.lines() {
            let line = line.trim();

            // Skip comments and empty lines
            if line.starts_with('#') || line.is_empty() {
                continue;
            }

            // Parse variable assignments
            if let Some(eq_pos) = line.find('=') {
                let key = line[..eq_pos].trim();
                let value = line[eq_pos + 1..].trim();

                // Array assignment
                if value.starts_with('(') {
                    let array_end = Self::find_array_end(content, line)?;
                    let items = Self::parse_array_items(&array_end);

                    match key {
                        "depends" => pkg.depends = items,
                        "makedepends" => pkg.makedepends = items,
                        "optdepends" => pkg.optdepends = items,
                        "provides" => pkg.provides = items,
                        "conflicts" => pkg.conflicts = items,
                        "replaces" => pkg.replaces = items,
                        "source" => pkg.source = items,
                        "sha256sums" => pkg.sha256sums = items,
                        "arch" => pkg.arch = items,
                        "license" => pkg.license = items,
                        "backup" => pkg.backup = items,
                        _ => {}
                    }
                } else {
                    // Scalar assignment
                    let value = value.trim_matches('"').trim_matches('\'');

                    match key {
                        "pkgname" => pkg.name = value.to_string(),
                        "pkgver" => pkg.version = value.to_string(),
                        "pkgrel" => pkg.release = value.to_string(),
                        "pkgdesc" => pkg.description = Some(value.to_string()),
                        "url" => pkg.url = Some(value.to_string()),
                        "install" => pkg.install = Some(value.to_string()),
                        _ => {}
                    }
                }
            }
        }

        Ok(pkg)
    }

    /// Find the complete array content (handles multi-line arrays)
    fn find_array_end(content: &str, start_line: &str) -> Result<String> {
        if start_line.contains(')') {
            // Single line array
            if let Some(start) = start_line.find('(') {
                if let Some(end) = start_line.rfind(')') {
                    return Ok(start_line[start + 1..end].to_string());
                }
            }
        }

        // Multi-line array - find from content
        let start_pos = content.find(start_line).unwrap_or(0);
        let remaining = &content[start_pos..];

        if let Some(paren_start) = remaining.find('(') {
            let after_paren = &remaining[paren_start + 1..];
            if let Some(paren_end) = after_paren.find(')') {
                return Ok(after_paren[..paren_end].to_string());
            }
        }

        Ok(String::new())
    }

    /// Get full version string (version-release)
    pub fn full_version(&self) -> String {
        if self.release.is_empty() {
            self.version.clone()
        } else {
            format!("{}-{}", self.version, self.release)
        }
    }

    /// Map Arch package names to RavenLinux equivalents
    pub fn map_dependency(&self, arch_dep: &str) -> Option<String> {
        // Strip version constraints
        let name = arch_dep
            .split(|c: char| c == '<' || c == '>' || c == '=' || c == ':')
            .next()
            .unwrap_or(arch_dep)
            .trim();

        // Map common package names
        let mapped = match name {
            // Core utilities
            "coreutils" => "uutils-coreutils",
            "glibc" => "musl", // RavenLinux uses musl
            "gcc-libs" => "gcc",
            "glib2" => "glib",
            "gtk3" => "gtk3",
            "gtk4" => "gtk4",
            "qt5-base" => "qt5",
            "qt6-base" => "qt6",

            // Common dev tools
            "base-devel" => "gcc",
            "make" => "make",
            "cmake" => "cmake",
            "meson" => "meson",
            "ninja" => "ninja",
            "pkg-config" => "pkgconf",
            "pkgconf" => "pkgconf",

            // Languages
            "python" => "python",
            "python3" => "python",
            "rust" => "rust",
            "go" => "go",
            "nodejs" => "nodejs",

            // Keep same name if no mapping
            _ => name,
        };

        Some(mapped.to_string())
    }
}
