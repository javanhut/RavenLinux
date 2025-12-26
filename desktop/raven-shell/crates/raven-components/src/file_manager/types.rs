// File manager types and utilities

use std::cmp::Ordering;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// A file or directory entry
#[derive(Debug, Clone)]
pub struct FileEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub is_hidden: bool,
    pub is_symlink: bool,
    pub size: u64,
    pub mod_time: Option<SystemTime>,
}

impl FileEntry {
    pub fn from_path(path: &Path) -> Option<Self> {
        let name = path.file_name()?.to_string_lossy().to_string();
        let metadata = fs::symlink_metadata(path).ok()?;
        let is_symlink = metadata.is_symlink();

        // For symlinks, get the target metadata for size
        let real_metadata = if is_symlink {
            fs::metadata(path).ok()
        } else {
            Some(metadata.clone())
        };

        Some(Self {
            name: name.clone(),
            path: path.to_path_buf(),
            is_dir: real_metadata.as_ref().map(|m| m.is_dir()).unwrap_or(false),
            is_hidden: name.starts_with('.'),
            is_symlink,
            size: real_metadata.as_ref().map(|m| m.len()).unwrap_or(0),
            mod_time: real_metadata.and_then(|m| m.modified().ok()),
        })
    }
}

/// Bookmark for sidebar
#[derive(Debug, Clone)]
pub struct Bookmark {
    pub name: String,
    pub path: PathBuf,
    pub icon: String,
}

impl Default for Bookmark {
    fn default() -> Self {
        Self {
            name: String::new(),
            path: PathBuf::new(),
            icon: "folder-symbolic".to_string(),
        }
    }
}

/// Get default bookmarks
pub fn get_default_bookmarks() -> Vec<Bookmark> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));

    vec![
        Bookmark {
            name: "Home".to_string(),
            path: home.clone(),
            icon: "user-home-symbolic".to_string(),
        },
        Bookmark {
            name: "Desktop".to_string(),
            path: home.join("Desktop"),
            icon: "user-desktop-symbolic".to_string(),
        },
        Bookmark {
            name: "Documents".to_string(),
            path: home.join("Documents"),
            icon: "folder-documents-symbolic".to_string(),
        },
        Bookmark {
            name: "Downloads".to_string(),
            path: home.join("Downloads"),
            icon: "folder-download-symbolic".to_string(),
        },
        Bookmark {
            name: "Pictures".to_string(),
            path: home.join("Pictures"),
            icon: "folder-pictures-symbolic".to_string(),
        },
        Bookmark {
            name: "Videos".to_string(),
            path: home.join("Videos"),
            icon: "folder-videos-symbolic".to_string(),
        },
        Bookmark {
            name: "Music".to_string(),
            path: home.join("Music"),
            icon: "folder-music-symbolic".to_string(),
        },
        Bookmark {
            name: "Trash".to_string(),
            path: home.join(".local/share/Trash/files"),
            icon: "user-trash-symbolic".to_string(),
        },
    ]
}

/// Read directory contents
pub fn read_directory(path: &Path, show_hidden: bool) -> Vec<FileEntry> {
    let mut entries = Vec::new();

    if let Ok(read_dir) = fs::read_dir(path) {
        for entry in read_dir.filter_map(|e| e.ok()) {
            let entry_path = entry.path();
            if let Some(file_entry) = FileEntry::from_path(&entry_path) {
                if show_hidden || !file_entry.is_hidden {
                    entries.push(file_entry);
                }
            }
        }
    }

    // Sort: directories first, then alphabetically
    entries.sort_by(|a, b| {
        match (a.is_dir, b.is_dir) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        }
    });

    entries
}

/// Get parent path
pub fn get_parent_path(path: &Path) -> PathBuf {
    path.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from("/"))
}

/// Get icon name for file type
pub fn get_file_icon(entry: &FileEntry) -> &'static str {
    if entry.is_dir {
        return "folder-symbolic";
    }

    let ext = entry.path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());

    match ext.as_deref() {
        // Images
        Some("png" | "jpg" | "jpeg" | "gif" | "bmp" | "svg" | "webp" | "ico") => "image-x-generic-symbolic",
        // Video
        Some("mp4" | "mkv" | "avi" | "mov" | "webm" | "flv" | "wmv") => "video-x-generic-symbolic",
        // Audio
        Some("mp3" | "flac" | "wav" | "ogg" | "m4a" | "aac" | "wma") => "audio-x-generic-symbolic",
        // Documents
        Some("pdf") => "x-office-document-symbolic",
        Some("doc" | "docx" | "odt" | "rtf") => "x-office-document-symbolic",
        Some("xls" | "xlsx" | "ods" | "csv") => "x-office-spreadsheet-symbolic",
        Some("ppt" | "pptx" | "odp") => "x-office-presentation-symbolic",
        Some("txt" | "md" | "rst") => "text-x-generic-symbolic",
        // Code
        Some("rs" | "go" | "py" | "js" | "ts" | "c" | "cpp" | "h" | "hpp" | "java" | "rb" | "php") => "text-x-script-symbolic",
        Some("html" | "htm" | "css" | "scss" | "sass" | "less") => "text-html-symbolic",
        Some("json" | "yaml" | "yml" | "toml" | "xml" | "ini" | "conf" | "cfg") => "text-x-generic-symbolic",
        Some("sh" | "bash" | "zsh" | "fish") => "application-x-executable-symbolic",
        // Archives
        Some("zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar" | "zst") => "package-x-generic-symbolic",
        // Packages
        Some("deb" | "rpm" | "pkg" | "appimage" | "flatpak" | "snap") => "package-x-generic-symbolic",
        // Disk images
        Some("iso" | "img" | "dmg") => "media-optical-symbolic",
        // Executables
        Some("exe" | "msi" | "bin" | "run") => "application-x-executable-symbolic",
        // Default
        _ => "text-x-generic-symbolic",
    }
}

/// Format file size as human readable
pub fn humanize_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes >= TB {
        format!("{:.1} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Format timestamp for display
pub fn format_date(time: Option<SystemTime>) -> String {
    time.and_then(|t| {
        t.duration_since(SystemTime::UNIX_EPOCH)
            .ok()
            .map(|d| {
                let secs = d.as_secs() as i64;
                let dt = chrono::DateTime::from_timestamp(secs, 0)?;
                Some(dt.format("%Y-%m-%d %H:%M").to_string())
            })
    })
    .flatten()
    .unwrap_or_else(|| "—".to_string())
}

/// Pluralize a count
pub fn pluralize(count: usize, singular: &str, plural: &str) -> String {
    if count == 1 {
        format!("{} {}", count, singular)
    } else {
        format!("{} {}", count, plural)
    }
}

/// Get disk space info for path
pub fn get_disk_space(path: &Path) -> (u64, u64) {
    // Try to get disk space using statvfs
    #[cfg(unix)]
    {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;

        let path_cstr = CString::new(path.as_os_str().as_bytes()).ok();
        if let Some(cstr) = path_cstr {
            unsafe {
                let mut stat: libc::statvfs = std::mem::zeroed();
                if libc::statvfs(cstr.as_ptr(), &mut stat) == 0 {
                    let free = stat.f_bavail as u64 * stat.f_bsize as u64;
                    let total = stat.f_blocks as u64 * stat.f_bsize as u64;
                    return (free, total);
                }
            }
        }
    }
    (0, 0)
}
