package filter

import (
	"path/filepath"
	"strings"
	"time"

	"raven-file-manager/pkg/fileview"
)

// State holds the current filter configuration
type State struct {
	ShowHidden  bool
	FileTypes   []string
	SizeMin     int64
	SizeMax     int64
	DateAfter   time.Time
	DateBefore  time.Time
	NamePattern string
}

// File type filter definitions
var FileTypeFilters = map[string][]string{
	"documents": {
		".pdf", ".doc", ".docx", ".odt", ".rtf", ".txt",
		".xls", ".xlsx", ".ods", ".csv",
		".ppt", ".pptx", ".odp",
		".md", ".markdown", ".rst", ".tex",
	},
	"images": {
		".png", ".jpg", ".jpeg", ".gif", ".bmp", ".svg", ".webp",
		".ico", ".tiff", ".tif", ".psd", ".raw", ".heic", ".heif",
	},
	"videos": {
		".mp4", ".mkv", ".avi", ".mov", ".wmv", ".flv", ".webm",
		".m4v", ".mpg", ".mpeg", ".3gp", ".ogv",
	},
	"audio": {
		".mp3", ".wav", ".flac", ".ogg", ".m4a", ".aac", ".wma",
		".opus", ".aiff", ".mid", ".midi",
	},
	"archives": {
		".zip", ".tar", ".gz", ".bz2", ".xz", ".7z", ".rar",
		".tar.gz", ".tar.bz2", ".tar.xz", ".tgz", ".tbz2",
	},
	"code": {
		".go", ".py", ".js", ".ts", ".jsx", ".tsx", ".rs", ".c", ".cpp",
		".h", ".hpp", ".java", ".rb", ".php", ".swift", ".kt", ".scala",
		".cs", ".vb", ".lua", ".perl", ".r", ".m", ".mm",
		".html", ".css", ".scss", ".less", ".sass",
		".json", ".xml", ".yaml", ".yml", ".toml",
		".sql", ".graphql", ".proto",
		".sh", ".bash", ".zsh", ".fish", ".ps1",
		".dockerfile", ".makefile",
		".vue", ".svelte",
	},
}

// NewState creates a new filter state with defaults
func NewState() *State {
	return &State{
		ShowHidden: false,
		FileTypes:  nil,
		SizeMin:    0,
		SizeMax:    0,
	}
}

// SetDateFilter sets the date filter based on selection index
func (fs *State) SetDateFilter(selection int) {
	now := time.Now()
	fs.DateBefore = time.Time{}

	switch selection {
	case 0:
		fs.DateAfter = time.Time{}
	case 1:
		fs.DateAfter = time.Date(now.Year(), now.Month(), now.Day(), 0, 0, 0, 0, now.Location())
	case 2:
		weekday := int(now.Weekday())
		if weekday == 0 {
			weekday = 7
		}
		fs.DateAfter = now.AddDate(0, 0, -(weekday - 1))
		fs.DateAfter = time.Date(fs.DateAfter.Year(), fs.DateAfter.Month(), fs.DateAfter.Day(), 0, 0, 0, 0, now.Location())
	case 3:
		fs.DateAfter = time.Date(now.Year(), now.Month(), 1, 0, 0, 0, 0, now.Location())
	case 4:
		fs.DateAfter = time.Date(now.Year(), 1, 1, 0, 0, 0, 0, now.Location())
	}
}

// Matches checks if a file entry matches the current filters
func (fs *State) Matches(entry fileview.FileEntry) bool {
	if entry.IsHidden && !fs.ShowHidden {
		return false
	}

	if len(fs.FileTypes) > 0 && !entry.IsDir {
		if !fs.matchesFileType(entry) {
			return false
		}
	}

	if !entry.IsDir {
		if fs.SizeMin > 0 && entry.Size < fs.SizeMin {
			return false
		}
		if fs.SizeMax > 0 && entry.Size > fs.SizeMax {
			return false
		}
	}

	if !fs.DateAfter.IsZero() && entry.ModTime.Before(fs.DateAfter) {
		return false
	}
	if !fs.DateBefore.IsZero() && entry.ModTime.After(fs.DateBefore) {
		return false
	}

	if fs.NamePattern != "" {
		if !fs.matchesNamePattern(entry) {
			return false
		}
	}

	return true
}

func (fs *State) matchesFileType(entry fileview.FileEntry) bool {
	ext := strings.ToLower(filepath.Ext(entry.Name))

	for _, filterType := range fs.FileTypes {
		filterType = strings.ToLower(filterType)

		extensions, ok := FileTypeFilters[filterType]
		if !ok {
			continue
		}

		for _, e := range extensions {
			if ext == e {
				return true
			}
		}
	}

	return false
}

func (fs *State) matchesNamePattern(entry fileview.FileEntry) bool {
	pattern := strings.ToLower(fs.NamePattern)
	name := strings.ToLower(entry.Name)

	if strings.Contains(pattern, "*") {
		parts := strings.Split(pattern, "*")
		pos := 0
		for _, part := range parts {
			if part == "" {
				continue
			}
			idx := strings.Index(name[pos:], part)
			if idx == -1 {
				return false
			}
			pos += idx + len(part)
		}
		return true
	}

	return strings.Contains(name, pattern)
}

// IsActive returns true if any filter is active
func (fs *State) IsActive() bool {
	return len(fs.FileTypes) > 0 ||
		fs.SizeMin > 0 ||
		fs.SizeMax > 0 ||
		!fs.DateAfter.IsZero() ||
		!fs.DateBefore.IsZero() ||
		fs.NamePattern != ""
}

// Reset clears all filters
func (fs *State) Reset() {
	fs.FileTypes = nil
	fs.SizeMin = 0
	fs.SizeMax = 0
	fs.DateAfter = time.Time{}
	fs.DateBefore = time.Time{}
	fs.NamePattern = ""
}

// ApplyFilters filters entries based on the current state
func (fs *State) ApplyFilters(entries []fileview.FileEntry) []fileview.FileEntry {
	if fs == nil {
		return entries
	}

	filtered := make([]fileview.FileEntry, 0, len(entries))
	for _, entry := range entries {
		if fs.Matches(entry) {
			filtered = append(filtered, entry)
		}
	}

	return filtered
}

// GetFileTypeCategory returns the category for a file extension
func GetFileTypeCategory(ext string) string {
	ext = strings.ToLower(ext)

	for category, extensions := range FileTypeFilters {
		for _, e := range extensions {
			if ext == e {
				return category
			}
		}
	}

	return "other"
}
