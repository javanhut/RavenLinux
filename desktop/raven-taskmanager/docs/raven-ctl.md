# raven-ctl

`raven-ctl` is a command-line utility for controlling windows in the Raven Desktop environment. It provides a unified interface for window management operations using Hyprland's native IPC.

## Installation

Build `raven-ctl` from the project root:

```bash
cargo build --release
```

Install to your PATH:

```bash
sudo cp target/release/raven-ctl /usr/local/bin/
```

## Usage

```
raven-ctl <command> [arguments]
```

## Commands

### focus

Focus the window belonging to a process.

```bash
raven-ctl focus <pid>
```

**Example:**
```bash
raven-ctl focus 12345
```

If the window is minimized (in a special workspace), it will be restored first before being focused.

### minimize

Minimize a window to Hyprland's special workspace.

```bash
raven-ctl minimize <pid>
```

**Example:**
```bash
raven-ctl minimize 12345
```

### restore

Restore a minimized window.

```bash
raven-ctl restore <pid>
```

**Example:**
```bash
raven-ctl restore 12345
```

### close

Close a window gracefully.

```bash
raven-ctl close <pid>
```

**Example:**
```bash
raven-ctl close 12345
```

### list

List all windows.

```bash
raven-ctl list
```

**Output format:**
```
<pid>	<app_id>	<title> [status]
```

Status can be `[focused]` or `[minimized]`.

### active

Get the currently active/focused window.

```bash
raven-ctl active
```

### version

Print version information.

```bash
raven-ctl version
```

### help

Print help message.

```bash
raven-ctl help
```

## Hyprland Integration

`raven-ctl` uses the `hyprland-rs` crate to communicate directly with Hyprland's IPC socket. This provides:

- **Zero subprocess overhead**: No fork+exec like the previous Go implementation
- **Direct socket communication**: Uses Hyprland's native JSON IPC
- **Type-safe API**: Compile-time checking of dispatch commands

### Window Operations

| Action | Hyprland Dispatch |
|--------|-------------------|
| Focus | `FocusWindow(Address)` |
| Close | `CloseWindow(Address)` |
| Minimize | `MoveToWorkspaceSilent(Special("minimized"))` |
| Restore | `MoveToWorkspaceSilent(Relative(0))` + `FocusWindow` |

### Window Discovery

Uses `hyprland::data::Clients::get()` to retrieve window information:
- Process ID
- Window class (app_id)
- Window title
- Workspace info
- Focus history

## Integration with raven-shell

The raven-shell panel uses `raven-ctl` for dock operations:

- Clicking a running dock item: `raven-ctl focus <pid>`
- Right-click minimize: `raven-ctl minimize <pid>`
- Right-click restore: `raven-ctl restore <pid>`
- Right-click close: `raven-ctl close <pid>`

Note: raven-shell also has built-in Hyprland IPC for window tracking, but `raven-ctl` can be used independently for scripting.

## Exit Codes

| Code | Description |
|------|-------------|
| 0 | Success |
| 1 | Error (invalid arguments, window not found, operation failed) |

## Environment Variables

| Variable | Description |
|----------|-------------|
| `XDG_RUNTIME_DIR` | Base directory for Hyprland socket |
| `HYPRLAND_INSTANCE_SIGNATURE` | Hyprland instance identifier |

## Examples

List all windows with their status:
```bash
$ raven-ctl list
12345	firefox	Mozilla Firefox [focused]
23456	kitty	Terminal
34567	code	Visual Studio Code [minimized]
```

Focus a specific window:
```bash
$ raven-ctl focus 23456
```

Get the active window:
```bash
$ raven-ctl active
12345	firefox	Mozilla Firefox
```

Minimize and restore a window:
```bash
$ raven-ctl minimize 12345
$ raven-ctl restore 12345
```

## See Also

- [Dock Documentation](dock.md) - Raven Shell dock integration
- [Hyprland Wiki](https://wiki.hypr.land/IPC/) - Hyprland IPC documentation
