// System Installer backend - disk operations and system installation

use std::fs;
use std::path::Path;
use std::process::Command;

/// Disk information
#[derive(Debug, Clone)]
pub struct Disk {
    pub path: String,
    pub name: String,
    pub size: u64,
    pub size_human: String,
    pub model: String,
    pub partitions: Vec<Partition>,
}

/// Partition information
#[derive(Debug, Clone)]
pub struct Partition {
    pub path: String,
    pub size: u64,
    pub size_human: String,
    pub filesystem: String,
    pub mountpoint: Option<String>,
}

/// Installation configuration
#[derive(Debug, Clone, Default)]
pub struct InstallConfig {
    pub target_disk: String,
    pub hostname: String,
    pub username: String,
    pub password: String,
    pub timezone: String,
    pub locale: String,
    pub keyboard_layout: String,
    pub auto_partition: bool,
    pub efi_partition: Option<String>,
    pub root_partition: Option<String>,
}

/// Installation progress callback
pub type ProgressCallback = Box<dyn Fn(&str, f64) + Send>;

/// System Installer backend
pub struct InstallerManager {
    source_path: String,
    target_mount: String,
}

impl InstallerManager {
    pub fn new() -> Self {
        Self {
            source_path: "/run/archiso/airootfs".to_string(),
            target_mount: "/mnt".to_string(),
        }
    }

