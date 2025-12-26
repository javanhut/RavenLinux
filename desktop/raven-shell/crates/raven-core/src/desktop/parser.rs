use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::debug;

/// Represents a parsed .desktop file entry
#[derive(Debug, Clone, Default)]
pub struct DesktopEntry {
    pub name: String,
    pub generic_name: Option<String>,
    pub comment: Option<String>,
    pub exec: String,
    pub icon: Option<String>,
    pub categories: Vec<String>,
    pub keywords: Vec<String>,
    pub terminal: bool,
    pub no_display: bool,
    pub hidden: bool,
    pub path: PathBuf,
}

impl DesktopEntry {
    /// Get the command to execute (strips field codes like %f, %u, etc.)
    pub fn command(&self) -> String {
        // Remove field codes from exec string
        self.exec
            .split_whitespace()
            .filter(|s| !s.starts_with('%'))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Get the primary category
    pub fn primary_category(&self) -> &str {
        self.categories.first().map(|s| s.as_str()).unwrap_or("Other")
    }
}

/// Parse a single .desktop file
pub fn parse_desktop_file(path: &Path) -> Option<DesktopEntry> {
    let content = std::fs::read_to_string(path).ok()?;

    let mut entry = DesktopEntry {
        path: path.to_path_buf(),
        ..Default::default()
    };

    let mut in_desktop_entry = false;

    for line in content.lines() {
        let line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Check for section header
        if line.starts_with('[') {
            in_desktop_entry = line == "[Desktop Entry]";
            continue;
        }

        // Only process lines in [Desktop Entry] section
        if !in_desktop_entry {
            continue;
        }

        // Parse key=value pairs
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim();

            match key {
                "Name" => entry.name = value.to_string(),
                "GenericName" => entry.generic_name = Some(value.to_string()),
                "Comment" => entry.comment = Some(value.to_string()),
                "Exec" => entry.exec = value.to_string(),
                "Icon" => entry.icon = Some(value.to_string()),
                "Categories" => {
                    entry.categories = value
                        .split(';')
                        .filter(|s| !s.is_empty())
                        .map(|s| s.to_string())
                        .collect();
                }
                "Keywords" => {
                    entry.keywords = value
                        .split(';')
                        .filter(|s| !s.is_empty())
                        .map(|s| s.to_string())
                        .collect();
                }
                "Terminal" => entry.terminal = value.eq_ignore_ascii_case("true"),
                "NoDisplay" => entry.no_display = value.eq_ignore_ascii_case("true"),
                "Hidden" => entry.hidden = value.eq_ignore_ascii_case("true"),
                _ => {}
            }
        }
    }

    // Validate required fields
    if entry.name.is_empty() || entry.exec.is_empty() {
        return None;
    }

    // Skip hidden/nodisplay entries
    if entry.no_display || entry.hidden {
        return None;
    }

    Some(entry)
}

/// Get all applications from standard directories
pub fn get_applications() -> Vec<DesktopEntry> {
    let dirs = [
        PathBuf::from("/usr/share/applications"),
        PathBuf::from("/usr/local/share/applications"),
        dirs::home_dir()
            .map(|h| h.join(".local/share/applications"))
            .unwrap_or_default(),
    ];

    let mut apps = Vec::new();
    let mut seen: HashMap<String, usize> = HashMap::new();

    for dir in dirs {
        if !dir.exists() {
            continue;
        }

        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "desktop").unwrap_or(false) {
                    if let Some(app) = parse_desktop_file(&path) {
                        // Use filename as unique key (later entries override earlier ones)
                        let filename = path.file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("")
                            .to_string();

                        if let Some(&idx) = seen.get(&filename) {
                            // Replace existing entry (user's takes precedence)
                            apps[idx] = app;
                        } else {
                            seen.insert(filename, apps.len());
                            apps.push(app);
                        }
                    }
                }
            }
        }
    }

    // Sort by name
    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    debug!("Found {} applications", apps.len());
    apps
}

/// Get applications grouped by category
pub fn get_applications_by_category() -> HashMap<String, Vec<DesktopEntry>> {
    let apps = get_applications();
    let mut by_category: HashMap<String, Vec<DesktopEntry>> = HashMap::new();

    // Standard category mappings
    let category_map: HashMap<&str, &str> = [
        ("AudioVideo", "Multimedia"),
        ("Audio", "Multimedia"),
        ("Video", "Multimedia"),
        ("Development", "Development"),
        ("IDE", "Development"),
        ("Education", "Education"),
        ("Game", "Games"),
        ("Graphics", "Graphics"),
        ("Network", "Internet"),
        ("Office", "Office"),
        ("Science", "Science"),
        ("Settings", "Settings"),
        ("System", "System"),
        ("Utility", "Utilities"),
        ("Accessories", "Utilities"),
    ].into_iter().collect();

    for app in apps {
        let category = app.categories
            .iter()
            .find_map(|c| category_map.get(c.as_str()))
            .copied()
            .unwrap_or("Other")
            .to_string();

        by_category.entry(category).or_default().push(app);
    }

    by_category
}
