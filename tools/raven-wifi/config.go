package main

import (
	"encoding/json"
	"os"
	"path/filepath"
	"time"
)

// Config holds application configuration
type Config struct {
	Window WindowConfig `json:"window"`
}

// WindowConfig stores window geometry
type WindowConfig struct {
	Width  int `json:"width"`
	Height int `json:"height"`
}

// LoadConfig loads configuration from disk or returns defaults
func LoadConfig() *Config {
	cfg := &Config{
		Window: WindowConfig{
			Width:  400,
			Height: 550,
		},
	}

	path := getConfigPath()
	data, err := os.ReadFile(path)
	if err != nil {
		// Config doesn't exist or can't be read - use defaults
		return cfg
	}

	// Try to parse config
	if err := json.Unmarshal(data, cfg); err != nil {
		// Corrupt config - use defaults
		return cfg
	}

	// Validate dimensions (ensure reasonable values)
	if cfg.Window.Width < 300 {
		cfg.Window.Width = 400
	}
	if cfg.Window.Height < 400 {
		cfg.Window.Height = 550
	}

	return cfg
}

// Save writes the configuration to disk
func (c *Config) Save() error {
	path := getConfigPath()

	// Ensure config directory exists
	dir := filepath.Dir(path)
	if err := os.MkdirAll(dir, 0755); err != nil {
		return err
	}

	// Marshal to JSON
	data, err := json.MarshalIndent(c, "", "  ")
	if err != nil {
		return err
	}

	// Write to file
	return os.WriteFile(path, data, 0644)
}

// getConfigPath returns the path to the config file
func getConfigPath() string {
	home, err := os.UserHomeDir()
	if err != nil {
		// Fallback to /tmp if we can't get home dir
		return "/tmp/raven-wifi-config.json"
	}
	return filepath.Join(home, ".config", "raven-wifi", "window.json")
}

// SaveDebouncer handles debounced config saves
type SaveDebouncer struct {
	config       *Config
	lastSaveTime time.Time
	saveTimer    *time.Timer
	delay        time.Duration
}

// NewSaveDebouncer creates a new save debouncer
func NewSaveDebouncer(cfg *Config, delay time.Duration) *SaveDebouncer {
	return &SaveDebouncer{
		config: cfg,
		delay:  delay,
	}
}

// RequestSave queues a save operation (debounced)
func (sd *SaveDebouncer) RequestSave() {
	if sd.saveTimer != nil {
		sd.saveTimer.Stop()
	}

	sd.saveTimer = time.AfterFunc(sd.delay, func() {
		sd.config.Save()
	})
}
