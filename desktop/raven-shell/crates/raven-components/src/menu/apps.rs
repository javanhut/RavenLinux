use compact_str::CompactString;
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::debug;

/// Application entry from .desktop file
#[derive(Debug, Clone)]
pub struct AppEntry {
    pub name: CompactString,
    pub exec: CompactString,
    pub icon: CompactString,
    pub comment: CompactString,
    pub category: AppCategory,
}

/// Application categories
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AppCategory {
    #[default]
    All,
    System,
    Utilities,
    Development,
    Network,
    Graphics,
    Multimedia,
    Office,
    Other,
}

impl AppCategory {
    pub fn name(&self) -> &'static str {
        match self {
            Self::All => "All",
            Self::System => "System",
            Self::Utilities => "Utilities",
            Self::Development => "Development",
            Self::Network => "Network",
            Self::Graphics => "Graphics",
            Self::Multimedia => "Multimedia",
            Self::Office => "Office",
            Self::Other => "Other",
        }
    }

    pub fn all_categories() -> &'static [AppCategory] {
        &[
            AppCategory::All,
            AppCategory::System,
            AppCategory::Utilities,
            AppCategory::Development,
            AppCategory::Network,
            AppCategory::Graphics,
            AppCategory::Multimedia,
            AppCategory::Office,
            AppCategory::Other,
        ]
    }

    fn from_desktop_categories(categories: &str) -> Self {
        let lower = categories.to_lowercase();

        if lower.contains("system") || lower.contains("settings") {
            AppCategory::System
        } else if lower.contains("utility") || lower.contains("accessories") {
            AppCategory::Utilities
        } else if lower.contains("development") || lower.contains("programming") {
            AppCategory::Development
        } else if lower.contains("network") || lower.contains("internet") {
            AppCategory::Network
        } else if lower.contains("graphics") || lower.contains("image") {
            AppCategory::Graphics
        } else if lower.contains("audio") || lower.contains("video") || lower.contains("multimedia") {
            AppCategory::Multimedia
        } else if lower.contains("office") {
            AppCategory::Office
        } else {
            AppCategory::Other
        }
    }
}

/// Application database
pub struct AppDatabase {
    apps: Vec<AppEntry>,
    by_category: HashMap<AppCategory, Vec<usize>>,
}

impl AppDatabase {
    pub fn new() -> Self {
        let mut db = Self {
            apps: Vec::new(),
            by_category: HashMap::new(),
        };
        db.load_applications();
        db
    }

    fn load_applications(&mut self) {
        let mut seen: HashMap<String, bool> = HashMap::new();

        // Built-in Raven applications
        self.add_builtin_apps(&mut seen);

        // Load from standard .desktop directories
        let desktop_dirs = [
            PathBuf::from("/usr/share/applications"),
            PathBuf::from("/usr/local/share/applications"),
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".local/share/applications"),
        ];

        for dir in &desktop_dirs {
            self.load_from_directory(dir, &mut seen);
        }

        // Sort apps alphabetically
        self.apps.sort_by(|a, b| {
            a.name.to_lowercase().cmp(&b.name.to_lowercase())
        });

        // Build category index
        self.build_category_index();

