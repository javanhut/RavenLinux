package config

import (
	"encoding/json"
	"os"
	"path/filepath"
	"sync"
)

// Settings holds all file manager configuration
type Settings struct {
	ViewMode         string     `json:"view_mode"`
	ShowHidden       bool       `json:"show_hidden"`
	SortBy           string     `json:"sort_by"`
	SortDescending   bool       `json:"sort_descending"`
	ShowPreview      bool       `json:"show_preview"`
	PreviewSize      int        `json:"preview_size"`
	SidebarWidth     int        `json:"sidebar_width"`
	DefaultPath      string     `json:"default_path"`
	ConfirmDelete    bool       `json:"confirm_delete"`
	SingleClick      bool       `json:"single_click"`
	ShowStatusBar    bool       `json:"show_status_bar"`
	RecentFiles      []string   `json:"recent_files"`
	Bookmarks        []Bookmark `json:"bookmarks"`
	SearchContentMax int64      `json:"search_content_max"`
	WindowWidth      int        `json:"window_width"`
	WindowHeight     int        `json:"window_height"`
}

// Bookmark represents a saved location
type Bookmark struct {
	Name string `json:"name"`
	Path string `json:"path"`
	Icon string `json:"icon"`
}

var (
	settingsMu   sync.RWMutex
	settingsPath string
)

func GetSettingsPath() string {
	if settingsPath != "" {
		return settingsPath
	}
	home := os.Getenv("HOME")
	configDir := filepath.Join(home, ".config", "raven", "file-manager")
	os.MkdirAll(configDir, 0755)
	settingsPath = filepath.Join(configDir, "settings.json")
	return settingsPath
}

func DefaultSettings() Settings {
	home := os.Getenv("HOME")
	return Settings{
		ViewMode:       "list",
		ShowHidden:     false,
		SortBy:         "name",
		SortDescending: false,
		ShowPreview:    true,
		PreviewSize:    300,
		SidebarWidth:   200,
		DefaultPath:    home,
		ConfirmDelete:  true,
		SingleClick:    false,
		ShowStatusBar:  true,
		RecentFiles:    []string{},
		Bookmarks: []Bookmark{
			{Name: "Home", Path: home, Icon: "user-home"},
			{Name: "Desktop", Path: filepath.Join(home, "Desktop"), Icon: "user-desktop"},
			{Name: "Documents", Path: filepath.Join(home, "Documents"), Icon: "folder-documents"},
			{Name: "Downloads", Path: filepath.Join(home, "Downloads"), Icon: "folder-download"},
			{Name: "Pictures", Path: filepath.Join(home, "Pictures"), Icon: "folder-pictures"},
			{Name: "Videos", Path: filepath.Join(home, "Videos"), Icon: "folder-videos"},
			{Name: "Music", Path: filepath.Join(home, "Music"), Icon: "folder-music"},
		},
		SearchContentMax: 10 * 1024 * 1024,
		WindowWidth:      1200,
		WindowHeight:     800,
	}
}

func LoadSettings() Settings {
	settingsMu.Lock()
	defer settingsMu.Unlock()

	settings := DefaultSettings()

	data, err := os.ReadFile(GetSettingsPath())
	if err != nil {
		return settings
	}

	var loaded Settings
	if err := json.Unmarshal(data, &loaded); err != nil {
		return settings
	}

	// Merge loaded settings with defaults
	if loaded.ViewMode != "" {
		settings.ViewMode = loaded.ViewMode
	}
	settings.ShowHidden = loaded.ShowHidden
	if loaded.SortBy != "" {
		settings.SortBy = loaded.SortBy
	}
	settings.SortDescending = loaded.SortDescending
	settings.ShowPreview = loaded.ShowPreview
	if loaded.PreviewSize > 0 {
		settings.PreviewSize = loaded.PreviewSize
	}
	if loaded.SidebarWidth > 0 {
		settings.SidebarWidth = loaded.SidebarWidth
	}
	if loaded.DefaultPath != "" {
		settings.DefaultPath = loaded.DefaultPath
	}
	settings.ConfirmDelete = loaded.ConfirmDelete
	settings.SingleClick = loaded.SingleClick
	settings.ShowStatusBar = loaded.ShowStatusBar
	if len(loaded.RecentFiles) > 0 {
		settings.RecentFiles = loaded.RecentFiles
	}
	if len(loaded.Bookmarks) > 0 {
		settings.Bookmarks = loaded.Bookmarks
	}
	if loaded.SearchContentMax > 0 {
		settings.SearchContentMax = loaded.SearchContentMax
	}
	if loaded.WindowWidth > 0 {
		settings.WindowWidth = loaded.WindowWidth
	}
	if loaded.WindowHeight > 0 {
		settings.WindowHeight = loaded.WindowHeight
	}

	return settings
}

func SaveSettings(settings Settings) {
	settingsMu.Lock()
	defer settingsMu.Unlock()

	data, err := json.MarshalIndent(settings, "", "  ")
	if err != nil {
		return
	}

	os.WriteFile(GetSettingsPath(), data, 0644)
}

func AddRecentFile(settings *Settings, path string) {
	// Remove if already exists
	recent := make([]string, 0, len(settings.RecentFiles))
	for _, r := range settings.RecentFiles {
		if r != path {
			recent = append(recent, r)
		}
	}

	// Add to front
	recent = append([]string{path}, recent...)

	// Keep only last 20
	if len(recent) > 20 {
		recent = recent[:20]
	}

	settings.RecentFiles = recent
	SaveSettings(*settings)
}
