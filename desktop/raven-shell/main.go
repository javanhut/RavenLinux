package main

import (
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"sync"
	"time"
	"unsafe"

	"github.com/diamondburned/gotk4/pkg/gdk/v4"
	"github.com/diamondburned/gotk4/pkg/gio/v2"
	"github.com/diamondburned/gotk4/pkg/glib/v2"
	"github.com/diamondburned/gotk4/pkg/gtk/v4"
)

/*
#cgo pkg-config: gtk4-layer-shell-0 gtk4
#include <gtk4-layer-shell.h>
#include <gtk/gtk.h>

// orientation: 0=top, 1=bottom, 2=left, 3=right
void init_layer_shell_oriented(GtkWidget *window, int orientation) {
    gtk_layer_init_for_window(GTK_WINDOW(window));
    gtk_layer_set_layer(GTK_WINDOW(window), GTK_LAYER_SHELL_LAYER_TOP);
    gtk_layer_auto_exclusive_zone_enable(GTK_WINDOW(window));

    // Reset all anchors first
    gtk_layer_set_anchor(GTK_WINDOW(window), GTK_LAYER_SHELL_EDGE_TOP, FALSE);
    gtk_layer_set_anchor(GTK_WINDOW(window), GTK_LAYER_SHELL_EDGE_BOTTOM, FALSE);
    gtk_layer_set_anchor(GTK_WINDOW(window), GTK_LAYER_SHELL_EDGE_LEFT, FALSE);
    gtk_layer_set_anchor(GTK_WINDOW(window), GTK_LAYER_SHELL_EDGE_RIGHT, FALSE);

    switch (orientation) {
        case 0: // Top
            gtk_layer_set_anchor(GTK_WINDOW(window), GTK_LAYER_SHELL_EDGE_TOP, TRUE);
            gtk_layer_set_anchor(GTK_WINDOW(window), GTK_LAYER_SHELL_EDGE_LEFT, TRUE);
            gtk_layer_set_anchor(GTK_WINDOW(window), GTK_LAYER_SHELL_EDGE_RIGHT, TRUE);
            break;
        case 1: // Bottom
            gtk_layer_set_anchor(GTK_WINDOW(window), GTK_LAYER_SHELL_EDGE_BOTTOM, TRUE);
            gtk_layer_set_anchor(GTK_WINDOW(window), GTK_LAYER_SHELL_EDGE_LEFT, TRUE);
            gtk_layer_set_anchor(GTK_WINDOW(window), GTK_LAYER_SHELL_EDGE_RIGHT, TRUE);
            break;
        case 2: // Left
            gtk_layer_set_anchor(GTK_WINDOW(window), GTK_LAYER_SHELL_EDGE_LEFT, TRUE);
            gtk_layer_set_anchor(GTK_WINDOW(window), GTK_LAYER_SHELL_EDGE_TOP, TRUE);
            gtk_layer_set_anchor(GTK_WINDOW(window), GTK_LAYER_SHELL_EDGE_BOTTOM, TRUE);
            break;
        case 3: // Right
            gtk_layer_set_anchor(GTK_WINDOW(window), GTK_LAYER_SHELL_EDGE_RIGHT, TRUE);
            gtk_layer_set_anchor(GTK_WINDOW(window), GTK_LAYER_SHELL_EDGE_TOP, TRUE);
            gtk_layer_set_anchor(GTK_WINDOW(window), GTK_LAYER_SHELL_EDGE_BOTTOM, TRUE);
            break;
    }
}

// For popup menus - positions based on panel orientation
void init_popup_layer_shell_oriented(GtkWidget *window, int orientation, int margin1, int margin2) {
    gtk_layer_init_for_window(GTK_WINDOW(window));
    gtk_layer_set_layer(GTK_WINDOW(window), GTK_LAYER_SHELL_LAYER_OVERLAY);
    gtk_layer_set_keyboard_mode(GTK_WINDOW(window), GTK_LAYER_SHELL_KEYBOARD_MODE_ON_DEMAND);

    // Reset all anchors
    gtk_layer_set_anchor(GTK_WINDOW(window), GTK_LAYER_SHELL_EDGE_TOP, FALSE);
    gtk_layer_set_anchor(GTK_WINDOW(window), GTK_LAYER_SHELL_EDGE_BOTTOM, FALSE);
    gtk_layer_set_anchor(GTK_WINDOW(window), GTK_LAYER_SHELL_EDGE_LEFT, FALSE);
    gtk_layer_set_anchor(GTK_WINDOW(window), GTK_LAYER_SHELL_EDGE_RIGHT, FALSE);

    switch (orientation) {
        case 0: // Top panel - popup below
            gtk_layer_set_anchor(GTK_WINDOW(window), GTK_LAYER_SHELL_EDGE_TOP, TRUE);
            gtk_layer_set_anchor(GTK_WINDOW(window), GTK_LAYER_SHELL_EDGE_RIGHT, TRUE);
            gtk_layer_set_margin(GTK_WINDOW(window), GTK_LAYER_SHELL_EDGE_TOP, margin1);
            gtk_layer_set_margin(GTK_WINDOW(window), GTK_LAYER_SHELL_EDGE_RIGHT, margin2);
            break;
        case 1: // Bottom panel - popup above
            gtk_layer_set_anchor(GTK_WINDOW(window), GTK_LAYER_SHELL_EDGE_BOTTOM, TRUE);
            gtk_layer_set_anchor(GTK_WINDOW(window), GTK_LAYER_SHELL_EDGE_RIGHT, TRUE);
            gtk_layer_set_margin(GTK_WINDOW(window), GTK_LAYER_SHELL_EDGE_BOTTOM, margin1);
            gtk_layer_set_margin(GTK_WINDOW(window), GTK_LAYER_SHELL_EDGE_RIGHT, margin2);
            break;
        case 2: // Left panel - popup to right
            gtk_layer_set_anchor(GTK_WINDOW(window), GTK_LAYER_SHELL_EDGE_LEFT, TRUE);
            gtk_layer_set_anchor(GTK_WINDOW(window), GTK_LAYER_SHELL_EDGE_TOP, TRUE);
            gtk_layer_set_margin(GTK_WINDOW(window), GTK_LAYER_SHELL_EDGE_LEFT, margin1);
            gtk_layer_set_margin(GTK_WINDOW(window), GTK_LAYER_SHELL_EDGE_TOP, margin2);
            break;
        case 3: // Right panel - popup to left
            gtk_layer_set_anchor(GTK_WINDOW(window), GTK_LAYER_SHELL_EDGE_RIGHT, TRUE);
            gtk_layer_set_anchor(GTK_WINDOW(window), GTK_LAYER_SHELL_EDGE_TOP, TRUE);
            gtk_layer_set_margin(GTK_WINDOW(window), GTK_LAYER_SHELL_EDGE_RIGHT, margin1);
            gtk_layer_set_margin(GTK_WINDOW(window), GTK_LAYER_SHELL_EDGE_TOP, margin2);
            break;
    }
}

GtkWidget* get_native_widget(void* obj) {
    return GTK_WIDGET(obj);
}
*/
import "C"

const (
	panelSize = 38

	// Panel orientations
	OrientationTop    = 0
	OrientationBottom = 1
	OrientationLeft   = 2
	OrientationRight  = 3
)

