# Raven File Manager

A modern GTK4-based file manager for Raven Linux with visual search and system-wide fuzzy finder capabilities.

## Features

### Core File Management
- Browse directories with list or grid view
- Copy, cut, paste, rename, and delete files
- Create new folders
- Move files to trash with recovery support
- View file properties

### Visual Search
- **fzf-style Fuzzy Finder**: Smart case-sensitive matching with scoring algorithm
- **Content Search**: Search inside file contents (grep-like)
- **Search Operators**:
  - Space for AND (match all terms)
  - `|` for OR (match any term)
  - `!` for NOT (exclude matches)

### Preview Pane
- Text file preview with scrolling
- Code preview with syntax highlighting (Go, Python, JavaScript, Rust, C/C++, Shell, JSON, YAML)
- Image preview with metadata (dimensions, format, size)
- Directory preview (file count, total size)

### Filters
- Filter by file type: Documents, Images, Videos, Audio, Archives, Code
- Filter by size: < 1 MB, 1-10 MB, 10-100 MB, > 100 MB
- Filter by date: Today, This Week, This Month, This Year

## Keyboard Shortcuts

### Navigation
| Shortcut | Action |
|----------|--------|
| `Alt+Left` or `Backspace` | Go back |
| `Alt+Right` | Go forward |
| `Alt+Up` | Go to parent directory |
| `Alt+Home` | Go to home directory |
| `Ctrl+L` | Focus location bar |

### Search
| Shortcut | Action |
|----------|--------|
| `Ctrl+F` | Focus search / fuzzy finder |
| `Ctrl+Shift+F` | Open content search |
| `Escape` | Clear search / cancel |

### View
| Shortcut | Action |
|----------|--------|
| `Ctrl+H` | Toggle hidden files |
| `Ctrl+P` | Toggle preview pane |
| `Ctrl+1` | List view |
| `Ctrl+2` | Grid view |
| `F5` | Refresh |

### File Operations
| Shortcut | Action |
|----------|--------|
| `Ctrl+C` | Copy selected files |
| `Ctrl+X` | Cut selected files |
| `Ctrl+V` | Paste files |
| `Ctrl+A` | Select all |
| `F2` | Rename selected |
| `Delete` | Move to trash |
| `Shift+Delete` | Permanent delete |
| `Ctrl+Shift+N` | New folder |

## Configuration

Settings are stored in `~/.config/raven/file-manager/settings.json`:

```json
{
  "view_mode": "list",
  "show_hidden": false,
  "sort_by": "name",
  "sort_descending": false,
  "show_preview": true,
  "preview_size": 300,
  "sidebar_width": 200,
  "confirm_delete": true,
  "show_status_bar": true,
  "bookmarks": [
    {"name": "Home", "path": "/home/user", "icon": "user-home"},
    {"name": "Documents", "path": "/home/user/Documents", "icon": "folder-documents"}
  ],
  "search_content_max": 10485760
}
```

### Settings Options

| Setting | Type | Description |
|---------|------|-------------|
| `view_mode` | string | "list" or "grid" |
| `show_hidden` | bool | Show hidden files (starting with .) |
| `sort_by` | string | "name", "size", "date", or "type" |
| `sort_descending` | bool | Reverse sort order |
| `show_preview` | bool | Show preview pane |
| `preview_size` | int | Width of preview pane in pixels |
| `sidebar_width` | int | Width of sidebar in pixels |
| `confirm_delete` | bool | Show confirmation before deleting |
| `single_click` | bool | Open files with single click |
| `bookmarks` | array | Custom sidebar bookmarks |
| `search_content_max` | int | Maximum file size for content search (bytes) |

## Architecture

### Source Files

| File | Description |
|------|-------------|
| `main.go` | Application entry point, keyboard shortcuts |
| `ui.go` | Main UI layout, window creation |
| `fileview.go` | File entry struct, directory reading, file operations |
| `search.go` | Fuzzy finder, content search engine |
| `preview.go` | Preview pane for different file types |
| `syntax.go` | Syntax highlighting for code files |
| `filter.go` | File type, size, date filters |
| `navigation.go` | History (back/forward), bookmarks |
| `clipboard.go` | Cut/copy/paste operations |
| `dialogs.go` | Rename, delete, new folder dialogs |
| `config.go` | Settings management |
| `css.go` | Dark theme CSS styles |

### Technology Stack

- **Language**: Go 1.23
- **UI Framework**: GTK4 via gotk4/pkg v0.3.1
- **Design**: Dark theme matching Raven Linux desktop
  - Background: #0f1720
  - Accent: #009688 (teal)
  - Text: #e0e0e0

## Building

```bash
cd raven-file-manager
go build -o raven-file-manager
```

### Dependencies

The following system packages are required:
- gtk4
- gobject-introspection

Install on Arch Linux:
```bash
sudo pacman -S gtk4 gobject-introspection
```

## Usage

```bash
# Launch file manager in home directory
raven-file-manager

# Launch in specific directory
raven-file-manager /path/to/directory
```

## Fuzzy Search Syntax

The fuzzy finder uses fzf-style matching:

- **Smart case**: Lowercase patterns are case-insensitive, uppercase makes it case-sensitive
- **Scoring**: Matches are ranked by:
  - Consecutive character matches (bonus)
  - Word boundary matches (after /, ., _, -)
  - CamelCase matches
  - First character matches
  - Shorter paths preferred

### Examples

| Pattern | Matches |
|---------|---------|
| `main` | main.go, main_test.go, domain.go |
| `Main` | Only files with capital M (case-sensitive) |
| `mg` | main.go, mylog.go (fuzzy match) |
| `src test` | Files matching both "src" AND "test" |
| `go \| rs` | Files matching "go" OR "rs" |
| `!test` | Files NOT matching "test" |

## Content Search

Content search (Ctrl+Shift+F) searches inside file contents:

- Searches text files only (binary files are skipped)
- Maximum file size configurable (default 10MB)
- Shows matching line with context
- Click result to open file at matching line

## Integration

### Desktop Entry

To add to application menu, create `~/.local/share/applications/raven-file-manager.desktop`:

```ini
[Desktop Entry]
Name=Raven Files
Comment=Browse and manage files
Exec=raven-file-manager
Icon=system-file-manager
Terminal=false
Type=Application
Categories=System;FileManager;
```

### Hyprland Keybinding

Add to `~/.config/hypr/hyprland.conf`:

```conf
bind = SUPER, E, exec, raven-file-manager
```
