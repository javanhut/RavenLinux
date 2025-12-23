package fileview

import (
	"fmt"
	"io/fs"
	"mime"
	"net/http"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"syscall"
	"time"
)

// FileEntry represents a file or directory
type FileEntry struct {
	Name       string
	Path       string
	Size       int64
	ModTime    time.Time
	Mode       fs.FileMode
	IsDir      bool
	IsHidden   bool
	MimeType   string
	Icon       string
	IsSymlink  bool
	LinkTarget string
}

// ReadDirectory reads all entries from a directory
func ReadDirectory(path string) ([]FileEntry, error) {
	entries, err := os.ReadDir(path)
	if err != nil {
		return nil, err
	}

	var files []FileEntry

	for _, entry := range entries {
		info, err := entry.Info()
		if err != nil {
			continue
		}

		name := entry.Name()
		fullPath := filepath.Join(path, name)

		isHidden := strings.HasPrefix(name, ".")

		var isSymlink bool
		var linkTarget string
		if info.Mode()&os.ModeSymlink != 0 {
			isSymlink = true
			linkTarget, _ = os.Readlink(fullPath)
		}

		mimeType := ""
		if !entry.IsDir() {
			mimeType = GetMimeType(fullPath)
		}

		files = append(files, FileEntry{
			Name:       name,
			Path:       fullPath,
			Size:       info.Size(),
			ModTime:    info.ModTime(),
			Mode:       info.Mode(),
			IsDir:      entry.IsDir(),
			IsHidden:   isHidden,
			MimeType:   mimeType,
			IsSymlink:  isSymlink,
			LinkTarget: linkTarget,
		})
	}

	return files, nil
}

// SortEntries sorts file entries by the given criteria
func SortEntries(entries []FileEntry, sortBy string, descending bool) []FileEntry {
	sorted := make([]FileEntry, len(entries))
	copy(sorted, entries)

	sort.Slice(sorted, func(i, j int) bool {
		// Directories first
		if sorted[i].IsDir != sorted[j].IsDir {
			return sorted[i].IsDir
		}

		var less bool
		switch sortBy {
		case "name":
			less = strings.ToLower(sorted[i].Name) < strings.ToLower(sorted[j].Name)
		case "size":
			less = sorted[i].Size < sorted[j].Size
		case "date":
			less = sorted[i].ModTime.Before(sorted[j].ModTime)
		case "type":
			extI := filepath.Ext(sorted[i].Name)
			extJ := filepath.Ext(sorted[j].Name)
			if extI == extJ {
				less = strings.ToLower(sorted[i].Name) < strings.ToLower(sorted[j].Name)
			} else {
				less = extI < extJ
			}
		default:
			less = strings.ToLower(sorted[i].Name) < strings.ToLower(sorted[j].Name)
		}

		if descending {
			return !less
		}
		return less
	})

	return sorted
}

// GetMimeType returns the MIME type of a file
func GetMimeType(path string) string {
	ext := filepath.Ext(path)
	if ext != "" {
		mimeType := mime.TypeByExtension(ext)
		if mimeType != "" {
			return strings.Split(mimeType, ";")[0]
		}
	}

	file, err := os.Open(path)
	if err != nil {
		return "application/octet-stream"
	}
	defer file.Close()

	buffer := make([]byte, 512)
	n, err := file.Read(buffer)
	if err != nil || n == 0 {
		return "application/octet-stream"
	}

	return http.DetectContentType(buffer[:n])
}

