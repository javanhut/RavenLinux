//! Installation logic

use crate::config::{InstallConfig, InstallProfile};
use crate::disk;
use anyhow::{Context, Result};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

/// Progress callback type
pub type ProgressCallback = Box<dyn Fn(f64, &str) + Send>;

/// Perform the installation
pub async fn install(config: &InstallConfig, progress: ProgressCallback) -> Result<()> {
    let target = "/mnt/raven";

    // Step 1: Partition disk (10%)
    progress(0.0, "Partitioning disk...");
    let layout = disk::create_standard_partitions(&config.disk.device, config.disk.encrypt)?;
    progress(0.10, "Disk partitioned");

    // Step 2: Setup encryption if enabled (20%)
    let root_device = if config.disk.encrypt {
        progress(0.10, "Setting up encryption...");
        let password = config
            .disk
            .encryption_password
            .as_ref()
            .context("Encryption password required")?;
        disk::setup_encryption(&layout.root, password)?
    } else {
        layout.root.clone()
    };
    progress(0.20, "Encryption configured");

    // Step 3: Format partitions (30%)
    progress(0.20, "Formatting partitions...");
    disk::format_partition(&layout.efi, "vfat", "EFI")?;
    disk::format_partition(&root_device, "ext4", "RAVEN_ROOT")?;
    if let Some(swap) = &layout.swap {
        disk::format_partition(swap, "swap", "RAVEN_SWAP")?;
    }
    progress(0.30, "Partitions formatted");

    // Step 4: Mount partitions (35%)
    progress(0.30, "Mounting partitions...");
    std::fs::create_dir_all(target)?;
    disk::mount_partition(&root_device, target)?;
    std::fs::create_dir_all(format!("{}/boot/efi", target))?;
    disk::mount_partition(&layout.efi, &format!("{}/boot/efi", target))?;
    progress(0.35, "Partitions mounted");

    // Step 5: Install base system (60%)
    progress(0.35, "Installing base system...");
    install_base_system(target, &config.profile)?;
    progress(0.60, "Base system installed");

    // Step 6: Configure system (70%)
    progress(0.60, "Configuring system...");
    configure_system(target, config)?;
    progress(0.70, "System configured");

    // Step 7: Install bootloader (80%)
    progress(0.70, "Installing bootloader...");
    install_bootloader(target, &config.disk.device, config.disk.encrypt)?;
    progress(0.80, "Bootloader installed");

    // Step 8: Create user (90%)
    progress(0.80, "Creating user account...");
    create_user(target, &config.user.username, &config.user.password, &config.user.full_name)?;
    progress(0.90, "User account created");

    // Step 9: Cleanup (100%)
    progress(0.90, "Cleaning up...");
    cleanup(target)?;
    progress(1.0, "Installation complete!");

    Ok(())
}

/// Install base system packages
fn install_base_system(target: &str, profile: &InstallProfile) -> Result<()> {
    // Get package list based on profile
    let packages = match profile {
        InstallProfile::Minimal => vec!["base", "linux", "linux-firmware"],
        InstallProfile::Standard => vec![
            "base",
            "linux",
            "linux-firmware",
            "raven-desktop",
            "raven-dev-base",
            "networkmanager",
            "grub",
            "efibootmgr",
        ],
        InstallProfile::Full => vec![
            "base",
            "linux",
            "linux-firmware",
            "raven-desktop",
            "raven-dev-full",
            "networkmanager",
            "grub",
            "efibootmgr",
            "raven-extras",
        ],
    };

    // Copy base system from squashfs (live environment)
    // In a real implementation, this would use rsync or unsquashfs
    Command::new("unsquashfs")
        .args(["-f", "-d", target, "/run/raven/filesystem.squashfs"])
        .output()
        .context("Failed to extract system")?;

    Ok(())
}