// Raven icon as SVG
const ravenIconSVG = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 542.24 539.74">
  <g transform="translate(-5.4832 -9.0863)">
    <g transform="translate(-199.29 -88.571)">
      <path fill="white" d="m517.25 0c-7.93-0.050211-18.94 1.3307-24.16 2.5938-23.57 5.7107-64.79 37.921-97.25 61.344-26.86 19.384-63.77 23.094-68.5 35.75-4.48 11.992-46.1 119.03-55.68 131.81-9.59 12.78-39.54-1.01-53.38 3.25s-33-4.93-34.06-10.25c-1.07-5.32 23.04-16.3-7.31-7.91-57.85 16-48.97 8.5-35.13-6.4 13.84-14.91 15.95-11.69-4.28-11.69s-65.214 52.55-51.969 21.59c20.429-47.75 6.5-43.39 6.5-47.65s-23.158 31.52-34.375 39.9c-10.407 7.79-0.879-7.51 1.25-16.03 2.13-8.51 9.171-13.93 6.219-17-6.517-6.77-9.594-44.71-9.594-34.06s-26.607 85.8-31.812 84.69c-6.144-1.32-10.765-42.09-11.844-46.35-1.065-4.2-16.242-10.4-10.656-2.12 4.8896 7.25 0.0202 106.44 5.3435 104.31 5.3235-2.13 30.876 12.78 30.876 12.78s14.897-3.18 19.156-4.25c4.258-1.06 101.15 45.77 106.47 51.1 5.33 5.32-21.31 28.76-22.37 36.21-1.07 7.46 5.33 8.5 12.78 28.72 7.45 20.23-1.57 61.42 3.75 60.36 5.33-1.07 26.04-42.52 30.3-48.91s19.19-15.69 40.48-18.88c21.29-3.2 76.67 31.94 83.06 30.87 6.39-1.06 11.51 10.03 9.38 28.13s-19.23 18.67-16.72 51.09c0.87 11.3 18.34 14.14 28.62 26.19 2.77 3.24-5.34-8.53-7.46-12.78-2.13-4.26 6.4-7.44 6.4-7.44s-10.65-0.03-13.84-1.09c-3.2-1.07-9-16.99-2.13-21.29 22.77-14.22 27.68 1.07 34.07 0 6.38-1.06 12.78 13.83 10.65 5.32-2.13-8.52-13.85-17.03-24.5-20.22-10.65-3.2-5.03-15.93-4.12-21.28 1.59-9.31 7.05-10.03 9.06-2.87 2.62 9.32 17.58 22.44 14.35 16.45-3.38-6.26-7.51-17.26-4.2-16.74 2.11 0.33 16.86 6.64 12.6 4.22-16.33-9.28-15.03-14.1-15.91-23.76 2.13-2.13 20.34-3.52 25.9-3.14-1.94-4.65-28.08-8.26-30.21-9.32s-1.06-20.22-1.06-20.22 7.43-2.12 10.62-5.31c3.2-3.2 17.93 0.49 21.13 2.62 3.19 2.13 17.7-2.15 24.09-4.28s14.99 1.2 18.19-5.18c3.19-6.39 22.09 0 25.28-2.13s6.09-11.71 8.22-15.97 29.81-9.15 31.94-11.28 4.56-9.87 4.56-14.12c0-4.26 23.09-14.62 24.15-17.82 1.07-3.19 5.35-6.36 4.29-10.62-1.07-4.26 13.83-14.9 19.15-18.1 5.33-3.19 9.6-24.49 8.53-27.68-1.06-3.2-14.92 17.03-20.25 18.09-5.32 1.06-43.63 9.59-56.4 13.84-12.78 4.26-51.12 12.78-56.44 13.85-5.32 1.06-14.9-9.59-15.97-13.85-1.06-4.25 13.83-6.4 6.38-7.47-7.46-1.06-48.96-21.27-53.22-25.53-4.26-4.25 9.58-11.71 13.84-13.84s7.44-5.33 8.5-10.66c1.07-5.32 13.86-11.72 22.38-25.56 8.51-13.84 19.15-77.7 18.09-86.22-1.06-8.51 18.27-21.73 23.41-31.93 5.35-10.636 24.81-30.668 37.34-40.224 23.9-18.232 58.37-59.638 56.53-63.687-0.63-1.3943-4.27-1.9389-9.03-1.969zm-196.91 406.94c3.91-0.08 8.1 1.25 8.1 1.25 4.64-3.44 0.62 3.41 1.99 13.96 1.21 9.33 3.97 8.86-11.28 0.47-7.85-4.32-15.25 1.78-5.31-11.71 1.82-2.47 3.46-3.91 6.5-3.97z" transform="translate(204.77 97.658)" fill-rule="evenodd"/>
    </g>
  </g>
