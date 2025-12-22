# raven-ctl

`raven-ctl` is a command-line utility for controlling windows in the Raven Desktop environment. It provides a unified interface for window management operations that works across multiple Wayland compositors.

## Installation

Build `raven-ctl` from the `cmd/raven-ctl` directory:

```bash
cd cmd/raven-ctl
go build -o raven-ctl .
```

Install to your PATH:

```bash
sudo cp raven-ctl /usr/local/bin/
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

### minimize

Minimize a window.

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

## Compositor Support

`raven-ctl` supports multiple backends with automatic fallback:

### 1. Raven Compositor (Primary)

When the Raven compositor is running, `raven-ctl` communicates via a Unix socket at:

```
$XDG_RUNTIME_DIR/raven-compositor.sock
```

The IPC protocol uses JSON messages:

**Request:**
```json
{
  "action": "focus|minimize|restore|close|list|get-active",
  "pid": 12345
}
```

**Response:**
```json
{
  "success": true,
  "error": "",
  "data": null
}
```

### 2. Hyprland

If `hyprctl` is available, `raven-ctl` uses Hyprland's IPC:

- Focus: `hyprctl dispatch focuswindow pid:<pid>`
- Close: `hyprctl dispatch closewindow pid:<pid>`
- Minimize: `hyprctl dispatch movetoworkspacesilent special,pid:<pid>`
- List: `hyprctl clients -j`
- Active: `hyprctl activewindow -j`

### 3. Sway

If `swaymsg` is available, `raven-ctl` uses Sway's IPC:

- Focus: `swaymsg [pid=<pid>] focus`
- Close: `swaymsg [pid=<pid>] kill`
- Minimize: `swaymsg [pid=<pid>] move scratchpad`
- List/Active: `swaymsg -t get_tree`

### 4. Signal Fallback

As a last resort, `raven-ctl` uses POSIX signals:

- Focus/Restore: `SIGCONT` (resume process)
- Minimize: `SIGSTOP` (pause process)
- Close: `SIGTERM` (graceful termination)

Note: Signal-based operations have limited effectiveness for window management.

## Integration with raven-shell

The raven-shell panel uses `raven-ctl` for dock operations:

- Clicking a running dock item calls `raven-ctl focus <pid>`
- Right-click minimize calls `raven-ctl minimize <pid>`
- Right-click restore calls `raven-ctl restore <pid>`
- Right-click close calls `kill <pid>` directly

## Exit Codes

| Code | Description |
|------|-------------|
| 0 | Success |
| 1 | Error (invalid arguments, operation failed) |

## Environment Variables

| Variable | Description |
|----------|-------------|
| `XDG_RUNTIME_DIR` | Base directory for the compositor socket |

## Implementing Compositor Support

To add support for the Raven compositor, implement a socket server that:

1. Listens on `$XDG_RUNTIME_DIR/raven-compositor.sock`
2. Accepts JSON-encoded `Command` messages
3. Responds with JSON-encoded `Response` messages

### Command Structure

```go
type Command struct {
    Action string `json:"action"`    // focus, minimize, restore, close, list, get-active
    PID    int    `json:"pid"`       // Process ID (for window operations)
    WinID  string `json:"win_id"`    // Optional window ID
}
```

### Response Structure

```go
type Response struct {
    Success bool   `json:"success"`
    Error   string `json:"error"`
    Data    any    `json:"data"`    // Window list or single window info
}
```

### Window Info Structure

```go
type WindowInfo struct {
    ID        string `json:"id"`
    PID       int    `json:"pid"`
    Title     string `json:"title"`
    AppID     string `json:"app_id"`
    Focused   bool   `json:"focused"`
    Minimized bool   `json:"minimized"`
}
```

## See Also

- [Dock Documentation](dock.md) - Raven Shell dock integration
