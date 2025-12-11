//! Disk management utilities

use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

/// Information about a disk
#[derive(Debug, Clone)]
pub struct DiskInfo {
    pub path: String,
    pub model: String,
    pub size_bytes: u64,
    pub size_human: String,
    pub removable: bool,
}

/// List available disks
pub fn list_disks() -> Result<Vec<DiskInfo>> {
    let output = Command::new("lsblk")
        .args(["-d", "-n", "-o", "NAME,SIZE,MODEL,RM", "-b"])
        .output()
        .context("Failed to run lsblk")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut disks = Vec::new();

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 3 {
            let name = parts[0];
            // Skip loop devices and other non-disk devices
            if name.starts_with("loop") || name.starts_with("sr") {
                continue;
            }

            let size_bytes: u64 = parts[1].parse().unwrap_or(0);
            let model = parts.get(2..).map(|p| p.join(" ")).unwrap_or_default();
            let removable = parts.last().map(|&s| s == "1").unwrap_or(false);

            disks.push(DiskInfo {
                path: format!("/dev/{}", name),
                model,
                size_bytes,
                size_human: human_readable_size(size_bytes),
                removable,
            });
        }
    }

    Ok(disks)
}

/// Format size in human-readable format
fn human_readable_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    format!("{:.1} {}", size, UNITS[unit_index])
}

/// Create partition table (GPT)
pub fn create_gpt_table(device: &str) -> Result<()> {
    Command::new("sgdisk")
        .args(["-Z", device]) // Zap existing partitions
        .output()
        .context("Failed to zap partition table")?;

    Command::new("sgdisk")
        .args(["-o", device]) // Create new GPT table
        .output()
        .context("Failed to create GPT table")?;

    Ok(())
}

/// Create partitions for a standard installation
pub fn create_standard_partitions(device: &str, encrypt: bool) -> Result<PartitionLayout> {
    // EFI partition (512 MB)
    Command::new("sgdisk")
        .args(["-n", "1:0:+512M", "-t", "1:EF00", "-c", "1:EFI", device])
        .output()
        .context("Failed to create EFI partition")?;

    if encrypt {
        // Single root partition for encryption
        Command::new("sgdisk")
            .args(["-n", "2:0:0", "-t", "2:8309", "-c", "2:root", device])
            .output()
            .context("Failed to create root partition")?;

        Ok(PartitionLayout {
            efi: format!("{}1", device),
            root: format!("{}2", device),
            home: None,
            swap: None,
        })
    } else {
        // Swap partition (RAM size, max 8GB)
        Command::new("sgdisk")
            .args(["-n", "2:0:+8G", "-t", "2:8200", "-c", "2:swap", device])
            .output()
            .context("Failed to create swap partition")?;

        // Root partition (rest of disk)
        Command::new("sgdisk")
            .args(["-n", "3:0:0", "-t", "3:8300", "-c", "3:root", device])
            .output()
            .context("Failed to create root partition")?;

        Ok(PartitionLayout {
            efi: format!("{}1", device),
            swap: Some(format!("{}2", device)),
            root: format!("{}3", device),
            home: None,
        })
    }
}

/// Partition layout after partitioning
#[derive(Debug, Clone)]
pub struct PartitionLayout {
    pub efi: String,
    pub root: String,
    pub home: Option<String>,
    pub swap: Option<String>,
}

/// Format a partition
pub fn format_partition(device: &str, filesystem: &str, label: &str) -> Result<()> {
    match filesystem {
        "vfat" | "fat32" => {
            Command::new("mkfs.vfat")
                .args(["-F", "32", "-n", label, device])
                .output()
                .context("Failed to format FAT32 partition")?;
        }
        "ext4" => {
            Command::new("mkfs.ext4")
                .args(["-L", label, "-F", device])
                .output()
                .context("Failed to format ext4 partition")?;
        }
        "btrfs" => {
            Command::new("mkfs.btrfs")
                .args(["-L", label, "-f", device])
                .output()
                .context("Failed to format btrfs partition")?;
        }
        "xfs" => {
            Command::new("mkfs.xfs")
                .args(["-L", label, "-f", device])
                .output()
                .context("Failed to format XFS partition")?;
        }
        "swap" => {
            Command::new("mkswap")
                .args(["-L", label, device])
                .output()
                .context("Failed to create swap")?;
        }
        _ => anyhow::bail!("Unknown filesystem: {}", filesystem),
    }

    Ok(())
}

/// Setup LUKS encryption
pub fn setup_encryption(device: &str, password: &str) -> Result<String> {
    // Format with LUKS
    let mut child = Command::new("cryptsetup")
        .args([
            "luksFormat",
            "--type", "luks2",
            "--cipher", "aes-xts-plain64",
            "--key-size", "512",
            "--hash", "sha512",
            "--iter-time", "2000",
            device,
        ])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .context("Failed to start cryptsetup")?;

    if let Some(stdin) = child.stdin.as_mut() {
        use std::io::Write;
        writeln!(stdin, "{}", password)?;
    }

    child.wait().context("cryptsetup failed")?;

    // Open the encrypted partition
    let mapper_name = "raven-root";
    let mut child = Command::new("cryptsetup")
        .args(["open", device, mapper_name])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .context("Failed to open encrypted partition")?;

    if let Some(stdin) = child.stdin.as_mut() {
        use std::io::Write;
        writeln!(stdin, "{}", password)?;
    }

    child.wait().context("Failed to open LUKS container")?;

    Ok(format!("/dev/mapper/{}", mapper_name))
}

/// Mount a partition
pub fn mount_partition(device: &str, mount_point: &str) -> Result<()> {
    std::fs::create_dir_all(mount_point)?;

    Command::new("mount")
        .args([device, mount_point])
        .output()
        .context("Failed to mount partition")?;

    Ok(())
}

/// Unmount a partition
pub fn unmount_partition(mount_point: &str) -> Result<()> {
    Command::new("umount")
        .arg(mount_point)
        .output()
        .context("Failed to unmount partition")?;

    Ok(())
}