/// Configure the installed system
fn configure_system(target: &str, config: &InstallConfig) -> Result<()> {
    // Generate fstab
    let fstab_path = format!("{}/etc/fstab", target);
    generate_fstab(target, &fstab_path)?;

    // Set hostname
    std::fs::write(
        format!("{}/etc/hostname", target),
        &config.system.hostname,
    )?;

    // Set timezone
    let tz_path = format!("{}/etc/localtime", target);
    std::fs::remove_file(&tz_path).ok();
    std::os::unix::fs::symlink(
        format!("/usr/share/zoneinfo/{}", config.system.timezone),
        &tz_path,
    )?;

    // Set locale
    let locale_conf = format!("{}/etc/locale.conf", target);
    std::fs::write(&locale_conf, format!("LANG={}\n", config.system.locale))?;

    // Set keyboard layout
    let vconsole_conf = format!("{}/etc/vconsole.conf", target);
    std::fs::write(&vconsole_conf, format!("KEYMAP={}\n", config.system.keyboard_layout))?;

    // Enable services
    chroot_exec(target, &["systemctl", "enable", "NetworkManager"])?;
    chroot_exec(target, &["systemctl", "enable", "raven-session"])?;

    Ok(())
}

/// Generate fstab
fn generate_fstab(target: &str, output: &str) -> Result<()> {
    let fstab = Command::new("genfstab")
        .args(["-U", target])
        .output()
        .context("Failed to generate fstab")?;

    std::fs::write(output, &fstab.stdout)?;
    Ok(())
}

/// Install bootloader (GRUB)
fn install_bootloader(target: &str, device: &str, encrypted: bool) -> Result<()> {
    // Install GRUB for EFI
    chroot_exec(
        target,
        &[
            "grub-install",
            "--target=x86_64-efi",
            "--efi-directory=/boot/efi",
            "--bootloader-id=RavenLinux",
        ],
    )?;

    // Configure GRUB for encryption if needed
    if encrypted {
        let grub_default = format!("{}/etc/default/grub", target);
        let content = std::fs::read_to_string(&grub_default)?;
        let content = content.replace(
            "GRUB_CMDLINE_LINUX=\"\"",
            "GRUB_CMDLINE_LINUX=\"cryptdevice=UUID=ROOTUUID:raven-root root=/dev/mapper/raven-root\"",
        );
        std::fs::write(&grub_default, content)?;
    }

    // Generate GRUB config
    chroot_exec(target, &["grub-mkconfig", "-o", "/boot/grub/grub.cfg"])?;

    Ok(())
}

/// Create user account
fn create_user(target: &str, username: &str, password: &str, full_name: &str) -> Result<()> {
    // Create user with zsh as default shell
    chroot_exec(
        target,
        &[
            "useradd",
            "-m",
            "-G", "wheel,video,audio",
            "-s", "/bin/zsh",
            "-c", full_name,
            username,
        ],
    )?;

    // Set password
    let password_input = format!("{}:{}", username, password);
    let mut child = Command::new("chroot")
        .args([target, "chpasswd"])
        .stdin(std::process::Stdio::piped())
        .spawn()?;

    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(password_input.as_bytes())?;
    }
    child.wait()?;

    // Enable sudo for wheel group
    let sudoers_dir = format!("{}/etc/sudoers.d", target);
    std::fs::create_dir_all(&sudoers_dir)?;
    let sudoers_path = format!("{}/wheel", sudoers_dir);
    std::fs::write(&sudoers_path, "%wheel ALL=(ALL:ALL) ALL\n")?;
    std::fs::set_permissions(&sudoers_path, std::fs::Permissions::from_mode(0o440))?;

    Ok(())
}

/// Execute command in chroot
fn chroot_exec(target: &str, args: &[&str]) -> Result<()> {
    let mut cmd = Command::new("chroot");
    cmd.arg(target);
    cmd.args(args);

    let output = cmd.output().context("Failed to execute chroot command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Chroot command failed: {}", stderr);
    }

    Ok(())
}

/// Cleanup after installation
fn cleanup(target: &str) -> Result<()> {
    // Unmount in reverse order
    disk::unmount_partition(&format!("{}/boot/efi", target)).ok();
    disk::unmount_partition(target).ok();

    Ok(())
}
