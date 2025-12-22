# RavenLinux Dependency Checker

The `check-deps.sh` script automatically detects your Linux distribution and checks for all required build dependencies. If any are missing, it offers to install them for you.

## Usage

```bash
# The build script automatically checks dependencies
./scripts/build.sh

# Skip the automatic dependency check
./scripts/build.sh --skip-deps

# Run the dependency checker standalone
./scripts/check-deps.sh

# Auto-install without prompting
./scripts/check-deps.sh -y

# Only list missing dependencies (quiet mode)
./scripts/check-deps.sh -q
```

## Supported Distributions

The script supports the following Linux distributions and their derivatives:

| Distribution Family | Package Manager | Derivatives |
|---------------------|-----------------|-------------|
| Arch Linux | pacman | Artix, Manjaro, EndeavourOS, Garuda |
| Debian | apt | Ubuntu, Linux Mint, Pop!_OS, Elementary, Zorin, Kali |
| Fedora | dnf | RHEL, CentOS, Rocky, Alma, Nobara |
| openSUSE | zypper | SLES |
| Void Linux | xbps-install | - |
| Alpine Linux | apk | - |

## Required Dependencies

The script checks for the following categories of tools:

### Core Build Tools
- `make` - Build automation
- `gcc` / `g++` - GNU Compilers
- `binutils` - Linker, archiver, assembler, strip, ranlib, etc.
- `autoconf`, `automake`, `libtool`, `m4` - GNU build system

### Archive/Compression
- `tar`, `gzip`, `xz`, `bzip2`, `zstd`, `unzip`
- `cpio` - Required for initramfs creation

### Download Tools
- `curl`, `wget`

### Version Control
- `git`

### File Utilities
- `findutils`, `file`, `patch`, `rsync`, `diffutils`

### Disk/Filesystem Tools
- `squashfs-tools` - For creating squashfs images
- `xorriso` - For ISO creation
- `util-linux` - For losetup, blkid, mount, fdisk
- `e2fsprogs` - For mkfs.ext4
- `dosfstools` - For mkfs.fat (EFI partitions)

### Build Systems
- `cargo` (Rust) - For building Rust components
- `go` - For building Go components
- `meson`, `ninja`, `cmake`
- `pkg-config`

### Kernel Build Requirements
- `bc`, `flex`, `bison`
- `perl`, `python3`
- `ncurses` (development headers)
- `libelf` (development headers)

### Python Modules
- `python-jinja` / `python3-jinja2` - Jinja2 templating (required for kernel and meson)

### Graphics/Wayland Development
- `wayland`, `wayland-protocols`
- `libxkbcommon`
- `pixman`, `libdrm`, `mesa`
- `libinput`, `seatd`
- `pango`, `cairo`, `gdk-pixbuf`

### Library Development Headers
- `openssl` / `libssl-dev`
- `zlib`
- `libffi`

## Automatic Check

The `build.sh` script automatically runs the dependency check before starting the build. If any dependencies are missing, it will display them and offer to install them.

To skip the automatic check (if you know dependencies are installed):

```bash
./scripts/build.sh --skip-deps
```

## Options

| Option | Description |
|--------|-------------|
| `-y`, `--yes` | Automatically install missing packages without prompting |
| `-q`, `--quiet` | Only output missing dependencies (useful for scripting) |
| `-h`, `--help` | Show help message |

## Exit Codes

- `0` - All dependencies are installed
- `1` - Missing dependencies (or installation cancelled/failed)

## Manual Installation

If automatic installation fails or you prefer to install manually, the script will display the exact command needed. For example, on Arch Linux:

```bash
sudo pacman -S --needed cpio squashfs-tools xorriso go rust meson ninja
```
