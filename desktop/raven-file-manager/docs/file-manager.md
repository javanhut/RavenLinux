# Raven File Manager

A GTK4-based file manager for Raven Linux with system-wide fuzzy search and file content search capabilities.

## Features

- **Fuzzy Finder**: fzf-style smart search with scoring
  - Smart case: lowercase = case-insensitive, uppercase = case-sensitive
  - Scoring bonuses for consecutive matches, word boundaries, camelCase
  - Search operators: space (AND), | (OR), ! (NOT/exclude)

- **Content Search**: Search within file contents (Ctrl+Shift+F)
  - Multi-threaded file scanning
  - Binary file detection (skipped automatically)
  - Line context display with match highlighting

- **Preview Pane**: Quick file preview without opening
  - Image preview with dimensions and format info
  - Syntax highlighting for code files (Go, Python, JS, Rust, C, Shell, JSON, YAML)
  - Text preview with scrolling
  - Directory stats (file/folder count, total size)

- **Filters**: Filter files by type, size, and date
  - File types: Documents, Images, Videos, Audio, Archives, Code
  - Toggle hidden files (Ctrl+H)

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| Enter | Open selected file/folder |
| Ctrl+F | Focus search (fuzzy finder) |
| Ctrl+Shift+F | Content search mode |
| Ctrl+L | Focus location bar |
| Ctrl+H | Toggle hidden files |
| Ctrl+Shift+N | New folder |
| F2 | Rename selected |
| F5 | Refresh |
| Delete | Move to trash |
| Shift+Delete | Permanent delete |
| Ctrl+C | Copy |
| Ctrl+X | Cut |
| Ctrl+V | Paste |
| Ctrl+A | Select all |
| Backspace | Go back |
| Alt+Left | Go back |
| Alt+Right | Go forward |
| Alt+Up | Parent directory |
| Alt+Home | Home directory |
| Ctrl+P | Toggle preview pane |
| Escape | Clear search / deselect |

Note: Double-click also opens files and folders.

## Configuration

Settings are stored in `~/.config/raven/file-manager.json`:

```json
{
  "window_width": 1200,
  "window_height": 800,
  "sidebar_width": 200,
  "preview_size": 300,
  "show_preview": true,
  "show_status_bar": true,
  "show_hidden": false,
  "sort_by": "name",
  "sort_descending": false,
  "view_mode": "list",
  "recent_files": [],
  "bookmarks": [
    {"name": "Home", "path": "$HOME", "icon": "user-home-symbolic"},
    {"name": "Documents", "path": "$HOME/Documents", "icon": "folder-documents-symbolic"}
  ],
  "search_content_max": 1048576
}
```

## Package Structure

```
raven-file-manager/
  main.go                    # Application entry point and UI
  go.mod                     # Go module dependencies
  pkg/
    config/config.go         # Settings management
    css/css.go               # Dark theme styles
    navigation/navigation.go # History (back/forward)
    fileview/fileview.go     # FileEntry, directory operations
    filter/filter.go         # Type/size/date filters
    search/search.go         # Fuzzy finder and content search
    clipboard/clipboard.go   # Cut/copy/paste operations
    preview/
      preview.go             # Preview panel
      syntax.go              # Syntax highlighting
```

## Dependencies

- Go 1.23+
- GTK4 (libgtk-4-dev)
- github.com/diamondburned/gotk4/pkg v0.3.1

## Building

```bash
cd raven-file-manager
go build -o raven-file-manager
```

## Running

```bash
./raven-file-manager
```

Or directly with Go:

```bash
go run main.go
```

## Theme

Uses the Raven Linux dark theme:
- Background: #0f1720
- Secondary: #1a2332
- Accent: #009688
- Text: #e0e0e0
- Border: #2a3a50