        debug!("Loaded {} applications", self.apps.len());
    }

    fn add_builtin_apps(&mut self, seen: &mut HashMap<String, bool>) {
        let builtins = [
            ("Raven Terminal", "raven-terminal", "utilities-terminal", "Terminal emulator", AppCategory::System),
            ("Raven Files", "raven-files", "system-file-manager", "File manager", AppCategory::System),
            ("Raven WiFi", "raven-wifi", "network-wireless", "WiFi manager", AppCategory::Network),
            ("Raven Settings", "raven-settings", "preferences-system", "System settings", AppCategory::System),
            ("Raven Installer", "raven-installer", "system-software-install", "System installer", AppCategory::System),
            ("Raven Launcher", "raven-launcher", "system-search", "Application launcher", AppCategory::Utilities),
            ("System Monitor", "raven-terminal -e htop", "utilities-system-monitor", "System resource monitor", AppCategory::System),
            ("Text Editor", "raven-terminal -e nvim", "text-editor", "Text editor", AppCategory::Utilities),
        ];

        for (name, exec, icon, comment, category) in builtins {
            if !seen.contains_key(exec) {
                seen.insert(exec.to_string(), true);
                self.apps.push(AppEntry {
                    name: name.into(),
                    exec: exec.into(),
                    icon: icon.into(),
                    comment: comment.into(),
                    category,
                });
            }
        }
    }

    fn load_from_directory(&mut self, dir: &PathBuf, seen: &mut HashMap<String, bool>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "desktop").unwrap_or(false) {
                if let Some(app) = self.parse_desktop_file(&path) {
                    let key = app.exec.to_string();
                    if !seen.contains_key(&key) {
                        seen.insert(key, true);
                        self.apps.push(app);
                    }
                }
            }
        }
    }

    fn parse_desktop_file(&self, path: &PathBuf) -> Option<AppEntry> {
        let content = std::fs::read_to_string(path).ok()?;

        let mut name = None;
        let mut exec = None;
        let mut icon = None;
        let mut comment = None;
        let mut categories = String::new();
        let mut hidden = false;
        let mut no_display = false;
        let mut app_type = String::new();
        let mut in_desktop_entry = false;

        for line in content.lines() {
            let line = line.trim();

            if line == "[Desktop Entry]" {
                in_desktop_entry = true;
                continue;
            }

            if line.starts_with('[') && line.ends_with(']') {
                in_desktop_entry = false;
                continue;
            }

            if !in_desktop_entry {
                continue;
            }

            if line.starts_with("Name=") && name.is_none() {
                name = Some(line.trim_start_matches("Name=").to_string());
            } else if line.starts_with("Exec=") && exec.is_none() {
                let cmd = line.trim_start_matches("Exec=")
                    .replace("%f", "")
                    .replace("%F", "")
                    .replace("%u", "")
                    .replace("%U", "")
                    .replace("%c", "")
                    .replace("%k", "")
                    .trim()
                    .to_string();
                exec = Some(cmd);
            } else if line.starts_with("Icon=") && icon.is_none() {
                icon = Some(line.trim_start_matches("Icon=").to_string());
            } else if line.starts_with("Comment=") && comment.is_none() {
                comment = Some(line.trim_start_matches("Comment=").to_string());
            } else if line.starts_with("Categories=") {
                categories = line.trim_start_matches("Categories=").to_string();
            } else if line == "Hidden=true" {
                hidden = true;
            } else if line == "NoDisplay=true" {
                no_display = true;
            } else if line.starts_with("Type=") {
                app_type = line.trim_start_matches("Type=").to_string();
            }
        }

        // Filter out hidden, non-applications
        if hidden || no_display {
            return None;
        }

        if app_type != "Application" && !app_type.is_empty() {
            return None;
        }

        let name = name?;
        let exec = exec?;

        if name.is_empty() || exec.is_empty() {
            return None;
        }

        Some(AppEntry {
            name: name.into(),
            exec: exec.into(),
            icon: icon.unwrap_or_else(|| "application-x-executable".to_string()).into(),
            comment: comment.unwrap_or_default().into(),
            category: AppCategory::from_desktop_categories(&categories),
        })
    }

    fn build_category_index(&mut self) {
        self.by_category.clear();

        // Initialize all categories
        for cat in AppCategory::all_categories() {
            self.by_category.insert(*cat, Vec::new());
        }

        // Index apps
        for (idx, app) in self.apps.iter().enumerate() {
            // Add to "All" category
            self.by_category.get_mut(&AppCategory::All).unwrap().push(idx);
            // Add to specific category
            self.by_category.get_mut(&app.category).unwrap().push(idx);
        }
    }

    /// Get all apps
    pub fn all_apps(&self) -> &[AppEntry] {
        &self.apps
    }

    /// Get apps by category (returns cloned entries for use in closures)
    pub fn apps_by_category(&self, category: AppCategory) -> Vec<AppEntry> {
        self.by_category
            .get(&category)
            .map(|indices| indices.iter().map(|&i| self.apps[i].clone()).collect())
            .unwrap_or_default()
    }

    /// Search apps by query (returns cloned entries for use in closures)
    pub fn search(&self, query: &str) -> Vec<AppEntry> {
        let query = query.to_lowercase();

        self.apps
            .iter()
            .filter(|app| {
                app.name.to_lowercase().contains(&query)
                    || app.comment.to_lowercase().contains(&query)
            })
            .cloned()
            .collect()
    }

    /// Get categories that have apps
    pub fn active_categories(&self) -> Vec<AppCategory> {
        AppCategory::all_categories()
            .iter()
            .filter(|cat| {
                **cat == AppCategory::All ||
                self.by_category.get(cat).map(|v| !v.is_empty()).unwrap_or(false)
            })
            .copied()
            .collect()
    }
}

impl Default for AppDatabase {
    fn default() -> Self {
        Self::new()
    }
}