// GetFileIcon returns the appropriate icon name for a file
func GetFileIcon(entry FileEntry) string {
	if entry.IsDir {
		name := strings.ToLower(entry.Name)
		switch name {
		case "documents":
			return "folder-documents-symbolic"
		case "downloads":
			return "folder-download-symbolic"
		case "pictures", "photos":
			return "folder-pictures-symbolic"
		case "videos", "movies":
			return "folder-videos-symbolic"
		case "music":
			return "folder-music-symbolic"
		case "desktop":
			return "user-desktop-symbolic"
		default:
			return "folder-symbolic"
		}
	}

	ext := strings.ToLower(filepath.Ext(entry.Name))
	switch ext {
	case ".pdf":
		return "application-pdf-symbolic"
	case ".doc", ".docx", ".odt":
		return "x-office-document-symbolic"
	case ".xls", ".xlsx", ".ods":
		return "x-office-spreadsheet-symbolic"
	case ".ppt", ".pptx", ".odp":
		return "x-office-presentation-symbolic"
	case ".txt", ".md", ".markdown":
		return "text-x-generic-symbolic"
	case ".png", ".jpg", ".jpeg", ".gif", ".bmp", ".svg", ".webp", ".ico":
		return "image-x-generic-symbolic"
	case ".mp4", ".mkv", ".avi", ".mov", ".webm", ".flv":
		return "video-x-generic-symbolic"
	case ".mp3", ".wav", ".flac", ".ogg", ".m4a", ".aac":
		return "audio-x-generic-symbolic"
	case ".zip", ".tar", ".gz", ".bz2", ".xz", ".7z", ".rar":
		return "package-x-generic-symbolic"
	case ".go", ".py", ".js", ".ts", ".rs", ".c", ".cpp", ".h", ".hpp", ".java":
		return "text-x-script-symbolic"
	case ".html", ".htm":
		return "text-html-symbolic"
	case ".css":
		return "text-css-symbolic"
	case ".json":
		return "text-x-generic-symbolic"
	case ".xml":
		return "text-xml-symbolic"
	case ".sh", ".bash":
		return "text-x-script-symbolic"
	case ".exe", ".msi", ".appimage":
		return "application-x-executable-symbolic"
	case ".deb", ".rpm":
		return "package-x-generic-symbolic"
	default:
		if strings.HasPrefix(entry.MimeType, "text/") {
			return "text-x-generic-symbolic"
		}
		if strings.HasPrefix(entry.MimeType, "image/") {
			return "image-x-generic-symbolic"
		}
		if strings.HasPrefix(entry.MimeType, "video/") {
			return "video-x-generic-symbolic"
		}
		if strings.HasPrefix(entry.MimeType, "audio/") {
			return "audio-x-generic-symbolic"
		}
		return "text-x-generic-symbolic"
	}
}

// HumanizeSize converts bytes to human-readable format
func HumanizeSize(size int64) string {
	if size < 1024 {
		return fmt.Sprintf("%d B", size)
	}
	if size < 1024*1024 {
		return fmt.Sprintf("%.1f KB", float64(size)/1024)
	}
	if size < 1024*1024*1024 {
		return fmt.Sprintf("%.1f MB", float64(size)/(1024*1024))
	}
	return fmt.Sprintf("%.1f GB", float64(size)/(1024*1024*1024))
}

// FormatDate formats a time for display
func FormatDate(t time.Time) string {
	now := time.Now()

	if t.Year() == now.Year() && t.YearDay() == now.YearDay() {
		return t.Format("3:04 PM")
	}

	yesterday := now.AddDate(0, 0, -1)
	if t.Year() == yesterday.Year() && t.YearDay() == yesterday.YearDay() {
		return "Yesterday"
	}

	if t.Year() == now.Year() {
		return t.Format("Jan 2")
	}

	return t.Format("Jan 2, 2006")
}

// GetParentPath returns the parent directory path
func GetParentPath(path string) string {
	parent := filepath.Dir(path)
	if parent == path {
		return "/"
	}
	return parent
}

// GetDiskSpace returns free and total disk space for a path
func GetDiskSpace(path string) (free, total int64) {
	var stat syscall.Statfs_t
	err := syscall.Statfs(path, &stat)
	if err != nil {
		return 0, 0
	}

	total = int64(stat.Blocks) * int64(stat.Bsize)
	free = int64(stat.Bavail) * int64(stat.Bsize)
	return free, total
}