    /// Detect available disks
    pub fn detect_disks(&self) -> Vec<Disk> {
        let mut disks = Vec::new();

        let block_dir = Path::new("/sys/block");
        if !block_dir.exists() {
            return disks;
        }

        if let Ok(entries) = fs::read_dir(block_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let name = entry.file_name().to_string_lossy().to_string();

                // Only include sd*, nvme*, and vd* devices
                if !name.starts_with("sd") && !name.starts_with("nvme") && !name.starts_with("vd") {
                    continue;
                }

                // Skip partitions
                if name.starts_with("sd") && name.len() > 3 && name.chars().last().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                    continue;
                }

                let device_path = format!("/sys/block/{}", name);

                // Skip removable devices (USB)
                let removable_path = format!("{}/removable", device_path);
                let is_removable = fs::read_to_string(&removable_path)
                    .map(|s| s.trim() == "1")
                    .unwrap_or(false);

                if is_removable {
                    continue;
                }

                // Get size
                let size = self.get_device_size(&device_path);
                if size == 0 {
                    continue;
                }

                // Skip small disks (< 8GB)
                if size < 8 * 1024 * 1024 * 1024 {
                    continue;
                }

                let model = self.read_sysfs_attr(&device_path, "device/model");
                let partitions = self.detect_partitions(&name);

                disks.push(Disk {
                    path: format!("/dev/{}", name),
                    name: name.clone(),
                    size,
                    size_human: Self::format_size(size),
                    model,
                    partitions,
                });
            }
        }

        disks
    }

    fn detect_partitions(&self, disk_name: &str) -> Vec<Partition> {
        let mut partitions = Vec::new();

        // Use lsblk for partition info
        let output = Command::new("lsblk")
            .args(["-o", "NAME,SIZE,FSTYPE,MOUNTPOINT", "-P", "-b", &format!("/dev/{}", disk_name)])
            .output();

        if let Ok(output) = output {
            let output_str = String::from_utf8_lossy(&output.stdout);
            for line in output_str.lines().skip(1) {
                // Skip the disk itself
                let mut name = String::new();
                let mut size = 0u64;
                let mut fstype = String::new();
                let mut mountpoint = None;

                for part in line.split(' ') {
                    if let Some((key, value)) = part.split_once('=') {
                        let value = value.trim_matches('"');
                        match key {
                            "NAME" => name = value.to_string(),
                            "SIZE" => size = value.parse().unwrap_or(0),
                            "FSTYPE" => fstype = value.to_string(),
                            "MOUNTPOINT" => {
                                if !value.is_empty() {
                                    mountpoint = Some(value.to_string());
                                }
                            }
                            _ => {}
                        }
                    }
                }

                if !name.is_empty() && name != disk_name {
                    partitions.push(Partition {
                        path: format!("/dev/{}", name),
                        size,
                        size_human: Self::format_size(size),
                        filesystem: fstype,
                        mountpoint,
                    });
                }
            }
        }

        partitions
    }

    fn get_device_size(&self, device_path: &str) -> u64 {
        let size_path = format!("{}/size", device_path);
        fs::read_to_string(&size_path)
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .map(|blocks| blocks * 512)
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

    /// Get available timezones
    pub fn get_timezones(&self) -> Vec<String> {
        let mut timezones = Vec::new();
        let tz_dir = Path::new("/usr/share/zoneinfo");

        if let Ok(regions) = fs::read_dir(tz_dir) {
            for region in regions.filter_map(|e| e.ok()) {
                let region_name = region.file_name().to_string_lossy().to_string();

                // Skip non-region directories
                if region_name.starts_with('.') || region_name == "posix" || region_name == "right" {
                    continue;
                }

                let region_path = region.path();
                if region_path.is_dir() {
                    if let Ok(cities) = fs::read_dir(&region_path) {
                        for city in cities.filter_map(|e| e.ok()) {
                            let city_name = city.file_name().to_string_lossy().to_string();
                            if !city_name.starts_with('.') && city.path().is_file() {
                                timezones.push(format!("{}/{}", region_name, city_name));
                            }
                        }
                    }
                } else if region_path.is_file() {
                    timezones.push(region_name);
                }
            }
        }

        timezones.sort();
        timezones
    }

    /// Get available keyboard layouts
    pub fn get_keyboard_layouts(&self) -> Vec<String> {
        // Try to get from system first
        let output = Command::new("localectl")
            .args(["list-keymaps"])
            .output();

        if let Ok(output) = output {
            if output.status.success() {
                return String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .map(|s| s.to_string())
                    .collect();
            }
        }

        // Fallback to common layouts
        vec![
            "us", "gb", "de", "fr", "es", "it", "pt", "br", "ru", "jp", "kr", "cn",
            "dvorak", "colemak", "workman",
        ].into_iter().map(|s| s.to_string()).collect()
    }

    /// Partition disk automatically (GPT with EFI + root)
    pub fn auto_partition(&self, disk: &str) -> Result<(String, String), String> {
        // Unmount any existing partitions
        self.unmount_disk(disk)?;

        // Create GPT partition table
        self.run_command("parted", &["-s", disk, "mklabel", "gpt"])?;

        // Create EFI partition (512MB)
        self.run_command("parted", &["-s", disk, "mkpart", "EFI", "fat32", "1MiB", "513MiB"])?;
        self.run_command("parted", &["-s", disk, "set", "1", "esp", "on"])?;

        // Create root partition (rest of disk)
        self.run_command("parted", &["-s", disk, "mkpart", "root", "ext4", "513MiB", "100%"])?;

        // Wait for udev
        std::thread::sleep(std::time::Duration::from_secs(1));

        // Determine partition names
        let (efi_part, root_part) = if disk.contains("nvme") || disk.contains("mmcblk") {
            (format!("{}p1", disk), format!("{}p2", disk))
        } else {
            (format!("{}1", disk), format!("{}2", disk))
        };

        // Format partitions
        self.run_command("mkfs.fat", &["-F", "32", &efi_part])?;
        self.run_command("mkfs.ext4", &["-F", &root_part])?;

        Ok((efi_part, root_part))
    }

    fn unmount_disk(&self, disk: &str) -> Result<(), String> {
        // Unmount all partitions
        for i in 1..=10 {
            let part = if disk.contains("nvme") || disk.contains("mmcblk") {
                format!("{}p{}", disk, i)
            } else {
                format!("{}{}", disk, i)
            };
            let _ = Command::new("umount").arg(&part).output();
        }
        Ok(())
    }

    /// Mount partitions for installation
    pub fn mount_partitions(&self, efi_part: &str, root_part: &str) -> Result<(), String> {
        // Mount root
        fs::create_dir_all(&self.target_mount).map_err(|e| e.to_string())?;
        self.run_command("mount", &[root_part, &self.target_mount])?;

        // Create and mount EFI
        let efi_mount = format!("{}/boot/efi", self.target_mount);
        fs::create_dir_all(&efi_mount).map_err(|e| e.to_string())?;
        self.run_command("mount", &[efi_part, &efi_mount])?;

        Ok(())
    }

    /// Install system files
    pub fn install_system(&self, progress: Option<ProgressCallback>) -> Result<(), String> {
        if let Some(ref cb) = progress {
            cb("Copying system files...", 0.1);
        }

        // Check source exists
        if !Path::new(&self.source_path).exists() {
            return Err("Source filesystem not found".to_string());
        }

        // Use rsync for copying
        let output = Command::new("rsync")
            .args([
                "-aAXH",
                "--info=progress2",
                "--exclude=/dev/*",
                "--exclude=/proc/*",
                "--exclude=/sys/*",
                "--exclude=/tmp/*",
                "--exclude=/run/*",
                "--exclude=/mnt/*",
                "--exclude=/media/*",
                "--exclude=/lost+found",
                &format!("{}/", self.source_path),
                &format!("{}/", self.target_mount),
            ])
            .output()
            .map_err(|e| e.to_string())?;

        if !output.status.success() {
            return Err("Failed to copy system files".to_string());
        }

        if let Some(ref cb) = progress {
            cb("System files copied", 0.5);
        }

        Ok(())
    }

    /// Configure the installed system
    pub fn configure_system(&self, config: &InstallConfig, progress: Option<ProgressCallback>) -> Result<(), String> {
        if let Some(ref cb) = progress {
            cb("Configuring system...", 0.6);
        }

        // Set hostname
        let hostname_path = format!("{}/etc/hostname", self.target_mount);
        fs::write(&hostname_path, &config.hostname).map_err(|e| e.to_string())?;

        // Set hosts
        let hosts_path = format!("{}/etc/hosts", self.target_mount);
        let hosts_content = format!(
            "127.0.0.1\tlocalhost\n::1\t\tlocalhost\n127.0.1.1\t{}.localdomain\t{}\n",
            config.hostname, config.hostname
        );
        fs::write(&hosts_path, hosts_content).map_err(|e| e.to_string())?;

        // Set timezone
        if !config.timezone.is_empty() {
            let tz_target = format!("{}/etc/localtime", self.target_mount);
            let tz_source = format!("/usr/share/zoneinfo/{}", config.timezone);
            let _ = fs::remove_file(&tz_target);
            std::os::unix::fs::symlink(&tz_source, &tz_target).map_err(|e| e.to_string())?;
        }

        // Set locale
        if !config.locale.is_empty() {
            let locale_conf = format!("{}/etc/locale.conf", self.target_mount);
            fs::write(&locale_conf, format!("LANG={}\n", config.locale)).map_err(|e| e.to_string())?;
        }

        // Set keyboard layout
        if !config.keyboard_layout.is_empty() {
            let vconsole = format!("{}/etc/vconsole.conf", self.target_mount);
            fs::write(&vconsole, format!("KEYMAP={}\n", config.keyboard_layout)).map_err(|e| e.to_string())?;
        }

        if let Some(ref cb) = progress {
            cb("System configured", 0.7);
        }

        Ok(())
    }

    /// Create user account
    pub fn create_user(&self, config: &InstallConfig, progress: Option<ProgressCallback>) -> Result<(), String> {
        if let Some(ref cb) = progress {
            cb("Creating user account...", 0.75);
        }

        if config.username.is_empty() {
            return Ok(());
        }

        // Create user with chroot
        self.run_chroot_command(&[
            "useradd", "-m", "-G", "wheel,audio,video,storage,optical",
            "-s", "/bin/bash", &config.username
        ])?;

        // Set password
        let passwd_cmd = format!("echo '{}:{}' | chpasswd", config.username, config.password);
        self.run_chroot_command(&["sh", "-c", &passwd_cmd])?;

        // Enable sudo for wheel group
        let sudoers = format!("{}/etc/sudoers.d/wheel", self.target_mount);
        fs::write(&sudoers, "%wheel ALL=(ALL:ALL) ALL\n").map_err(|e| e.to_string())?;

        if let Some(ref cb) = progress {
            cb("User created", 0.8);
        }

        Ok(())
    }

    /// Generate fstab
    pub fn generate_fstab(&self, efi_part: &str, root_part: &str) -> Result<(), String> {
        let fstab_path = format!("{}/etc/fstab", self.target_mount);

        // Get UUIDs
        let root_uuid = self.get_uuid(root_part)?;
        let efi_uuid = self.get_uuid(efi_part)?;

        let fstab = format!(
            "# /etc/fstab: static file system information\n\
             # <file system> <mount point> <type> <options> <dump> <pass>\n\n\
             UUID={}\t/\text4\tdefaults,noatime\t0\t1\n\
             UUID={}\t/boot/efi\tvfat\tumask=0077\t0\t2\n",
            root_uuid, efi_uuid
        );

        fs::write(&fstab_path, fstab).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn get_uuid(&self, partition: &str) -> Result<String, String> {
        let output = Command::new("blkid")
            .args(["-s", "UUID", "-o", "value", partition])
            .output()
            .map_err(|e| e.to_string())?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            Err("Failed to get UUID".to_string())
        }
    }

    /// Install bootloader (GRUB)
    pub fn install_bootloader(&self, disk: &str, progress: Option<ProgressCallback>) -> Result<(), String> {
        if let Some(ref cb) = progress {
            cb("Installing bootloader...", 0.85);
        }

        // Install GRUB for UEFI
        self.run_chroot_command(&[
            "grub-install",
            "--target=x86_64-efi",
            "--efi-directory=/boot/efi",
            "--bootloader-id=RavenLinux",
            "--recheck",
        ])?;

        // Generate GRUB config
        self.run_chroot_command(&["grub-mkconfig", "-o", "/boot/grub/grub.cfg"])?;

        if let Some(ref cb) = progress {
            cb("Bootloader installed", 0.9);
        }

        Ok(())
    }

    /// Finalize installation
    pub fn finalize(&self, progress: Option<ProgressCallback>) -> Result<(), String> {
        if let Some(ref cb) = progress {
            cb("Finalizing installation...", 0.95);
        }

        // Enable essential services
        let services = ["NetworkManager", "sddm"];
        for service in services {
            let _ = self.run_chroot_command(&["systemctl", "enable", service]);
        }

        // Sync and unmount
        let _ = Command::new("sync").output();

        let efi_mount = format!("{}/boot/efi", self.target_mount);
        let _ = Command::new("umount").arg(&efi_mount).output();
        let _ = Command::new("umount").arg(&self.target_mount).output();

        if let Some(ref cb) = progress {
            cb("Installation complete!", 1.0);
        }

        Ok(())
    }

    fn run_command(&self, cmd: &str, args: &[&str]) -> Result<(), String> {
        let output = Command::new(cmd)
            .args(args)
            .output()
            .map_err(|e| format!("Failed to run {}: {}", cmd, e))?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!("{} failed: {}", cmd, stderr))
        }
    }

    fn run_chroot_command(&self, args: &[&str]) -> Result<(), String> {
        let output = Command::new("arch-chroot")
            .arg(&self.target_mount)
            .args(args)
            .output()
            .map_err(|e| format!("Failed to run arch-chroot: {}", e))?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!("chroot command failed: {}", stderr))
        }
    }

    /// Run complete installation
    pub fn install(&self, config: &InstallConfig, progress: Option<ProgressCallback>) -> Result<(), String> {
        // Auto partition if requested
        let (efi_part, root_part) = if config.auto_partition {
            self.auto_partition(&config.target_disk)?
        } else {
            (
                config.efi_partition.clone().ok_or("No EFI partition specified")?,
                config.root_partition.clone().ok_or("No root partition specified")?,
            )
        };

        // Mount partitions
        self.mount_partitions(&efi_part, &root_part)?;

        // Install system
        self.install_system(None)?;

        // Generate fstab
        self.generate_fstab(&efi_part, &root_part)?;

        // Configure system
        self.configure_system(config, None)?;

        // Create user
        self.create_user(config, None)?;

        // Install bootloader
        self.install_bootloader(&config.target_disk, None)?;

        // Finalize
        self.finalize(None)?;

        Ok(())
    }
}

impl Default for InstallerManager {
    fn default() -> Self {
        Self::new()
    }
}
