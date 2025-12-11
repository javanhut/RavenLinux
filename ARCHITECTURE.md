# RavenLinux Architecture

## Overview
RavenLinux is an independent, developer-focused Linux distribution built from scratch with a custom desktop environment and package manager.

## Design Principles
1. **Developer Experience First** - Every decision optimized for coding workflows
2. **Performance** - Fast boot, fast package management, responsive DE
3. **Simplicity** - Clear, understandable system design
4. **Reproducibility** - Declarative configuration where possible

## System Components

### 1. Base System (raven-core)
- **Kernel**: Linux kernel (LTS or latest stable)
- **C Library**: musl libc (lightweight, secure) or glibc
- **Init System**: Custom `raven-init` or systemd
- **Core Utilities**: uutils (Rust coreutils)
- **Shell**: zsh (default), bash, sh available

### 2. Package Manager (rvn)
- Written in Rust for performance
- Declarative package definitions (TOML-based)
- Binary package distribution with source build fallback
- Atomic upgrades with rollback support
- Developer workspace management (per-project dependencies)

### 3. Desktop Environment (RavenDE)
- Built on Wayland (wlroots-based compositor)
- Custom compositor: `raven-compositor`
- Panel/dock: `raven-panel`
- Application launcher: `raven-launcher`
- File manager: `raven-files`
- Settings: `raven-settings`
- Notification daemon: `raven-notify`
- Built with GTK4 or Qt6

### 4. Developer Tools Integration (raven-sdk)
- Language version managers (rustup, nvm, pyenv integration)
- Container runtime (podman/docker)
- Virtual machine support (QEMU/KVM)
- Git integration throughout DE
- Terminal emulator: `raven-term`
- Code editor integration hooks

## Directory Structure

```
/
├── bin/          -> /usr/bin (symlink)
├── boot/         # Bootloader, kernel, initramfs
├── dev/          # Device files
├── etc/          # System configuration
│   ├── raven/    # RavenLinux specific configs
│   ├── rvn/      # Package manager config
│   └── ...
├── home/         # User home directories
├── lib/          -> /usr/lib (symlink)
├── lib64/        -> /usr/lib (symlink)
├── mnt/          # Mount points
├── opt/          # Optional/third-party software
├── proc/         # Process information
├── root/         # Root user home
├── run/          # Runtime data
├── sbin/         -> /usr/bin (symlink)
├── sys/          # Kernel/system information
├── tmp/          # Temporary files
├── usr/
│   ├── bin/      # All executables
│   ├── include/  # Header files
│   ├── lib/      # Libraries
│   ├── share/    # Architecture-independent data
│   └── src/      # Source code (optional)
└── var/          # Variable data
    ├── cache/    # Package cache
    ├── lib/rvn/  # Package database
    └── log/      # System logs
```

## Build System (raven-build)

### Build Stages
1. **Stage 0**: Cross-compile minimal toolchain
2. **Stage 1**: Build base system with Stage 0 toolchain
3. **Stage 2**: Rebuild everything natively
4. **Stage 3**: Build additional packages
5. **Stage 4**: Generate ISO image

### Build Dependencies (Host)
- GCC/Clang
- Make, CMake, Meson
- Rust toolchain
- Python 3
- Git

## Package Format

### Package Definition (package.toml)
```toml
[package]
name = "example"
version = "1.0.0"
description = "Example package"
license = "MIT"
homepage = "https://example.com"

[build]
system = "meson"  # or "cmake", "make", "cargo", etc.
configure = []
build = []
install = []

[dependencies]
runtime = ["libc", "libfoo"]
build = ["meson", "ninja"]

[source]
url = "https://example.com/example-1.0.0.tar.gz"
sha256 = "..."
```

### Binary Package Format
- Compressed archive (.rvn)
- Contains: metadata, file manifest, compressed files
- Supports: pre/post install scripts, triggers

## Desktop Environment Architecture

### RavenDE Components

```
┌─────────────────────────────────────────────────────────┐
│                    raven-compositor                      │
│                  (Wayland Compositor)                    │
├─────────────────────────────────────────────────────────┤
│  raven-panel  │  raven-launcher  │  raven-notify        │
├───────────────┴──────────────────┴──────────────────────┤
│                    raven-session                         │
│              (Session/Login Manager)                     │
├─────────────────────────────────────────────────────────┤
│  raven-files  │  raven-term  │  raven-settings          │
│               │              │                           │
│  (File Mgr)   │  (Terminal)  │  (System Settings)       │
└─────────────────────────────────────────────────────────┘
```

### Theming
- Custom icon theme: `raven-icons`
- GTK/Qt theme: `raven-theme`
- Cursor theme: `raven-cursors`
- Color scheme: Dark-first with accent colors

## Developer Workflow Features

### Project Environments
```bash
# Create isolated development environment
rvn workspace create myproject --lang rust,python

# Activate environment
rvn workspace enter myproject

# Environment automatically:
# - Sets up PATH for project-specific tools
# - Configures language versions
# - Mounts project-specific packages
```

### Quick Commands
```bash
rvn dev rust      # Install Rust toolchain
rvn dev node 20   # Install Node.js 20
rvn dev docker    # Set up container runtime
rvn template web  # Scaffold web project
```

## Installer (raven-installer)

### Features
- Graphical installer (runs in live environment)
- Disk partitioning (auto + manual)
- Encryption support (LUKS)
- User creation
- Package selection (minimal, standard, full)
- Post-install configuration

## Versioning

- Rolling release model
- Stable snapshots monthly
- Version format: YYYY.MM (e.g., 2025.12)

## Target Specifications

### Minimum Requirements
- CPU: x86_64 (ARM64 future)
- RAM: 2GB (4GB recommended)
- Storage: 20GB (50GB recommended)
- Graphics: Vulkan-capable GPU for compositor

### Supported Hardware
- Primary: Modern laptops/desktops
- Secondary: Virtual machines
- Future: ARM64 SBCs, servers