</svg>`

// HyprlandClient represents a window from hyprctl clients -j
type HyprlandClient struct {
	Address   string `json:"address"`
	Mapped    bool   `json:"mapped"`
	Hidden    bool   `json:"hidden"`
	At        []int  `json:"at"`
	Size      []int  `json:"size"`
	Workspace struct {
		ID   int    `json:"id"`
		Name string `json:"name"`
	} `json:"workspace"`
	Floating       bool   `json:"floating"`
	Fullscreen     int    `json:"fullscreen"`
	FullscreenMode int    `json:"fullscreenMode"`
	FakeFullscreen bool   `json:"fakeFullscreen"`
	Grouped        []any  `json:"grouped"`
	Tags           []any  `json:"tags"`
	Swallowing     string `json:"swallowing"`
	FocusHistoryID int    `json:"focusHistoryID"`
	PID            int    `json:"pid"`
	Class          string `json:"class"`
	Title          string `json:"title"`
	InitialClass   string `json:"initialClass"`
	InitialTitle   string `json:"initialTitle"`
}

// DockItem represents an application in the dock
type DockItem struct {
	ID          string `json:"id"`
	Name        string `json:"name"`
	Command     string `json:"command"`
	Icon        string `json:"icon"`
	Pinned      bool   `json:"pinned"`
	Running     bool   `json:"-"`
	Minimized   bool   `json:"-"`
	PID         int    `json:"-"`
	Address     string `json:"-"` // Hyprland window address
	WorkspaceID int    `json:"-"` // Current workspace ID
	button      *gtk.Button
}

// PanelConfig holds persistent panel configuration (dock.json)
type PanelConfig struct {
	PinnedApps []DockItem `json:"pinned_apps"`
}

// RavenSettings matches raven-settings-menu config (settings.json)
type RavenSettings struct {
	PanelPosition string `json:"panel_position"` // "top", "bottom", "left", "right"
	PanelHeight   int    `json:"panel_height"`
	// Other settings we don't modify but need to preserve
	Theme                 string  `json:"theme,omitempty"`
	AccentColor           string  `json:"accent_color,omitempty"`
	FontSize              int     `json:"font_size,omitempty"`
	IconTheme             string  `json:"icon_theme,omitempty"`
	CursorTheme           string  `json:"cursor_theme,omitempty"`
	PanelOpacity          float64 `json:"panel_opacity,omitempty"`
	EnableAnimations      bool    `json:"enable_animations,omitempty"`
	WallpaperPath         string  `json:"wallpaper_path,omitempty"`
	WallpaperMode         string  `json:"wallpaper_mode,omitempty"`
	ShowDesktopIcons      bool    `json:"show_desktop_icons,omitempty"`
	ShowClock             bool    `json:"show_clock,omitempty"`
	ClockFormat           string  `json:"clock_format,omitempty"`
	ShowWorkspaces        bool    `json:"show_workspaces,omitempty"`
	BorderWidth           int     `json:"border_width,omitempty"`
	GapSize               int     `json:"gap_size,omitempty"`
	FocusFollowsMouse     bool    `json:"focus_follows_mouse,omitempty"`
	TitlebarButtons       string  `json:"titlebar_buttons,omitempty"`
	KeyboardLayout        string  `json:"keyboard_layout,omitempty"`
	MouseSpeed            float64 `json:"mouse_speed,omitempty"`
	TouchpadNaturalScroll bool    `json:"touchpad_natural_scroll,omitempty"`
	TouchpadTapToClick    bool    `json:"touchpad_tap_to_click,omitempty"`
	ScreenTimeout         int     `json:"screen_timeout,omitempty"`
	SuspendTimeout        int     `json:"suspend_timeout,omitempty"`
	LidCloseAction        string  `json:"lid_close_action,omitempty"`
	MasterVolume          int     `json:"master_volume,omitempty"`
	MuteOnLock            bool    `json:"mute_on_lock,omitempty"`
}

// RavenPanel represents the main panel/taskbar
type RavenPanel struct {
	app               *gtk.Application
	window            *gtk.Window
	clockLabel        *gtk.Label
	dockBox           *gtk.Box
	mainBox           *gtk.Box
	settingsBtn       *gtk.Button
	powerBtn          *gtk.Button
	startBtn          *gtk.Button
	dockItems         map[string]*DockItem
	mu                sync.RWMutex
	configPath        string // dock.json for pinned apps
	ravenSettingsPath string // raven settings.json for panel position
	settingsWindow    *gtk.Window
	powerWindow       *gtk.Window
	orientation       int
	ravenSettings     RavenSettings
}

func main() {
	// Use NonUnique flag to avoid D-Bus registration issues
	app := gtk.NewApplication("org.ravenlinux.shell", gio.ApplicationNonUnique)

	configDir, _ := os.UserConfigDir()
	configPath := filepath.Join(configDir, "raven-shell", "dock.json")
	ravenSettingsPath := filepath.Join(os.Getenv("HOME"), ".config", "raven", "settings.json")

	panel := &RavenPanel{
		app:               app,
		dockItems:         make(map[string]*DockItem),
		configPath:        configPath,
		ravenSettingsPath: ravenSettingsPath,
	}

	app.ConnectActivate(func() {
		panel.activate()
	})

	if code := app.Run(os.Args); code > 0 {
		os.Exit(code)
	}
}

func (p *RavenPanel) activate() {
	// Load config first to get orientation
	p.loadConfig()

	// Create the panel window
	p.window = gtk.NewWindow()
	p.window.SetTitle("Raven Panel")
	if p.orientation == OrientationLeft || p.orientation == OrientationRight {
		p.window.SetDefaultSize(panelSize, -1)
	} else {
		p.window.SetDefaultSize(-1, panelSize)
	}
	p.window.SetDecorated(false)

	// Initialize layer shell BEFORE window is realized
	p.initLayerShell()

	// Apply dark theme CSS
	p.applyCSS()

	// Create the panel content
	content := p.createPanelContent()
	p.window.SetChild(content)

	// Start clock update
	go p.updateClock()

	// Start process monitor to track running apps
	go p.monitorProcesses()

	p.window.SetApplication(p.app)
	p.window.Present()
}

func (p *RavenPanel) initLayerShell() {
	obj := p.window.Object
	if obj != nil {
		ptr := obj.Native()
		C.init_layer_shell_oriented((*C.GtkWidget)(unsafe.Pointer(ptr)), C.int(p.orientation))
	}
}

func (p *RavenPanel) applyCSS() {
	css := `
		window {
			background-color: rgba(30, 30, 30, 0.75);
		}
		
		.panel-container {
			background: linear-gradient(
				to bottom,
				rgba(255, 255, 255, 0.12) 0%,
				rgba(255, 255, 255, 0.05) 50%,
				rgba(0, 0, 0, 0.1) 100%
			);
			border-bottom: 1px solid rgba(255, 255, 255, 0.1);
			padding: 2px 8px;
		}
		
		.panel-section {
			background: rgba(255, 255, 255, 0.08);
			border-radius: 8px;
			padding: 3px 4px;
			margin: 2px 4px;
		}
		
		.dock-container {
			background: rgba(255, 255, 255, 0.1);
			border-radius: 10px;
			padding: 3px 6px;
			margin: 2px 8px;
			border: 1px solid rgba(255, 255, 255, 0.08);
			min-width: 50px;
		}
		
		button {
			background: transparent;
			border: none;
			border-radius: 6px;
			padding: 4px 14px;
			color: rgba(255, 255, 255, 0.9);
			font-size: 13px;
			font-weight: 500;
			min-height: 26px;
			min-width: 40px;
			transition: all 150ms ease;
		}
		
		button:hover {
			background: rgba(255, 255, 255, 0.15);
		}
		
		button:active {
			background: rgba(255, 255, 255, 0.25);
			transform: scale(0.97);
		}
		
		label {
			color: rgba(255, 255, 255, 0.9);
			font-size: 13px;
			font-weight: 500;
		}
		
		.start-button {
			background: linear-gradient(
				to bottom,
				rgba(0, 150, 136, 0.9) 0%,
				rgba(0, 130, 118, 0.9) 100%
			);
			font-weight: 600;
			padding: 4px 12px;
			border-radius: 6px;
			box-shadow: 0 1px 3px rgba(0, 0, 0, 0.3),
			            inset 0 1px 0 rgba(255, 255, 255, 0.2);
		}
		
		.start-button:hover {
			background: linear-gradient(
				to bottom,
				rgba(0, 170, 150, 0.95) 0%,
				rgba(0, 150, 136, 0.95) 100%
			);
		}
		
		.start-button:active {
			background: linear-gradient(
				to bottom,
				rgba(0, 130, 118, 0.95) 0%,
				rgba(0, 110, 100, 0.95) 100%
			);
		}

		.raven-icon {
			min-width: 24px;
			min-height: 24px;
		}
		
		.dock-item {
			background: rgba(255, 255, 255, 0.05);
			border-radius: 8px;
			min-width: 44px;
			padding: 4px 12px;
			margin: 0 2px;
		}
		
		.dock-item:hover {
			background: rgba(255, 255, 255, 0.18);
			box-shadow: 0 2px 8px rgba(0, 0, 0, 0.2);
		}
		
		.dock-item:active {
			background: rgba(255, 255, 255, 0.25);
		}
		
		.dock-item-running {
			border-bottom: 2px solid rgba(0, 150, 136, 0.9);
		}
		
		.dock-item-pinned {
			background: rgba(255, 255, 255, 0.08);
		}
		
		.dock-item-minimized {
			opacity: 0.6;
		}
		
		.clock {
			font-weight: 600;
			padding: 4px 14px;
			color: rgba(255, 255, 255, 0.95);
			font-size: 13px;
			letter-spacing: 0.3px;
		}
		
		.power-button {
			color: rgba(255, 120, 120, 0.9);
		}
		
		.power-button:hover {
			background: rgba(255, 100, 100, 0.2);
			color: rgba(255, 140, 140, 1);
		}
		
		separator {
			background-color: rgba(255, 255, 255, 0.15);
			min-width: 1px;
			margin: 6px 4px;
		}

		.context-menu {
			background: rgba(40, 40, 40, 0.95);
			border-radius: 8px;
			padding: 4px;
			border: 1px solid rgba(255, 255, 255, 0.1);
		}

		.context-menu button {
			border-radius: 4px;
			padding: 8px 16px;
			min-width: 120px;
		}

		.context-menu button:hover {
			background: rgba(255, 255, 255, 0.1);
		}

		.context-menu-close:hover {
			background: rgba(255, 80, 80, 0.3);
			color: rgba(255, 150, 150, 1);
		}

		.settings-button {
			color: rgba(200, 200, 200, 0.9);
		}

		.settings-button:hover {
			background: rgba(255, 255, 255, 0.15);
			color: rgba(255, 255, 255, 1);
		}

		.settings-menu {
			background: rgba(30, 30, 35, 0.95);
			border-radius: 12px;
			padding: 8px;
			border: 1px solid rgba(255, 255, 255, 0.1);
			min-width: 200px;
		}

		.settings-menu button {
			border-radius: 6px;
			padding: 10px 16px;
			min-width: 180px;
			margin: 2px 0;
		}

		.settings-menu button:hover {
			background: rgba(255, 255, 255, 0.1);
		}

		.settings-section-label {
			color: rgba(150, 150, 150, 0.8);
			font-size: 11px;
			font-weight: 600;
			padding: 8px 16px 4px 16px;
			text-transform: uppercase;
			letter-spacing: 0.5px;
		}

		.settings-menu-separator {
			background-color: rgba(255, 255, 255, 0.1);
			min-height: 1px;
			margin: 6px 8px;
		}

		.quick-toggle {
			background: rgba(255, 255, 255, 0.05);
			border-radius: 8px;
			padding: 8px 12px;
			margin: 4px 0;
		}

		.quick-toggle:hover {
			background: rgba(255, 255, 255, 0.12);
		}

		.quick-toggle-active {
			background: rgba(0, 150, 136, 0.3);
			border: 1px solid rgba(0, 150, 136, 0.5);
		}

		.quick-toggle-active:hover {
			background: rgba(0, 150, 136, 0.4);
		}
	`

	provider := gtk.NewCSSProvider()
	provider.LoadFromString(css)

	display := gdk.DisplayGetDefault()
	gtk.StyleContextAddProviderForDisplay(display, provider, gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)
}

func (p *RavenPanel) loadConfig() {
	// Ensure config directories exist
	configDir := filepath.Dir(p.configPath)
	os.MkdirAll(configDir, 0755)

	ravenConfigDir := filepath.Dir(p.ravenSettingsPath)
	os.MkdirAll(ravenConfigDir, 0755)

	// Load panel position from raven settings.json
	p.orientation = OrientationTop // Default
	p.ravenSettings = RavenSettings{
		PanelPosition: "top",
		PanelHeight:   38,
	}

	if data, err := os.ReadFile(p.ravenSettingsPath); err == nil {
		if err := json.Unmarshal(data, &p.ravenSettings); err == nil {
			switch p.ravenSettings.PanelPosition {
			case "top":
				p.orientation = OrientationTop
			case "bottom":
				p.orientation = OrientationBottom
			case "left":
				p.orientation = OrientationLeft
			case "right":
				p.orientation = OrientationRight
			default:
				p.orientation = OrientationTop
			}
		}
	}

	// Load pinned apps from dock.json
	data, err := os.ReadFile(p.configPath)
	if err != nil {
		// No dock config yet, start fresh
		return
	}

	var config PanelConfig
	if err := json.Unmarshal(data, &config); err != nil {
		return
	}

	p.mu.Lock()
	defer p.mu.Unlock()

	for _, item := range config.PinnedApps {
		item.Pinned = true
		item.Running = false
		itemCopy := item
		p.dockItems[item.ID] = &itemCopy
	}
}

func (p *RavenPanel) saveConfig() {
	p.mu.RLock()
	var pinnedApps []DockItem
	for _, item := range p.dockItems {
		if item.Pinned {
			pinnedApps = append(pinnedApps, DockItem{
				ID:      item.ID,
				Name:    item.Name,
				Command: item.Command,
				Icon:    item.Icon,
				Pinned:  true,
			})
		}
	}
	p.mu.RUnlock()

	config := PanelConfig{PinnedApps: pinnedApps}
	data, err := json.MarshalIndent(config, "", "  ")
	if err != nil {
		return
	}

	os.WriteFile(p.configPath, data, 0644)
}

// saveRavenSettings saves panel position to raven settings.json
func (p *RavenPanel) saveRavenSettings() {
	// Convert orientation to string
	switch p.orientation {
	case OrientationTop:
		p.ravenSettings.PanelPosition = "top"
	case OrientationBottom:
		p.ravenSettings.PanelPosition = "bottom"
	case OrientationLeft:
		p.ravenSettings.PanelPosition = "left"
	case OrientationRight:
		p.ravenSettings.PanelPosition = "right"
	}

	// Ensure directory exists
	ravenConfigDir := filepath.Dir(p.ravenSettingsPath)
	os.MkdirAll(ravenConfigDir, 0755)

	data, err := json.MarshalIndent(p.ravenSettings, "", "  ")
	if err != nil {
		return
	}

	os.WriteFile(p.ravenSettingsPath, data, 0644)
}

func (p *RavenPanel) setOrientation(orientation int) {
	if p.orientation == orientation {
		return
	}

	p.orientation = orientation
	p.saveRavenSettings()
	p.closeSettingsMenu()

	// Recreate the panel with new orientation
	p.rebuildPanel()
}

func (p *RavenPanel) rebuildPanel() {
	// Close any open menus
	p.closeSettingsMenu()
	p.closePowerMenu()

	// Destroy current window
	if p.window != nil {
		p.window.Close()
	}

	// Create new window with updated orientation
	p.window = gtk.NewWindow()
	p.window.SetTitle("Raven Panel")
	if p.orientation == OrientationLeft || p.orientation == OrientationRight {
		p.window.SetDefaultSize(panelSize, -1)
	} else {
		p.window.SetDefaultSize(-1, panelSize)
	}
	p.window.SetDecorated(false)

	// Initialize layer shell with new orientation
	p.initLayerShell()

	// Recreate panel content
	content := p.createPanelContent()
	p.window.SetChild(content)

	p.window.SetApplication(p.app)
	p.window.Present()
}

func (p *RavenPanel) createPanelContent() *gtk.Box {
	// Determine orientation for layout
	isVertical := p.orientation == OrientationLeft || p.orientation == OrientationRight

	var boxOrientation gtk.Orientation
	var separatorOrientation gtk.Orientation
	if isVertical {
		boxOrientation = gtk.OrientationVertical
		separatorOrientation = gtk.OrientationHorizontal
	} else {
		boxOrientation = gtk.OrientationHorizontal
		separatorOrientation = gtk.OrientationVertical
	}

	p.mainBox = gtk.NewBox(boxOrientation, 0)
	p.mainBox.SetHomogeneous(false)
	p.mainBox.AddCSSClass("panel-container")

	// Start section: Raven button with icon
	startBox := gtk.NewBox(boxOrientation, 6)
	if isVertical {
		startBox.SetMarginTop(6)
	} else {
		startBox.SetMarginStart(6)
	}
	startBox.AddCSSClass("panel-section")

	// Create raven button with icon
	p.startBtn = gtk.NewButton()
	ravenContent := p.createRavenIcon()
	p.startBtn.SetChild(ravenContent)
	p.startBtn.AddCSSClass("start-button")
	p.startBtn.SetTooltipText("Raven Menu")
	p.startBtn.ConnectClicked(func() {
		p.showMenu()
	})
	startBox.Append(p.startBtn)

	p.mainBox.Append(startBox)

	// First spacer
	spacer1 := gtk.NewBox(boxOrientation, 0)
	if isVertical {
		spacer1.SetVExpand(true)
	} else {
		spacer1.SetHExpand(true)
	}
	p.mainBox.Append(spacer1)

	// Center section: Dock
	p.dockBox = gtk.NewBox(boxOrientation, 4)
	p.dockBox.AddCSSClass("dock-container")

	// Render initial pinned apps
	p.renderDock()

	p.mainBox.Append(p.dockBox)

	// Second spacer
	spacer2 := gtk.NewBox(boxOrientation, 0)
	if isVertical {
		spacer2.SetVExpand(true)
	} else {
		spacer2.SetHExpand(true)
	}
	p.mainBox.Append(spacer2)

	// End section: Clock + Settings + Power
	endBox := gtk.NewBox(boxOrientation, 6)
	if isVertical {
		endBox.SetMarginBottom(6)
	} else {
		endBox.SetMarginEnd(6)
	}
	endBox.AddCSSClass("panel-section")

	p.clockLabel = gtk.NewLabel("")
	p.clockLabel.AddCSSClass("clock")
	p.updateClockLabel()
	endBox.Append(p.clockLabel)

	sep1 := gtk.NewSeparator(separatorOrientation)
	endBox.Append(sep1)

	// Settings button
	p.settingsBtn = gtk.NewButton()
	p.settingsBtn.SetLabel("Settings")
	p.settingsBtn.AddCSSClass("settings-button")
	p.settingsBtn.ConnectClicked(func() {
		p.showSettingsMenu()
	})
	endBox.Append(p.settingsBtn)

	sep2 := gtk.NewSeparator(separatorOrientation)
	endBox.Append(sep2)

	p.powerBtn = gtk.NewButton()
	p.powerBtn.SetLabel("Power")
	p.powerBtn.AddCSSClass("power-button")
	p.powerBtn.ConnectClicked(func() {
		p.showPowerMenu()
	})
	endBox.Append(p.powerBtn)

	p.mainBox.Append(endBox)

	return p.mainBox
}

func (p *RavenPanel) createRavenIcon() *gtk.Box {
	// Create a box with the raven icon
	box := gtk.NewBox(gtk.OrientationHorizontal, 0)

	// Write the SVG to a file and load it as a Picture
	configDir, _ := os.UserConfigDir()
	iconDir := filepath.Join(configDir, "raven-shell", "icons")
	os.MkdirAll(iconDir, 0755)
	iconPath := filepath.Join(iconDir, "raven.svg")

	// Write the SVG file
	os.WriteFile(iconPath, []byte(ravenIconSVG), 0644)

	// Create a Picture widget to display the SVG
	picture := gtk.NewPictureForFilename(iconPath)
	picture.SetContentFit(gtk.ContentFitContain)
	picture.SetCanShrink(true)
	picture.AddCSSClass("raven-icon")

	box.Append(picture)

	return box
}

func (p *RavenPanel) renderDock() {
	// Remove all existing children
	for {
		child := p.dockBox.FirstChild()
		if child == nil {
			break
		}
		p.dockBox.Remove(child)
	}

	p.mu.RLock()
	defer p.mu.RUnlock()

	// First render pinned apps, then running (non-pinned) apps
	for _, item := range p.dockItems {
		if item.Pinned {
			btn := p.createDockItem(item)
			item.button = btn
			p.dockBox.Append(btn)
		}
	}

	for _, item := range p.dockItems {
		if !item.Pinned && item.Running {
			btn := p.createDockItem(item)
			item.button = btn
			p.dockBox.Append(btn)
		}
	}
}

func (p *RavenPanel) createDockItem(item *DockItem) *gtk.Button {
	btn := gtk.NewButton()
	btn.SetLabel(item.Name)
	btn.AddCSSClass("dock-item")

	if item.Running {
		btn.AddCSSClass("dock-item-running")
	}
	if item.Pinned {
		btn.AddCSSClass("dock-item-pinned")
	}
	if item.Minimized {
		btn.AddCSSClass("dock-item-minimized")
	}

	// Left click: launch or focus
	btn.ConnectClicked(func() {
		if item.Running {
			p.focusApp(item)
		} else {
			p.launchApp(item.Command)
		}
	})

	// Right click: context menu
	rightClick := gtk.NewGestureClick()
	rightClick.SetButton(3) // Right mouse button
	rightClick.ConnectPressed(func(nPress int, x, y float64) {
		p.showDockItemMenu(item, btn)
	})
	btn.AddController(rightClick)

	return btn
}

func (p *RavenPanel) showDockItemMenu(item *DockItem, btn *gtk.Button) {
	popover := gtk.NewPopover()
	popover.SetParent(btn)
	popover.AddCSSClass("context-menu")

	menuBox := gtk.NewBox(gtk.OrientationVertical, 2)

	// Pin/Unpin button
	pinBtn := gtk.NewButton()
	if item.Pinned {
		pinBtn.SetLabel("Unpin from Dock")
	} else {
		pinBtn.SetLabel("Pin to Dock")
	}
	pinBtn.ConnectClicked(func() {
		p.togglePin(item)
		popover.Popdown()
	})
	menuBox.Append(pinBtn)

	// Only show these options for running apps
	if item.Running {
		// Minimize/Restore button
		minBtn := gtk.NewButton()
		if item.Minimized {
			minBtn.SetLabel("Restore")
		} else {
			minBtn.SetLabel("Minimize")
		}
		minBtn.ConnectClicked(func() {
			p.toggleMinimize(item)
			popover.Popdown()
		})
		menuBox.Append(minBtn)

		// Close button
		closeBtn := gtk.NewButton()
		closeBtn.SetLabel("Close")
		closeBtn.AddCSSClass("context-menu-close")
		closeBtn.ConnectClicked(func() {
			p.closeApp(item)
			popover.Popdown()
		})
		menuBox.Append(closeBtn)
	}

	popover.SetChild(menuBox)
	popover.Popup()
}

func (p *RavenPanel) togglePin(item *DockItem) {
	p.mu.Lock()
	item.Pinned = !item.Pinned

	// If unpinned and not running, remove from dock
	if !item.Pinned && !item.Running {
		delete(p.dockItems, item.ID)
	}
	p.mu.Unlock()

	p.saveConfig()

	// Update UI on main thread
	glib.IdleAdd(func() {
		p.renderDock()
	})
}

func (p *RavenPanel) toggleMinimize(item *DockItem) {
	p.mu.Lock()
	item.Minimized = !item.Minimized
	address := item.Address
	p.mu.Unlock()

	if address == "" {
		return
	}

	// Use Hyprland's special workspace for minimize functionality
	if item.Minimized {
		// Move window to special workspace (minimized)
		exec.Command("hyprctl", "dispatch", "movetoworkspacesilent", fmt.Sprintf("special:minimized,address:%s", address)).Start()
	} else {
		// Bring window back from special workspace and focus it
		exec.Command("hyprctl", "dispatch", "movetoworkspacesilent", fmt.Sprintf("e+0,address:%s", address)).Start()
		exec.Command("hyprctl", "dispatch", "focuswindow", fmt.Sprintf("address:%s", address)).Start()
	}

	glib.IdleAdd(func() {
		p.renderDock()
	})
}

func (p *RavenPanel) focusApp(item *DockItem) {
	p.mu.RLock()
	address := item.Address
	minimized := item.Minimized
	p.mu.RUnlock()

	if address == "" {
		return
	}

	if minimized {
		// Restore from special workspace first
		p.toggleMinimize(item)
	} else {
		// Focus the window using Hyprland
		exec.Command("hyprctl", "dispatch", "focuswindow", fmt.Sprintf("address:%s", address)).Start()
	}
}

func (p *RavenPanel) closeApp(item *DockItem) {
	p.mu.RLock()
	address := item.Address
	p.mu.RUnlock()

	if address != "" {
		// Close window using Hyprland
		exec.Command("hyprctl", "dispatch", "closewindow", fmt.Sprintf("address:%s", address)).Start()
	}
}

func (p *RavenPanel) launchApp(cmd string) {
	go func() {
		exec.Command("sh", "-c", cmd).Start()
	}()
}

// AddRunningApp adds a new running application to the dock
func (p *RavenPanel) AddRunningApp(id, name, command string, pid int) {
	p.mu.Lock()
	defer p.mu.Unlock()

	if existing, ok := p.dockItems[id]; ok {
		existing.Running = true
		existing.PID = pid
		existing.Minimized = false
	} else {
		p.dockItems[id] = &DockItem{
			ID:      id,
			Name:    name,
			Command: command,
			Running: true,
			PID:     pid,
		}
	}

	glib.IdleAdd(func() {
		p.renderDock()
	})
}

// RemoveRunningApp removes a running application from the dock
func (p *RavenPanel) RemoveRunningApp(id string) {
	p.mu.Lock()
	defer p.mu.Unlock()

	if item, ok := p.dockItems[id]; ok {
		item.Running = false
		item.PID = 0
		item.Minimized = false

		// Remove if not pinned
		if !item.Pinned {
			delete(p.dockItems, id)
		}
	}

	glib.IdleAdd(func() {
		p.renderDock()
	})
}

// getHyprlandClients fetches the current window list from Hyprland
func getHyprlandClients() ([]HyprlandClient, error) {
	output, err := exec.Command("hyprctl", "clients", "-j").Output()
	if err != nil {
		return nil, err
	}

	var clients []HyprlandClient
	if err := json.Unmarshal(output, &clients); err != nil {
		return nil, err
	}

	return clients, nil
}

// monitorWindows monitors Hyprland windows and updates the dock
func (p *RavenPanel) monitorProcesses() {
	// Known application class mappings (window class -> display info)
	knownApps := map[string]struct {
		name    string
		command string
		icon    string
	}{
		"raven-terminal":     {"Terminal", "raven-terminal", "utilities-terminal"},
		"raven-wifi":         {"WiFi", "raven-wifi", "network-wireless"},
		"raven-menu":         {"Menu", "raven-menu", "application-menu"},
		"raven-settings":     {"Settings", "raven-settings", "preferences-system"},
		"raven-files":        {"Files", "raven-files", "system-file-manager"},
		"raven-editor":       {"Editor", "raven-editor", "text-editor"},
		"raven-launcher":     {"Launcher", "raven-launcher", "system-search"},
		"raven-installer":    {"Installer", "raven-installer", "system-software-install"},
		"kitty":              {"Terminal", "kitty", "utilities-terminal"},
		"Alacritty":          {"Terminal", "alacritty", "utilities-terminal"},
		"foot":               {"Terminal", "foot", "utilities-terminal"},
		"firefox":            {"Firefox", "firefox", "firefox"},
		"chromium":           {"Chromium", "chromium", "chromium"},
		"org.gnome.Nautilus": {"Files", "nautilus", "system-file-manager"},
		"thunar":             {"Files", "thunar", "system-file-manager"},
		"code":               {"VS Code", "code", "visual-studio-code"},
		"Code":               {"VS Code", "code", "visual-studio-code"},
	}

	// Classes to exclude from dock (shell components)
	excludeClasses := map[string]bool{
		"raven-shell":   true,
		"raven-desktop": true,
		"raven-panel":   true,
		"":              true, // Exclude windows with no class
	}

	trackedAddresses := make(map[string]string) // Address -> dock item ID

	ticker := time.NewTicker(1500 * time.Millisecond) // Reduced polling for better performance under software rendering
	for range ticker.C {
		clients, err := getHyprlandClients()
		if err != nil {
			// Hyprland not running or hyprctl failed
			continue
		}

		currentAddresses := make(map[string]bool)

		for _, client := range clients {
			// Skip excluded classes
			if excludeClasses[client.Class] || excludeClasses[client.InitialClass] {
				continue
			}

			// Skip unmapped/hidden windows (but not special workspace windows)
			if !client.Mapped {
				continue
			}

			currentAddresses[client.Address] = true

			// Check if already tracked
			if existingID, tracked := trackedAddresses[client.Address]; tracked {
				// Update minimized state based on workspace
				p.mu.Lock()
				if item, ok := p.dockItems[existingID]; ok {
					// Check if in special workspace (minimized)
					item.Minimized = strings.HasPrefix(client.Workspace.Name, "special:")
					item.WorkspaceID = client.Workspace.ID
				}
				p.mu.Unlock()
				continue
			}

			// Determine display name and command
			class := client.Class
			if class == "" {
				class = client.InitialClass
			}

			var displayName, command, icon string
			if appInfo, ok := knownApps[class]; ok {
				displayName = appInfo.name
				command = appInfo.command
				icon = appInfo.icon
			} else {
				// Use window title or class as display name
				displayName = client.Title
				if displayName == "" {
					displayName = class
				}
				// Truncate long titles
				if len(displayName) > 20 {
					displayName = displayName[:17] + "..."
				}
				command = class
				icon = "application-x-executable"
			}

			// Create unique ID using address
			itemID := fmt.Sprintf("hypr-%s", client.Address)
			trackedAddresses[client.Address] = itemID

			// Check if minimized (in special workspace)
			minimized := strings.HasPrefix(client.Workspace.Name, "special:")

			p.addHyprlandWindow(itemID, displayName, command, icon, client.PID, client.Address, client.Workspace.ID, minimized)
		}

		// Remove windows that are no longer present
		for addr, itemID := range trackedAddresses {
			if !currentAddresses[addr] {
				p.RemoveRunningApp(itemID)
				delete(trackedAddresses, addr)
			}
		}
	}
}

// addHyprlandWindow adds a Hyprland window to the dock
func (p *RavenPanel) addHyprlandWindow(id, name, command, icon string, pid int, address string, workspaceID int, minimized bool) {
	p.mu.Lock()
	defer p.mu.Unlock()

	if existing, ok := p.dockItems[id]; ok {
		existing.Running = true
		existing.PID = pid
		existing.Address = address
		existing.WorkspaceID = workspaceID
		existing.Minimized = minimized
	} else {
		p.dockItems[id] = &DockItem{
			ID:          id,
			Name:        name,
			Command:     command,
			Icon:        icon,
			Running:     true,
			PID:         pid,
			Address:     address,
			WorkspaceID: workspaceID,
			Minimized:   minimized,
		}
	}

	glib.IdleAdd(func() {
		p.renderDock()
	})
}

func (p *RavenPanel) showMenu() {
	p.launchApp("raven-menu")
}

func (p *RavenPanel) closePowerMenu() {
	if p.powerWindow != nil {
		p.powerWindow.Close()
		p.powerWindow = nil
	}
}

func (p *RavenPanel) showPowerMenu() {
	// Toggle - if already open, close it
	if p.powerWindow != nil {
		p.closePowerMenu()
		return
	}

	// Close settings menu if open
	if p.settingsWindow != nil {
		p.closeSettingsMenu()
	}

	// Create a new layer-shell window for the menu
	p.powerWindow = gtk.NewWindow()
	p.powerWindow.SetTitle("Power")
	p.powerWindow.SetDecorated(false)
	p.powerWindow.SetDefaultSize(160, -1)

	// Initialize as layer shell overlay
	obj := p.powerWindow.Object
	if obj != nil {
		ptr := obj.Native()
		C.init_popup_layer_shell_oriented((*C.GtkWidget)(unsafe.Pointer(ptr)), C.int(p.orientation), C.int(panelSize+4), C.int(10))
	}

	menuBox := gtk.NewBox(gtk.OrientationVertical, 4)
	menuBox.AddCSSClass("settings-menu")
	menuBox.SetMarginTop(8)
	menuBox.SetMarginBottom(8)
	menuBox.SetMarginStart(8)
	menuBox.SetMarginEnd(8)

	// Logout button - exits Hyprland
	logoutBtn := gtk.NewButton()
	logoutBtn.SetLabel("Logout")
	logoutBtn.ConnectClicked(func() {
		p.closePowerMenu()
		exec.Command("hyprctl", "dispatch", "exit").Start()
	})
	menuBox.Append(logoutBtn)

	// Lock button
	lockBtn := gtk.NewButton()
	lockBtn.SetLabel("Lock Screen")
	lockBtn.ConnectClicked(func() {
		p.closePowerMenu()
		exec.Command("sh", "-c", "swaylock || hyprlock || loginctl lock-session").Start()
	})
	menuBox.Append(lockBtn)

	// Reboot button
	rebootBtn := gtk.NewButton()
	rebootBtn.SetLabel("Reboot")
	rebootBtn.ConnectClicked(func() {
		p.closePowerMenu()
		exec.Command("sh", "-c", "systemctl reboot || reboot || raven-powerctl reboot").Start()
	})
	menuBox.Append(rebootBtn)

	// Shutdown button
	shutdownBtn := gtk.NewButton()
	shutdownBtn.SetLabel("Shutdown")
	shutdownBtn.AddCSSClass("context-menu-close")
	shutdownBtn.ConnectClicked(func() {
		p.closePowerMenu()
		exec.Command("sh", "-c", "systemctl poweroff || poweroff || raven-powerctl poweroff").Start()
	})
	menuBox.Append(shutdownBtn)

	p.powerWindow.SetChild(menuBox)

	// Close on Escape key
	keyController := gtk.NewEventControllerKey()
	keyController.ConnectKeyPressed(func(keyval, keycode uint, state gdk.ModifierType) bool {
		if keyval == gdk.KEY_Escape {
			p.closePowerMenu()
			return true
		}
		return false
	})
	p.powerWindow.AddController(keyController)

	// Close when clicking outside (focus lost)
	focusController := gtk.NewEventControllerFocus()
	focusController.ConnectLeave(func() {
		glib.TimeoutAdd(100, func() bool {
			p.closePowerMenu()
			return false
		})
	})
	p.powerWindow.AddController(focusController)

	p.powerWindow.SetApplication(p.app)
	p.powerWindow.Present()
}

func (p *RavenPanel) closeSettingsMenu() {
	if p.settingsWindow != nil {
		p.settingsWindow.Close()
		p.settingsWindow = nil
	}
}

func (p *RavenPanel) showSettingsMenu() {
	// Toggle - if already open, close it
	if p.settingsWindow != nil {
		p.closeSettingsMenu()
		return
	}

	// Close power menu if open
	if p.powerWindow != nil {
		p.closePowerMenu()
	}

	// Create a new layer-shell window for the menu
	p.settingsWindow = gtk.NewWindow()
	p.settingsWindow.SetTitle("Settings")
	p.settingsWindow.SetDecorated(false)
	p.settingsWindow.SetDefaultSize(220, -1)

	// Initialize as layer shell overlay
	obj := p.settingsWindow.Object
	if obj != nil {
		ptr := obj.Native()
		C.init_popup_layer_shell_oriented((*C.GtkWidget)(unsafe.Pointer(ptr)), C.int(p.orientation), C.int(panelSize+4), C.int(100))
	}

	menuBox := gtk.NewBox(gtk.OrientationVertical, 0)
	menuBox.AddCSSClass("settings-menu")
	menuBox.SetMarginTop(8)
	menuBox.SetMarginBottom(8)
	menuBox.SetMarginStart(8)
	menuBox.SetMarginEnd(8)

	// Quick Toggles Section
	quickLabel := gtk.NewLabel("Quick Settings")
	quickLabel.AddCSSClass("settings-section-label")
	quickLabel.SetHAlign(gtk.AlignStart)
	menuBox.Append(quickLabel)

	// WiFi toggle
	wifiBtn := gtk.NewButton()
	wifiBtn.SetLabel("WiFi")
	wifiBtn.AddCSSClass("quick-toggle")
	wifiBtn.ConnectClicked(func() {
		p.closeSettingsMenu()
		p.launchApp("raven-wifi")
	})
	menuBox.Append(wifiBtn)

	// Bluetooth toggle
	bluetoothBtn := gtk.NewButton()
	bluetoothBtn.SetLabel("Bluetooth")
	bluetoothBtn.AddCSSClass("quick-toggle")
	bluetoothBtn.ConnectClicked(func() {
		p.closeSettingsMenu()
		p.launchApp("blueman-manager || blueberry || gnome-bluetooth-panel")
	})
	menuBox.Append(bluetoothBtn)

	// Volume/Sound
	soundBtn := gtk.NewButton()
	soundBtn.SetLabel("Sound")
	soundBtn.AddCSSClass("quick-toggle")
	soundBtn.ConnectClicked(func() {
		p.closeSettingsMenu()
		p.launchApp("pavucontrol || gnome-control-center sound || pwvucontrol")
	})
	menuBox.Append(soundBtn)

	// Separator
	sep1 := gtk.NewSeparator(gtk.OrientationHorizontal)
	sep1.AddCSSClass("settings-menu-separator")
	menuBox.Append(sep1)

	// System Settings Section
	sysLabel := gtk.NewLabel("System")
	sysLabel.AddCSSClass("settings-section-label")
	sysLabel.SetHAlign(gtk.AlignStart)
	menuBox.Append(sysLabel)

	// Display settings
	displayBtn := gtk.NewButton()
	displayBtn.SetLabel("Display")
	displayBtn.ConnectClicked(func() {
		p.closeSettingsMenu()
		p.launchApp("wdisplays || nwg-displays || gnome-control-center display")
	})
	menuBox.Append(displayBtn)

	// Network settings
	networkBtn := gtk.NewButton()
	networkBtn.SetLabel("Network")
	networkBtn.ConnectClicked(func() {
		p.closeSettingsMenu()
		p.launchApp("nm-connection-editor || gnome-control-center network || raven-wifi")
	})
	menuBox.Append(networkBtn)

	// Power/Battery settings
	powerSettingsBtn := gtk.NewButton()
	powerSettingsBtn.SetLabel("Power & Battery")
	powerSettingsBtn.ConnectClicked(func() {
		p.closeSettingsMenu()
		p.launchApp("gnome-control-center power || xfce4-power-manager-settings")
	})
	menuBox.Append(powerSettingsBtn)

	// Keyboard settings
	keyboardBtn := gtk.NewButton()
	keyboardBtn.SetLabel("Keyboard")
	keyboardBtn.ConnectClicked(func() {
		p.closeSettingsMenu()
		p.launchApp("gnome-control-center keyboard || fcitx5-configtool")
	})
	menuBox.Append(keyboardBtn)

	// Separator
	sep2 := gtk.NewSeparator(gtk.OrientationHorizontal)
	sep2.AddCSSClass("settings-menu-separator")
	menuBox.Append(sep2)

	// Appearance Section
	appearLabel := gtk.NewLabel("Appearance")
	appearLabel.AddCSSClass("settings-section-label")
	appearLabel.SetHAlign(gtk.AlignStart)
	menuBox.Append(appearLabel)

	// Theme/Appearance
	themeBtn := gtk.NewButton()
	themeBtn.SetLabel("Theme & Appearance")
	themeBtn.ConnectClicked(func() {
		p.closeSettingsMenu()
		p.launchApp("nwg-look || lxappearance || gnome-control-center appearance")
	})
	menuBox.Append(themeBtn)

	// Wallpaper
	wallpaperBtn := gtk.NewButton()
	wallpaperBtn.SetLabel("Wallpaper")
	wallpaperBtn.ConnectClicked(func() {
		p.closeSettingsMenu()
		p.launchApp("waypaper || nitrogen || gnome-control-center background")
	})
	menuBox.Append(wallpaperBtn)

	// Separator
	sep3 := gtk.NewSeparator(gtk.OrientationHorizontal)
	sep3.AddCSSClass("settings-menu-separator")
	menuBox.Append(sep3)

	// Panel Position Section
	panelLabel := gtk.NewLabel("Panel Position")
	panelLabel.AddCSSClass("settings-section-label")
	panelLabel.SetHAlign(gtk.AlignStart)
	menuBox.Append(panelLabel)

	// Position buttons in a grid-like arrangement
	positionBox := gtk.NewBox(gtk.OrientationVertical, 4)

	topBottomBox := gtk.NewBox(gtk.OrientationHorizontal, 4)
	topBottomBox.SetHomogeneous(true)

	topBtn := gtk.NewButton()
	topBtn.SetLabel("Top")
	if p.orientation == OrientationTop {
		topBtn.AddCSSClass("quick-toggle-active")
	}
	topBtn.ConnectClicked(func() {
		p.setOrientation(OrientationTop)
	})
	topBottomBox.Append(topBtn)

	bottomBtn := gtk.NewButton()
	bottomBtn.SetLabel("Bottom")
	if p.orientation == OrientationBottom {
		bottomBtn.AddCSSClass("quick-toggle-active")
	}
	bottomBtn.ConnectClicked(func() {
		p.setOrientation(OrientationBottom)
	})
	topBottomBox.Append(bottomBtn)

	positionBox.Append(topBottomBox)

	leftRightBox := gtk.NewBox(gtk.OrientationHorizontal, 4)
	leftRightBox.SetHomogeneous(true)

	leftBtn := gtk.NewButton()
	leftBtn.SetLabel("Left")
	if p.orientation == OrientationLeft {
		leftBtn.AddCSSClass("quick-toggle-active")
	}
	leftBtn.ConnectClicked(func() {
		p.setOrientation(OrientationLeft)
	})
	leftRightBox.Append(leftBtn)

	rightBtn := gtk.NewButton()
	rightBtn.SetLabel("Right")
	if p.orientation == OrientationRight {
		rightBtn.AddCSSClass("quick-toggle-active")
	}
	rightBtn.ConnectClicked(func() {
		p.setOrientation(OrientationRight)
	})
	leftRightBox.Append(rightBtn)

	positionBox.Append(leftRightBox)
	menuBox.Append(positionBox)

	// Separator
	sep4 := gtk.NewSeparator(gtk.OrientationHorizontal)
	sep4.AddCSSClass("settings-menu-separator")
	menuBox.Append(sep4)

	// All Settings button
	allSettingsBtn := gtk.NewButton()
	allSettingsBtn.SetLabel("All Settings...")
	allSettingsBtn.ConnectClicked(func() {
		p.closeSettingsMenu()
		p.launchApp("raven-settings || gnome-control-center || systemsettings5 || xfce4-settings-manager")
	})
	menuBox.Append(allSettingsBtn)

	p.settingsWindow.SetChild(menuBox)

	// Close on Escape key
	keyController := gtk.NewEventControllerKey()
	keyController.ConnectKeyPressed(func(keyval, keycode uint, state gdk.ModifierType) bool {
		if keyval == gdk.KEY_Escape {
			p.closeSettingsMenu()
			return true
		}
		return false
	})
	p.settingsWindow.AddController(keyController)

	// Close when clicking outside (focus lost)
	focusController := gtk.NewEventControllerFocus()
	focusController.ConnectLeave(func() {
		// Small delay to allow button clicks to register
		glib.TimeoutAdd(100, func() bool {
			p.closeSettingsMenu()
			return false
		})
	})
	p.settingsWindow.AddController(focusController)

	p.settingsWindow.SetApplication(p.app)
	p.settingsWindow.Present()
}

func (p *RavenPanel) updateClock() {
	ticker := time.NewTicker(time.Second)
	for range ticker.C {
		if p.clockLabel != nil {
			glib.IdleAdd(func() {
				p.updateClockLabel()
			})
		}
	}
}

func (p *RavenPanel) updateClockLabel() {
	now := time.Now()
	p.clockLabel.SetText(now.Format("Mon Jan 2  3:04 PM"))
}
