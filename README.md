# RavenLinux

A developer-focused Linux distribution built from scratch with custom tooling.

```
  _____                         _      _
 |  __ \                       | |    (_)
 | |__) |__ ___   _____ _ __   | |     _ _ __  _   ___  __
 |  _  // _` \ \ / / _ \ '_ \  | |    | | '_ \| | | \ \/ /
 | | \ \ (_| |\ V /  __/ | | | | |____| | | | | |_| |>  <
 |_|  \_\__,_| \_/ \___|_| |_| |______|_|_| |_|\__,_/_/\_\
```

## Overview

RavenLinux is an independent Linux distribution designed for developers and application development. It features:

- **Custom Package Manager** (`rvn`) - Fast, Rust-based package management with developer workspaces
- **Custom Bootloader** (RavenBoot) - Multi-boot UEFI bootloader with Linux EFI stub support
- **Native Tools Integration** - First-class support for Vem editor, Carrion language, and Ivaldi VCS
- **Minimal Base** - Built with musl libc for a clean, lightweight system
- **Rolling Release** - Always up-to-date packages

## Downloads

Download the latest release from [GitHub Releases](../../releases):

| File | Description |
|------|-------------|
| `ravenlinux-vX.X.X.iso` | Bootable ISO image |
| `raven-usb-vX.X.X-linux-x86_64` | USB flasher tool |
| `ravenlinux-vX.X.X-source.tar.gz` | Complete source code |
| `ravenlinux-tools-vX.X.X-linux-x86_64.tar.gz` | All tools bundled |

### Quick Install

```bash
# Download the ISO and raven-usb tool, then flash to USB
chmod +x raven-usb-*-linux-x86_64
sudo ./raven-usb-*-linux-x86_64 ravenlinux-*.iso /dev/sdX

# Boot from USB and follow the installer
```

## Technology Stack

| Component | Language | Description |
|-----------|----------|-------------|
| Kernel | C | Linux kernel with EFI stub |
| Bootloader | Rust | UEFI multi-boot loader (RavenBoot) |
| Init System | Rust | Custom init and service manager |
| Package Manager | Rust | System package management (rvn) |
| Text Editor | Go | Vem - Modal editor with syntax highlighting |
| Version Control | Go | Ivaldi - Distributed VCS |
| Programming Language | Go | Carrion - Custom programming language |
| WiFi Manager | Go | TUI/GUI tools to connect to WiFi networks |
| Installer | Go | GUI system installer |
| USB Flasher | Go | Tool to create bootable USB drives |

## Native Tools

RavenLinux includes these custom development tools:

### Vem (Text Editor)
Modal text editor with syntax highlighting and modern editing features.
```bash
vem myfile.txt
```

### Carrion (Programming Language)
Custom programming language designed for system scripting and application development.
```bash
carrion run script.crn
```

### Ivaldi (Version Control)
Distributed version control system with a focus on simplicity.
```bash
ivaldi init
ivaldi commit -m "Initial commit"
```

## Networking

### WiFi Setup
Connecting to WiFi is simple. Just run:
```bash
sudo wifi
```

This opens an interactive terminal interface where you can:
- See all available networks with signal strength
- Use arrow keys to select a network
- Enter password when prompted
- Connection is automatically saved for next time

That's it! No complicated commands to remember.

Note: RavenLinux ships `rtw89` firmware and sets `options rtw89_pci disable_aspm=1` by default to ensure RTL8852BE cards reliably create a `wlan*` interface at boot.

### Alternative: GUI WiFi Manager
If you prefer a graphical interface:
```bash
raven-wifi
```

### Advanced: Manual WiFi (CLI)
For power users who prefer raw commands:
```bash
# Using iwd
iwctl station wlan0 scan
iwctl station wlan0 get-networks
iwctl station wlan0 connect "NetworkName"

# Using wpa_supplicant (fallback)
wpa_passphrase "NetworkName" "password" > /etc/wpa_supplicant.conf
wpa_supplicant -B -i wlan0 -c /etc/wpa_supplicant.conf
dhcpcd wlan0
```

## Building from Source

### Requirements

- Linux system (tested on Arch Linux)
- 20GB+ disk space
- 8GB+ RAM recommended
- Rust toolchain
- Go toolchain (1.23+)
- Build tools: gcc, make, cmake, meson

### Build Steps

```bash
# Clone the repository
git clone https://github.com/javanhut/RavenLinux.git
cd RavenLinux

# Build everything and generate ISO
./scripts/build.sh all

# Or build individual stages:
./scripts/build.sh stage1    # Download toolchain and build base
./scripts/build.sh stage2    # Build native sysroot
./scripts/build.sh stage3    # Build packages
./scripts/build.sh stage4    # Generate ISO
```

### Development Environment

```bash
# Set up development environment
./scripts/dev-env.sh setup

# Mount overlay filesystem for editing
./scripts/dev-env.sh mount

# Enter chroot to test
./scripts/dev-env.sh chroot

# Boot in QEMU with graphics
./scripts/dev-env.sh qemu -g

# Boot in QEMU headless (no new window, serial console in this terminal)
# Tip: In the RavenBoot menu select "Raven Linux (Serial Console)" for best logs/debugging.
./scripts/dev-env.sh qemu

# Console access:
# - tty1: starts `/bin/raven-shell` via `agetty --skip-login` (PAM-free rescue shell)
# - tty2: starts a normal `login` prompt (use this to test PAM/password logins)

# Check status
./scripts/dev-env.sh status
```

## Project Structure

```
RavenLinux/
├── bootloader/          # RavenBoot UEFI bootloader (Rust)
├── init/                # Init system and service manager (Rust)
├── tools/
│   ├── rvn/             # Package manager (Rust)
│   ├── raven-installer/ # GUI system installer (Go)
│   └── raven-usb/       # USB flasher tool (Go)
├── configs/             # System configurations
│   └── kernel/          # Kernel configs
├── scripts/             # Build and utility scripts
│   ├── build.sh         # Main build script
│   ├── dev-env.sh       # Development environment
│   ├── build-initramfs.sh
│   └── stages/          # Build stage scripts
├── etc/                 # Base system configuration
└── .github/workflows/   # CI/CD pipelines
```

## Package Manager (rvn)

```bash
# Install packages
rvn install rust nodejs python

# Search packages
rvn search editor

# Sync repository metadata (required for fast/offline search)
rvn sync

# Generate an index.json for a repo directory (expects ./packages/*.rvn)
rvn repo index /path/to/raven_linux_v0.1.0

# Developer tools
rvn dev rust           # Set up Rust toolchain
rvn dev node 20        # Set up Node.js 20

# Workspaces (isolated dev environments)
rvn workspace create myproject --lang rust,python
rvn workspace enter myproject

# System management
rvn system snapshot    # Create system snapshot
rvn system rollback    # Rollback to snapshot
rvn system health      # Check system health
```

## Bootloader (RavenBoot)

The custom UEFI bootloader supports:

- Linux EFI stub booting with initrd support
- Multi-boot with other operating systems
- Auto-detection of Windows, other Linux distros
- Keyboard navigation and configurable timeout
- Configuration via `/EFI/raven/boot.cfg` (preferred) or `/EFI/raven/boot.conf`
- Recovery mode boot option

### Boot Configuration

```conf
# /EFI/raven/boot.cfg
timeout = 5
default = 0

[entry]
name = "Raven Linux"
kernel = "\EFI\raven\vmlinuz"
initrd = "\EFI\raven\initramfs.img"
cmdline = "rdinit=/init quiet"
type = linux-efi

[entry]
name = "Raven Linux (Recovery)"
kernel = "\EFI\raven\vmlinuz"
initrd = "\EFI\raven\initramfs.img"
cmdline = "rdinit=/init single"
type = linux-efi
```

## System Requirements

### Target System
- x86_64 CPU with UEFI firmware
- 2GB RAM minimum (4GB recommended)
- 20GB disk space

### Build Host
- Linux system
- 20GB+ disk space
- 8GB+ RAM recommended
- Internet connection for downloading sources

## Contributing

Contributions welcome! Please open an issue or pull request on GitHub.

## License

MIT License - See LICENSE file
