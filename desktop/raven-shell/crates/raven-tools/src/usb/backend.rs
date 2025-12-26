// USB Creator backend - device detection and ISO writing

use std::fs::{self, File};
use std::io::{Read, Write, BufReader, BufWriter};
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

/// USB device information
#[derive(Debug, Clone)]
pub struct UsbDevice {
    pub path: String,
    pub name: String,
    pub size: u64,
    pub size_human: String,
    pub model: String,
    pub vendor: String,
}

/// Write progress callback
pub type ProgressCallback = Box<dyn Fn(u64, u64) + Send>;

/// USB Creator backend
pub struct UsbManager {
    cancel_flag: Arc<AtomicBool>,
    bytes_written: Arc<AtomicU64>,
}

impl UsbManager {
    pub fn new() -> Self {
        Self {
            cancel_flag: Arc::new(AtomicBool::new(false)),
            bytes_written: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Detect available USB devices
    pub fn detect_devices(&self) -> Vec<UsbDevice> {
        let mut devices = Vec::new();

        let block_dir = Path::new("/sys/block");
        if !block_dir.exists() {
            return devices;
        }

        if let Ok(entries) = fs::read_dir(block_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let name = entry.file_name().to_string_lossy().to_string();

                // Skip non-removable and non-USB devices
                if !name.starts_with("sd") && !name.starts_with("nvme") {
                    continue;
                }

                // Skip if it has partitions we're looking for (we want whole disk)
                if name.len() > 3 && name.chars().last().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                    // This might be a partition, skip for now
                    if name.starts_with("sd") && name.len() > 3 {
                        continue;
                    }
                }

                let device_path = format!("/sys/block/{}", name);

                // Check if removable
                let removable_path = format!("{}/removable", device_path);
                let is_removable = fs::read_to_string(&removable_path)
                    .map(|s| s.trim() == "1")
                    .unwrap_or(false);

                // Also check if it's a USB device via device path
                let device_link = format!("{}/device", device_path);
                let is_usb = if let Ok(link) = fs::read_link(&device_link) {
                    link.to_string_lossy().contains("usb")
                } else {
                    false
                };

                if !is_removable && !is_usb {
                    continue;
                }

                // Get device info
                let size = self.get_device_size(&device_path);
                if size == 0 {
                    continue; // Skip devices with no media
                }

                let model = self.read_sysfs_attr(&device_path, "device/model");
                let vendor = self.read_sysfs_attr(&device_path, "device/vendor");

                let display_name = if !model.is_empty() {
                    format!("{} {}", vendor, model).trim().to_string()
                } else {
                    format!("USB Device ({})", name)
                };

                devices.push(UsbDevice {
                    path: format!("/dev/{}", name),
                    name: name.clone(),
                    size,
                    size_human: Self::format_size(size),
                    model,
                    vendor,
                });
            }
        }

        devices
    }

    fn get_device_size(&self, device_path: &str) -> u64 {
        let size_path = format!("{}/size", device_path);
        fs::read_to_string(&size_path)
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .map(|blocks| blocks * 512) // Convert 512-byte blocks to bytes
            .unwrap_or(0)
    }

    fn read_sysfs_attr(&self, device_path: &str, attr: &str) -> String {
        let path = format!("{}/{}", device_path, attr);
        fs::read_to_string(&path)
            .map(|s| s.trim().to_string())
            .unwrap_or_default()
    }

