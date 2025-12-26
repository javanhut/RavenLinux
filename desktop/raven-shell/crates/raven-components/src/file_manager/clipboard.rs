// Clipboard operations for file manager

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Clipboard operation type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Copy,
    Cut,
}

/// File clipboard manager
#[derive(Debug, Default)]
pub struct Clipboard {
    files: Vec<PathBuf>,
    operation: Option<Operation>,
}

impl Clipboard {
    pub fn new() -> Self {
        Self::default()
    }

    /// Copy files to clipboard
    pub fn copy(&mut self, files: Vec<PathBuf>) {
        self.files = files;
        self.operation = Some(Operation::Copy);
    }

    /// Cut files to clipboard
    pub fn cut(&mut self, files: Vec<PathBuf>) {
        self.files = files;
        self.operation = Some(Operation::Cut);
    }

    /// Check if clipboard has files
    pub fn has_files(&self) -> bool {
        !self.files.is_empty() && self.operation.is_some()
    }

    /// Get clipboard operation type
    pub fn operation(&self) -> Option<Operation> {
        self.operation
    }

    /// Clear clipboard
    pub fn clear(&mut self) {
        self.files.clear();
        self.operation = None;
    }

    /// Paste files to destination directory
    pub fn paste(&mut self, dest: &Path) -> io::Result<()> {
        let op = self.operation.ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "No clipboard operation")
        })?;

        for src in &self.files {
            let file_name = src.file_name().ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "Invalid file path")
            })?;

            let mut dest_path = dest.join(file_name);

            // Handle name conflicts
            let mut counter = 1;
            while dest_path.exists() {
                let stem = src.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("file");
                let ext = src.extension()
                    .and_then(|e| e.to_str())
                    .map(|e| format!(".{}", e))
                    .unwrap_or_default();
                let new_name = format!("{} ({}){}", stem, counter, ext);
                dest_path = dest.join(&new_name);
                counter += 1;
            }

            match op {
                Operation::Copy => {
                    if src.is_dir() {
                        copy_dir_recursive(src, &dest_path)?;
                    } else {
                        fs::copy(src, &dest_path)?;
                    }
                }
                Operation::Cut => {
                    fs::rename(src, &dest_path)?;
                }
            }
        }

        // Clear after cut
        if op == Operation::Cut {
            self.clear();
        }

        Ok(())
    }
}

/// Recursively copy a directory
fn copy_dir_recursive(src: &Path, dest: &Path) -> io::Result<()> {
    fs::create_dir_all(dest)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let entry_path = entry.path();
        let file_name = entry.file_name();
        let dest_path = dest.join(&file_name);

        if entry_path.is_dir() {
            copy_dir_recursive(&entry_path, &dest_path)?;
        } else {
            fs::copy(&entry_path, &dest_path)?;
        }
    }

    Ok(())
}

/// Move files to trash
pub fn trash_files(files: &[PathBuf]) -> io::Result<()> {
    let trash_dir = dirs::home_dir()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Home directory not found"))?
        .join(".local/share/Trash");

    let files_dir = trash_dir.join("files");
    let info_dir = trash_dir.join("info");

    fs::create_dir_all(&files_dir)?;
    fs::create_dir_all(&info_dir)?;

    for file in files {
        let file_name = file.file_name()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Invalid file path"))?;

        // Generate unique name for trash
        let mut trash_name = file_name.to_os_string();
        let mut counter = 1;
        while files_dir.join(&trash_name).exists() {
            let stem = file.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("file");
            let ext = file.extension()
                .and_then(|e| e.to_str())
                .map(|e| format!(".{}", e))
                .unwrap_or_default();
            trash_name = format!("{}.{}{}", stem, counter, ext).into();
            counter += 1;
        }

        let trash_path = files_dir.join(&trash_name);

        // Create .trashinfo file
        let info_file = info_dir.join(format!("{}.trashinfo", trash_name.to_string_lossy()));
        let deletion_date = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S");
        let trash_info = format!(
            "[Trash Info]\nPath={}\nDeletionDate={}",
            file.display(),
            deletion_date
        );
        fs::write(&info_file, trash_info)?;

        // Move file to trash
        fs::rename(file, &trash_path)?;
    }

    Ok(())
}

/// Permanently delete files
pub fn delete_files(files: &[PathBuf]) -> io::Result<()> {
    for file in files {
        if file.is_dir() {
            fs::remove_dir_all(file)?;
        } else {
            fs::remove_file(file)?;
        }
    }
    Ok(())
}