// IsTextFile checks if a file is a text file
func IsTextFile(path string) bool {
	mimeType := GetMimeType(path)
	return strings.HasPrefix(mimeType, "text/") ||
		mimeType == "application/json" ||
		mimeType == "application/xml" ||
		mimeType == "application/javascript"
}

// IsImageFile checks if a file is an image
func IsImageFile(path string) bool {
	mimeType := GetMimeType(path)
	return strings.HasPrefix(mimeType, "image/")
}

// IsCodeFile checks if a file is source code
func IsCodeFile(path string) bool {
	ext := strings.ToLower(filepath.Ext(path))
	codeExtensions := map[string]bool{
		".go": true, ".py": true, ".js": true, ".ts": true, ".jsx": true, ".tsx": true,
		".rs": true, ".c": true, ".cpp": true, ".h": true, ".hpp": true, ".java": true,
		".rb": true, ".php": true, ".swift": true, ".kt": true, ".scala": true,
		".html": true, ".css": true, ".scss": true, ".less": true, ".sass": true,
		".json": true, ".xml": true, ".yaml": true, ".yml": true, ".toml": true,
		".sh": true, ".bash": true, ".zsh": true, ".fish": true,
		".sql": true, ".graphql": true, ".proto": true,
		".md": true, ".markdown": true, ".rst": true,
		".vue": true, ".svelte": true,
		".lua": true, ".perl": true, ".r": true,
		".dockerfile": true, ".makefile": true,
	}
	return codeExtensions[ext]
}

// IsBinaryFile checks if a file is binary
func IsBinaryFile(path string) bool {
	file, err := os.Open(path)
	if err != nil {
		return true
	}
	defer file.Close()

	buffer := make([]byte, 512)
	n, err := file.Read(buffer)
	if err != nil || n == 0 {
		return true
	}

	for i := 0; i < n; i++ {
		if buffer[i] == 0 {
			return true
		}
	}

	return false
}

// FileExists checks if a file exists
func FileExists(path string) bool {
	_, err := os.Stat(path)
	return err == nil
}

// IsDirectory checks if a path is a directory
func IsDirectory(path string) bool {
	info, err := os.Stat(path)
	if err != nil {
		return false
	}
	return info.IsDir()
}

// GetFileTypeDescription returns a human-readable file type
func GetFileTypeDescription(entry FileEntry) string {
	if entry.IsDir {
		return "Folder"
	}

	ext := filepath.Ext(entry.Name)
	if ext == "" {
		return "File"
	}

	descriptions := map[string]string{
		".txt": "Text Document", ".md": "Markdown Document", ".pdf": "PDF Document",
		".doc": "Word Document", ".docx": "Word Document",
		".xls": "Excel Spreadsheet", ".xlsx": "Excel Spreadsheet",
		".png": "PNG Image", ".jpg": "JPEG Image", ".jpeg": "JPEG Image",
		".gif": "GIF Image", ".svg": "SVG Image",
		".mp3": "MP3 Audio", ".wav": "WAV Audio", ".flac": "FLAC Audio",
		".mp4": "MP4 Video", ".mkv": "MKV Video", ".avi": "AVI Video",
		".zip": "ZIP Archive", ".tar": "TAR Archive", ".gz": "Gzip Archive",
		".go": "Go Source Code", ".py": "Python Script",
		".js": "JavaScript", ".ts": "TypeScript",
		".rs": "Rust Source Code", ".c": "C Source Code", ".cpp": "C++ Source Code",
		".java": "Java Source Code", ".html": "HTML Document",
		".css": "CSS Stylesheet", ".json": "JSON Data",
		".xml": "XML Document", ".yaml": "YAML Document", ".yml": "YAML Document",
		".sh": "Shell Script",
	}

	if desc, ok := descriptions[ext]; ok {
		return desc
	}

	return ext[1:] + " File"
}

// Pluralize returns singular or plural form based on count
func Pluralize(count int, singular, plural string) string {
	if count == 1 {
		return fmt.Sprintf("1 %s", singular)
	}
	return fmt.Sprintf("%d %s", count, plural)
}