    fn format_size(bytes: u64) -> String {
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

    /// Check if ISO file is valid
    pub fn validate_iso(&self, path: &str) -> Result<u64, String> {
        let path = Path::new(path);

        if !path.exists() {
            return Err("File does not exist".to_string());
        }

        if !path.is_file() {
            return Err("Path is not a file".to_string());
        }

        // Check extension
        let ext = path.extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();

        if ext != "iso" && ext != "img" {
            return Err("File must be .iso or .img".to_string());
        }

        // Get file size
        let metadata = fs::metadata(path).map_err(|e| e.to_string())?;
        let size = metadata.len();

        if size == 0 {
            return Err("File is empty".to_string());
        }

        Ok(size)
    }

    /// Format device as FAT32
    pub fn format_device(&self, device: &str, label: &str) -> Result<(), String> {
        // Unmount if mounted
        let _ = Command::new("umount").arg(device).output();

        // Create partition table
        let output = Command::new("parted")
            .args(["-s", device, "mklabel", "msdos"])
            .output()
            .map_err(|e| e.to_string())?;

        if !output.status.success() {
            return Err("Failed to create partition table".to_string());
        }

        // Create partition
        let output = Command::new("parted")
            .args(["-s", device, "mkpart", "primary", "fat32", "1MiB", "100%"])
            .output()
            .map_err(|e| e.to_string())?;

        if !output.status.success() {
            return Err("Failed to create partition".to_string());
        }

        // Format partition
        let partition = format!("{}1", device);
        std::thread::sleep(std::time::Duration::from_millis(500)); // Wait for udev

        let label_arg = if label.is_empty() { "RAVEN".to_string() } else { label.to_string() };
        let output = Command::new("mkfs.vfat")
            .args(["-F", "32", "-n", &label_arg, &partition])
            .output()
            .map_err(|e| e.to_string())?;

        if !output.status.success() {
            return Err("Failed to format partition".to_string());
        }

        Ok(())
    }

    /// Write ISO to device
    pub fn write_iso(
        &self,
        iso_path: &str,
        device: &str,
        progress_callback: Option<ProgressCallback>,
    ) -> Result<(), String> {
        self.cancel_flag.store(false, Ordering::SeqCst);
        self.bytes_written.store(0, Ordering::SeqCst);

        // Unmount device first
        let _ = Command::new("umount").arg(device).output();
        let _ = Command::new("umount").arg(format!("{}1", device)).output();

        // Get ISO size
        let iso_size = self.validate_iso(iso_path)?;

        // Open files
        let iso_file = File::open(iso_path).map_err(|e| format!("Failed to open ISO: {}", e))?;
        let device_file = File::options()
            .write(true)
            .open(device)
            .map_err(|e| format!("Failed to open device: {}", e))?;

        let mut reader = BufReader::with_capacity(4 * 1024 * 1024, iso_file); // 4MB buffer
        let mut writer = BufWriter::with_capacity(4 * 1024 * 1024, device_file);

        let mut buffer = vec![0u8; 4 * 1024 * 1024];
        let mut total_written: u64 = 0;

        loop {
            // Check for cancellation
            if self.cancel_flag.load(Ordering::SeqCst) {
                return Err("Operation cancelled".to_string());
            }

            // Read chunk
            let bytes_read = reader.read(&mut buffer).map_err(|e| e.to_string())?;
            if bytes_read == 0 {
                break;
            }

            // Write chunk
            writer.write_all(&buffer[..bytes_read]).map_err(|e| e.to_string())?;

            total_written += bytes_read as u64;
            self.bytes_written.store(total_written, Ordering::SeqCst);

            // Report progress
            if let Some(ref callback) = progress_callback {
                callback(total_written, iso_size);
            }
        }

        // Flush and sync
        writer.flush().map_err(|e| e.to_string())?;
        drop(writer);

        // Sync to ensure all data is written
        Command::new("sync").output().map_err(|e| e.to_string())?;

        Ok(())
    }

    /// Cancel ongoing write operation
    pub fn cancel(&self) {
        self.cancel_flag.store(true, Ordering::SeqCst);
    }

    /// Get current bytes written (for external progress tracking)
    pub fn get_bytes_written(&self) -> u64 {
        self.bytes_written.load(Ordering::SeqCst)
    }

    /// Safely eject device
    pub fn eject_device(&self, device: &str) -> Result<(), String> {
        // Unmount all partitions
        let _ = Command::new("umount").arg(device).output();
        let _ = Command::new("umount").arg(format!("{}1", device)).output();
        let _ = Command::new("umount").arg(format!("{}2", device)).output();

        // Eject
        let output = Command::new("eject").arg(device).output();

        match output {
            Ok(o) if o.status.success() => Ok(()),
            Ok(_) => Err("Eject command failed".to_string()),
            Err(e) => Err(format!("Failed to eject: {}", e)),
        }
    }
}

impl Default for UsbManager {
    fn default() -> Self {
        Self::new()
    }
}
