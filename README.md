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
- **Custom Desktop Environment** (RavenDE) - Wayland compositor built for coding workflows
- **Custom Bootloader** (RavenBoot) - Multi-boot UEFI bootloader for coexistence with other OSes
- **Native Tools Integration** - First-class support for custom development tools
- **Rolling Release** - Always up-to-date packages

## Technology Stack

| Component | Language | Purpose |
|-----------|----------|---------|
| Kernel | C | Linux kernel |
| Bootloader | Rust | UEFI multi-boot loader |
| Package Manager | Rust | System package management |
| Compositor | Rust | Wayland desktop compositor |
| Installer | Rust | System installer |
| Native Tools | Go | Custom editor, VCS, language |

## Quick Start

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

# Check status
./scripts/dev-env.sh status
```

### Building

```bash
# Build cross-compilation toolchain
./scripts/build.sh stage0

# Build base system
./scripts/build.sh stage1

# Native rebuild
./scripts/build.sh stage2

# Build packages
./scripts/build.sh stage3

# Generate ISO
./scripts/build.sh stage4

# Or build everything
./scripts/build.sh all
```

## Project Structure

```
RavenLinux/
├── bootloader/          # Custom UEFI bootloader (Rust)
├── build/               # Build artifacts (generated)
├── configs/             # System configurations
├── desktop/             # RavenDE desktop environment
│   ├── compositor/      # Wayland compositor (Rust)
│   ├── panel/           # Top panel
│   ├── launcher/        # Application launcher
│   ├── files/           # File manager
│   ├── terminal/        # Terminal emulator
│   └── settings/        # System settings
├── etc/                 # Base system configuration
├── installer/           # System installer (Rust)
├── iso/                 # ISO generation files
├── native-tools/        # Your custom Go tools
│   ├── editor/          # Custom file editor
│   ├── vcs/             # Custom version control
│   └── lang/            # Custom programming language
├── packages/            # Package definitions
│   ├── core/            # Core system packages
│   ├── base/            # Base utilities
│   ├── desktop/         # Desktop packages
│   └── dev-tools/       # Developer tools
├── scripts/             # Build and utility scripts
│   ├── build.sh         # Main build script
│   ├── dev-env.sh       # Development environment
│   └── stages/          # Build stage scripts
└── tools/
    └── rvn/             # Package manager (Rust)
```

## Package Manager (rvn)

```bash
# Install packages
rvn install rust nodejs python

# Search packages
rvn search editor

# Developer tools
rvn dev rust           # Set up Rust toolchain
rvn dev node 20        # Set up Node.js 20
rvn dev docker         # Set up containers

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

- Multi-boot with other operating systems
- Auto-detection of Windows, other Linux distros
- Keyboard navigation and timeout
- Configuration via `/EFI/raven/boot.conf`
- Recovery mode boot option

## Desktop Environment (RavenDE)

- **Compositor**: Wayland-native with tiling + floating modes
- **Keybindings**: Vim-style navigation (Super+H/J/K/L)
- **Workspaces**: Dynamic workspace management
- **Developer Focus**: Git integration, terminal-centric

Default shortcuts:
- `Super + Enter` - Terminal
- `Super + Space` - Launcher
- `Super + Q` - Close window
- `Super + 1-9` - Switch workspace
- `Super + H/J/K/L` - Focus navigation

## Native Tools

RavenLinux includes first-class support for custom development tools:

1. **Custom Editor** - Default system editor
2. **Custom VCS** - Integrated version control
3. **Custom Language** - Pre-installed programming language

See `native-tools/README.md` for integration details.

## Requirements

### Build Host
- Linux system (tested on Arch Linux)
- 20GB+ disk space
- 8GB+ RAM recommended
- GCC, Make, CMake, Meson
- Rust toolchain
- Go toolchain

### Target System
- x86_64 CPU
- UEFI firmware
- 2GB RAM minimum (4GB recommended)
- 20GB disk space

## Development

### Testing Changes

1. Edit files in `build/dev-merged/` (after mounting overlay)
2. Test in chroot: `./scripts/dev-env.sh chroot`
3. Test in VM: `./scripts/dev-env.sh qemu -g`

### Adding Packages

Create `packages/<category>/<name>/package.toml`:

```toml
[package]
name = "example"
version = "1.0.0"
description = "Example package"

[source]
type = "tarball"
url = "https://example.com/example-1.0.0.tar.gz"
sha256 = "..."

[build]
system = "make"

[dependencies]
runtime = ["libc"]
build = ["gcc"]
```

## License

MIT License - See LICENSE file

## Contributing

Contributions welcome! Please read CONTRIBUTING.md first.
