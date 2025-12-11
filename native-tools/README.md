# RavenLinux Native Tools

This directory contains the native tools that are first-class citizens in RavenLinux.
These are your custom Go applications that will be integrated into the core OS.

## Directory Structure

```
native-tools/
├── editor/         # Your custom file editor
│   └── (your editor source)
├── vcs/            # Your custom version control system
│   └── (your vcs source)
├── lang/           # Your custom programming language
│   └── (your language source)
└── README.md
```

## Integration Points

### 1. Default Editor
Your editor will be the default for:
- `$EDITOR` environment variable
- `rvn edit` command
- File manager "Open With" default
- Git commit messages
- System configuration editing

### 2. Version Control System
Your VCS will be:
- Available system-wide
- Integrated into `rvn` for package development
- Used by RavenDE file manager for status indicators
- Git will remain available for compatibility with external projects

### 3. Programming Language
Your language will be:
- Pre-installed on all RavenLinux systems
- Integrated into the SDK (`rvn dev yourlang`)
- Syntax highlighting in your editor
- LSP support if available

## Package Definitions

Each tool should have a `package.toml` for the RavenLinux package system:

```toml
# Example: native-tools/editor/package.toml
[package]
name = "raven-editor"  # or your tool's name
version = "1.0.0"
description = "The native RavenLinux editor"
license = "your-license"
categories = ["native", "editor"]

[source]
type = "local"
path = "/usr/src/raven/native-tools/editor"

[build]
system = "go"
build_flags = ["-ldflags", "-s -w"]

[dependencies]
runtime = []
build = ["go"]
```

## Building Native Tools

During the RavenLinux build process:

```bash
# Stage 3 will build native tools
./scripts/build.sh stage3

# Or build individually
cd native-tools/editor
go build -o raven-editor .
```

## Adding Your Tools

1. Copy/clone your tool source into the appropriate directory
2. Create a `package.toml` for each tool
3. The build system will automatically detect and build them

### Editor Integration
```bash
# Copy your editor
cp -r /path/to/your/editor/* native-tools/editor/

# Create package definition
# Edit native-tools/editor/package.toml
```

### VCS Integration
```bash
# Copy your VCS
cp -r /path/to/your/vcs/* native-tools/vcs/
```

### Language Integration
```bash
# Copy your language
cp -r /path/to/your/lang/* native-tools/lang/
```

## Environment Configuration

RavenLinux will set up these environment variables by default:

```bash
# /etc/profile.d/raven-native.sh
export EDITOR="raven-editor"        # Your editor
export VISUAL="raven-editor"
export RAVEN_VCS="your-vcs"         # Your VCS command
export RAVEN_LANG_PATH="/usr/lib/your-lang"
```

## Desktop Integration

The RavenDE desktop environment will have special integrations:

1. **File Manager**: Shows VCS status using your VCS
2. **Terminal**: Your editor opens in current directory
3. **Launcher**: Quick access to your tools
4. **Workspace**: Language-aware project templates
