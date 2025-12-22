package main

import (
	"encoding/json"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
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

void init_settings_layer_shell(GtkWidget *window) {
    gtk_layer_init_for_window(GTK_WINDOW(window));
    gtk_layer_set_layer(GTK_WINDOW(window), GTK_LAYER_SHELL_LAYER_OVERLAY);
    gtk_layer_set_keyboard_mode(GTK_WINDOW(window), GTK_LAYER_SHELL_KEYBOARD_MODE_EXCLUSIVE);
}
*/
import "C"

// SettingsCategory represents a settings category
type SettingsCategory struct {
	Name        string
	Icon        string
	Description string
}

// RavenSettings holds the application settings
type RavenSettings struct {
	// Appearance
	Theme            string  `json:"theme"`
	AccentColor      string  `json:"accent_color"`
	FontSize         int     `json:"font_size"`
	IconTheme        string  `json:"icon_theme"`
	CursorTheme      string  `json:"cursor_theme"`
	PanelOpacity     float64 `json:"panel_opacity"`
	EnableAnimations bool    `json:"enable_animations"`

	// Desktop
	WallpaperPath    string `json:"wallpaper_path"`
	WallpaperMode    string `json:"wallpaper_mode"`
	ShowDesktopIcons bool   `json:"show_desktop_icons"`

	// Panel
	PanelPosition  string `json:"panel_position"`
	PanelHeight    int    `json:"panel_height"`
	ShowClock      bool   `json:"show_clock"`
	ClockFormat    string `json:"clock_format"`
	ShowWorkspaces bool   `json:"show_workspaces"`

	// Window
	BorderWidth       int    `json:"border_width"`
	GapSize           int    `json:"gap_size"`
	FocusFollowsMouse bool   `json:"focus_follows_mouse"`
	TitlebarButtons   string `json:"titlebar_buttons"`

	// Input
	KeyboardLayout        string  `json:"keyboard_layout"`
	MouseSpeed            float64 `json:"mouse_speed"`
	TouchpadNaturalScroll bool    `json:"touchpad_natural_scroll"`
	TouchpadTapToClick    bool    `json:"touchpad_tap_to_click"`

	// Power
	ScreenTimeout  int    `json:"screen_timeout"`
	SuspendTimeout int    `json:"suspend_timeout"`
	LidCloseAction string `json:"lid_close_action"`

	// Sound
	MasterVolume int  `json:"master_volume"`
	MuteOnLock   bool `json:"mute_on_lock"`
}

// RavenSettingsMenu is the settings application
type RavenSettingsMenu struct {
	app          *gtk.Application
	window       *gtk.Window
	categoryList *gtk.ListBox
	contentStack *gtk.Stack
	categories   []SettingsCategory
	settings     RavenSettings
	settingsPath string
}

func main() {
	app := gtk.NewApplication("org.ravenlinux.settings", gio.ApplicationFlagsNone)

	menu := &RavenSettingsMenu{
		app: app,
	}

	app.ConnectActivate(func() {
		menu.activate()
	})

	if code := app.Run(os.Args); code > 0 {
		os.Exit(code)
	}
}

func (m *RavenSettingsMenu) activate() {
	m.window = gtk.NewWindow()
	m.window.SetTitle("Raven Settings")
	m.window.SetDefaultSize(800, 600)
	m.window.SetDecorated(false)

	// Initialize categories
	m.initCategories()

	// Load settings
	m.loadSettings()

	// Apply CSS
	m.applyCSS()

	// Create UI
	content := m.createUI()
	m.window.SetChild(content)

	// Initialize layer shell
	m.initLayerShell()

	// Close on Escape
	keyController := gtk.NewEventControllerKey()
	keyController.ConnectKeyPressed(func(keyval, keycode uint, state gdk.ModifierType) bool {
		if keyval == gdk.KEY_Escape {
			m.window.Close()
			return true
		}
		return false
	})
	m.window.AddController(keyController)

	m.window.SetApplication(m.app)
	m.window.Present()
}

func (m *RavenSettingsMenu) initLayerShell() {
	obj := m.window.Object
	if obj != nil {
		ptr := obj.Native()
		C.init_settings_layer_shell((*C.GtkWidget)(unsafe.Pointer(ptr)))
	}
}

func (m *RavenSettingsMenu) initCategories() {
	m.categories = []SettingsCategory{
		{Name: "Appearance", Icon: "preferences-desktop-theme", Description: "Theme, colors, and fonts"},
		{Name: "Desktop", Icon: "preferences-desktop-wallpaper", Description: "Wallpaper and desktop icons"},
		{Name: "Panel", Icon: "preferences-desktop-display", Description: "Panel position and widgets"},
		{Name: "Windows", Icon: "preferences-system-windows", Description: "Window behavior and borders"},
		{Name: "Input", Icon: "input-keyboard", Description: "Keyboard and mouse settings"},
		{Name: "Power", Icon: "preferences-system-power", Description: "Power management options"},
		{Name: "Sound", Icon: "audio-volume-high", Description: "Audio settings"},
		{Name: "About", Icon: "help-about", Description: "System information"},
	}
}

func (m *RavenSettingsMenu) loadSettings() {
	configDir := filepath.Join(os.Getenv("HOME"), ".config", "raven")
	m.settingsPath = filepath.Join(configDir, "settings.json")

	// Default settings
	m.settings = RavenSettings{
		Theme:                 "dark",
		AccentColor:           "#009688",
		FontSize:              14,
		IconTheme:             "Papirus-Dark",
		CursorTheme:           "Adwaita",
		PanelOpacity:          0.95,
		EnableAnimations:      true,
		WallpaperPath:         "",
		WallpaperMode:         "fill",
		ShowDesktopIcons:      false,
		PanelPosition:         "top",
		PanelHeight:           36,
		ShowClock:             true,
		ClockFormat:           "24h",
		ShowWorkspaces:        true,
		BorderWidth:           2,
		GapSize:               8,
		FocusFollowsMouse:     false,
		TitlebarButtons:       "close,minimize,maximize",
		KeyboardLayout:        "us",
		MouseSpeed:            0.5,
		TouchpadNaturalScroll: true,
		TouchpadTapToClick:    true,
		ScreenTimeout:         300,
		SuspendTimeout:        900,
		LidCloseAction:        "suspend",
		MasterVolume:          80,
		MuteOnLock:            false,
	}

	// Try to load existing settings
	data, err := os.ReadFile(m.settingsPath)
	if err == nil {
		json.Unmarshal(data, &m.settings)
	}
}

func (m *RavenSettingsMenu) saveSettings() {
	configDir := filepath.Dir(m.settingsPath)
	os.MkdirAll(configDir, 0755)

	data, err := json.MarshalIndent(m.settings, "", "  ")
	if err == nil {
		os.WriteFile(m.settingsPath, data, 0644)
	}
}

func (m *RavenSettingsMenu) applyCSS() {
	css := `
		window {
			background-color: #0f1720;
		}
		.settings-header {
			background-color: #1a2332;
			padding: 16px 20px;
			border-bottom: 1px solid #333;
		}
		.settings-title {
			font-size: 20px;
			font-weight: bold;
			color: #00bfa5;
		}
		.settings-subtitle {
			font-size: 12px;
			color: #888;
		}
		.category-sidebar {
			background-color: #151d28;
			border-right: 1px solid #333;
		}
		.category-list {
			background-color: transparent;
		}
		.category-list row {
			padding: 12px 16px;
			border-radius: 0;
			margin: 0;
		}
		.category-list row:selected {
			background-color: #009688;
		}
		.category-list row:hover:not(:selected) {
			background-color: rgba(255, 255, 255, 0.05);
		}
		.category-name {
			color: #e0e0e0;
			font-size: 14px;
			font-weight: 500;
		}
		.category-desc {
			color: #666;
			font-size: 11px;
		}
		.content-area {
			background-color: #0f1720;
			padding: 20px;
		}
		.section-title {
			color: #00bfa5;
			font-size: 16px;
			font-weight: bold;
			margin-bottom: 16px;
		}
		.setting-row {
			background-color: #1a2332;
			border-radius: 8px;
			padding: 12px 16px;
			margin-bottom: 8px;
		}
		.setting-label {
			color: #e0e0e0;
			font-size: 14px;
		}
		.setting-description {
			color: #666;
			font-size: 11px;
		}
		.setting-control {
			min-width: 200px;
		}
		entry {
			background-color: #0f1720;
			border: 1px solid #333;
			border-radius: 6px;
			padding: 8px 12px;
			color: #e0e0e0;
		}
		entry:focus {
			border-color: #009688;
		}
		dropdown, combobox {
			background-color: #0f1720;
			border: 1px solid #333;
			border-radius: 6px;
			color: #e0e0e0;
		}
		dropdown button, combobox button {
			background-color: #1a2332;
			border: 1px solid #333;
			border-radius: 6px;
			padding: 8px 12px;
			color: #e0e0e0;
		}
		scale {
			padding: 8px 0;
		}
		scale trough {
			background-color: #333;
			border-radius: 4px;
			min-height: 6px;
		}
		scale highlight {
			background-color: #009688;
			border-radius: 4px;
		}
		scale slider {
			background-color: #e0e0e0;
			border-radius: 50%;
			min-width: 16px;
			min-height: 16px;
		}
		switch {
			background-color: #333;
			border-radius: 14px;
			min-width: 48px;
			min-height: 24px;
		}
		switch:checked {
			background-color: #009688;
		}
		switch slider {
			background-color: #e0e0e0;
			border-radius: 50%;
			min-width: 20px;
			min-height: 20px;
			margin: 2px;
		}
		button {
			background-color: #1a2332;
			border: 1px solid #333;
			border-radius: 6px;
			padding: 8px 16px;
			color: #e0e0e0;
		}
		button:hover {
			background-color: #252f3f;
		}
		button.primary {
			background-color: #009688;
			border-color: #009688;
		}
		button.primary:hover {
			background-color: #00796b;
		}
		button.destructive {
			background-color: #b71c1c;
			border-color: #b71c1c;
		}
		button.destructive:hover {
			background-color: #c62828;
		}
		.close-button {
			background-color: transparent;
			border: none;
			padding: 8px;
			color: #888;
		}
		.close-button:hover {
			color: #e0e0e0;
			background-color: rgba(255, 255, 255, 0.1);
		}
		.about-logo {
			font-size: 48px;
			color: #009688;
		}
		.about-title {
			font-size: 24px;
			font-weight: bold;
			color: #e0e0e0;
		}
		.about-version {
			font-size: 14px;
			color: #888;
		}
		.about-description {
			font-size: 13px;
			color: #aaa;
			margin: 16px 0;
		}
		spinbutton {
			background-color: #0f1720;
			border: 1px solid #333;
			border-radius: 6px;
			color: #e0e0e0;
		}
		spinbutton button {
			background-color: #1a2332;
			border: none;
			padding: 4px 8px;
		}
		spinbutton button:hover {
			background-color: #252f3f;
		}
		.color-button {
			min-width: 48px;
			min-height: 32px;
			border-radius: 6px;
			border: 2px solid #333;
		}
	`

	provider := gtk.NewCSSProvider()
	provider.LoadFromString(css)
	display := gdk.DisplayGetDefault()
	gtk.StyleContextAddProviderForDisplay(display, provider, gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)
}

func (m *RavenSettingsMenu) createUI() *gtk.Box {
	mainBox := gtk.NewBox(gtk.OrientationVertical, 0)

	// Header
	header := m.createHeader()
	mainBox.Append(header)

	// Content area (sidebar + settings)
	contentBox := gtk.NewBox(gtk.OrientationHorizontal, 0)
	contentBox.SetVExpand(true)

	// Category sidebar
	sidebar := m.createSidebar()
	contentBox.Append(sidebar)

	// Settings content stack
	m.contentStack = gtk.NewStack()
	m.contentStack.SetTransitionType(gtk.StackTransitionTypeCrossfade)
	m.contentStack.SetTransitionDuration(200)
	m.contentStack.AddCSSClass("content-area")
	m.contentStack.SetHExpand(true)

	// Create pages for each category
	m.contentStack.AddNamed(m.createAppearancePage(), "Appearance")
	m.contentStack.AddNamed(m.createDesktopPage(), "Desktop")
	m.contentStack.AddNamed(m.createPanelPage(), "Panel")
	m.contentStack.AddNamed(m.createWindowsPage(), "Windows")
	m.contentStack.AddNamed(m.createInputPage(), "Input")
	m.contentStack.AddNamed(m.createPowerPage(), "Power")
	m.contentStack.AddNamed(m.createSoundPage(), "Sound")
	m.contentStack.AddNamed(m.createAboutPage(), "About")

	contentBox.Append(m.contentStack)
	mainBox.Append(contentBox)

	// Select first category
	glib.IdleAdd(func() {
		if firstRow := m.categoryList.RowAtIndex(0); firstRow != nil {
			m.categoryList.SelectRow(firstRow)
		}
	})

	return mainBox
}

func (m *RavenSettingsMenu) createHeader() *gtk.Box {
	header := gtk.NewBox(gtk.OrientationHorizontal, 12)
	header.AddCSSClass("settings-header")

	// Title section
	titleBox := gtk.NewBox(gtk.OrientationVertical, 4)
	titleBox.SetHExpand(true)

	title := gtk.NewLabel("Raven Settings")
	title.AddCSSClass("settings-title")
	title.SetHAlign(gtk.AlignStart)
	titleBox.Append(title)

	subtitle := gtk.NewLabel("Configure your Raven desktop environment")
	subtitle.AddCSSClass("settings-subtitle")
	subtitle.SetHAlign(gtk.AlignStart)
	titleBox.Append(subtitle)

	header.Append(titleBox)

	// Close button
	closeBtn := gtk.NewButton()
	closeBtn.SetLabel("X")
	closeBtn.AddCSSClass("close-button")
	closeBtn.ConnectClicked(func() {
		m.window.Close()
	})
	header.Append(closeBtn)

	return header
}

func (m *RavenSettingsMenu) createSidebar() *gtk.Box {
	sidebar := gtk.NewBox(gtk.OrientationVertical, 0)
	sidebar.AddCSSClass("category-sidebar")
	sidebar.SetSizeRequest(220, -1)

	scroll := gtk.NewScrolledWindow()
	scroll.SetPolicy(gtk.PolicyNever, gtk.PolicyAutomatic)
	scroll.SetVExpand(true)

	m.categoryList = gtk.NewListBox()
	m.categoryList.AddCSSClass("category-list")
	m.categoryList.SetSelectionMode(gtk.SelectionSingle)
	m.categoryList.ConnectRowSelected(func(row *gtk.ListBoxRow) {
		if row != nil {
			idx := row.Index()
			if idx >= 0 && idx < len(m.categories) {
				m.contentStack.SetVisibleChildName(m.categories[idx].Name)
			}
		}
	})

	for _, cat := range m.categories {
		row := m.createCategoryRow(cat)
		m.categoryList.Append(row)
	}

	scroll.SetChild(m.categoryList)
	sidebar.Append(scroll)

	return sidebar
}

func (m *RavenSettingsMenu) createCategoryRow(cat SettingsCategory) *gtk.ListBoxRow {
	row := gtk.NewListBoxRow()

	box := gtk.NewBox(gtk.OrientationVertical, 4)
	box.SetMarginTop(4)
	box.SetMarginBottom(4)

	nameLabel := gtk.NewLabel(cat.Name)
	nameLabel.AddCSSClass("category-name")
	nameLabel.SetHAlign(gtk.AlignStart)
	box.Append(nameLabel)

	descLabel := gtk.NewLabel(cat.Description)
	descLabel.AddCSSClass("category-desc")
	descLabel.SetHAlign(gtk.AlignStart)
	box.Append(descLabel)

	row.SetChild(box)
	return row
}

func (m *RavenSettingsMenu) createSettingRow(title, description string, control gtk.Widgetter) *gtk.Box {
	row := gtk.NewBox(gtk.OrientationHorizontal, 12)
	row.AddCSSClass("setting-row")

	// Labels
	labelBox := gtk.NewBox(gtk.OrientationVertical, 4)
	labelBox.SetHExpand(true)

	titleLabel := gtk.NewLabel(title)
	titleLabel.AddCSSClass("setting-label")
	titleLabel.SetHAlign(gtk.AlignStart)
	labelBox.Append(titleLabel)

	if description != "" {
		descLabel := gtk.NewLabel(description)
		descLabel.AddCSSClass("setting-description")
		descLabel.SetHAlign(gtk.AlignStart)
		labelBox.Append(descLabel)
	}

	row.Append(labelBox)
	row.Append(control)

	return row
}

func (m *RavenSettingsMenu) createAppearancePage() *gtk.ScrolledWindow {
	scroll := gtk.NewScrolledWindow()
	scroll.SetPolicy(gtk.PolicyNever, gtk.PolicyAutomatic)

	content := gtk.NewBox(gtk.OrientationVertical, 16)
	content.SetMarginStart(20)
	content.SetMarginEnd(20)
	content.SetMarginTop(20)
	content.SetMarginBottom(20)

	// Section title
	sectionTitle := gtk.NewLabel("Appearance")
	sectionTitle.AddCSSClass("section-title")
	sectionTitle.SetHAlign(gtk.AlignStart)
	content.Append(sectionTitle)

	// Theme dropdown
	themes := []string{"dark", "light", "system"}
	themeDropdown := gtk.NewDropDown(gtk.NewStringList([]string{"Dark", "Light", "System"}), nil)
	switch m.settings.Theme {
	case "light":
		themeDropdown.SetSelected(1)
	case "system":
		themeDropdown.SetSelected(2)
	default:
		themeDropdown.SetSelected(0)
	}
	themeDropdown.Connect("notify::selected", func() {
		idx := themeDropdown.Selected()
		if idx < uint(len(themes)) {
			m.settings.Theme = themes[idx]
			m.saveSettings()
		}
	})
	content.Append(m.createSettingRow("Theme", "Choose your preferred color theme", themeDropdown))

	// Accent color
	accentColors := []string{"#009688", "#2196F3", "#9C27B0", "#FF5722", "#4CAF50", "#FFC107"}
	accentBox := gtk.NewBox(gtk.OrientationHorizontal, 8)
	for _, color := range accentColors {
		colorBtn := gtk.NewButton()
		colorBtn.SetSizeRequest(32, 32)
		colorBtn.AddCSSClass("color-button")
		// Apply inline style for background
		colorCSS := gtk.NewCSSProvider()
		colorCSS.LoadFromString(".color-btn-" + strings.TrimPrefix(color, "#") + " { background-color: " + color + "; }")
		colorBtn.AddCSSClass("color-btn-" + strings.TrimPrefix(color, "#"))
		display := gdk.DisplayGetDefault()
		gtk.StyleContextAddProviderForDisplay(display, colorCSS, gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)

		c := color // capture for closure
		colorBtn.ConnectClicked(func() {
			m.settings.AccentColor = c
			m.saveSettings()
		})
		accentBox.Append(colorBtn)
	}
	content.Append(m.createSettingRow("Accent Color", "Primary color for highlights and accents", accentBox))

	// Font size
	fontSizeAdj := gtk.NewAdjustment(float64(m.settings.FontSize), 10, 24, 1, 2, 0)
	fontSpin := gtk.NewSpinButton(fontSizeAdj, 1, 0)
	fontSpin.ConnectValueChanged(func() {
		m.settings.FontSize = int(fontSpin.Value())
		m.saveSettings()
	})
	content.Append(m.createSettingRow("Font Size", "Base font size in pixels", fontSpin))

	// Panel opacity
	opacityAdj := gtk.NewAdjustment(m.settings.PanelOpacity*100, 0, 100, 5, 10, 0)
	opacityScale := gtk.NewScale(gtk.OrientationHorizontal, opacityAdj)
	opacityScale.SetSizeRequest(200, -1)
	opacityScale.SetDrawValue(true)
	opacityScale.ConnectValueChanged(func() {
		m.settings.PanelOpacity = opacityScale.Value() / 100
		m.saveSettings()
	})
	content.Append(m.createSettingRow("Panel Opacity", "Transparency level for panels", opacityScale))

	// Animations toggle
	animSwitch := gtk.NewSwitch()
	animSwitch.SetActive(m.settings.EnableAnimations)
	animSwitch.ConnectStateSet(func(state bool) bool {
		m.settings.EnableAnimations = state
		m.saveSettings()
		return false
	})
	content.Append(m.createSettingRow("Enable Animations", "Smooth transitions and effects", animSwitch))

	scroll.SetChild(content)
	return scroll
}

func (m *RavenSettingsMenu) createDesktopPage() *gtk.ScrolledWindow {
	scroll := gtk.NewScrolledWindow()
	scroll.SetPolicy(gtk.PolicyNever, gtk.PolicyAutomatic)

	content := gtk.NewBox(gtk.OrientationVertical, 16)
	content.SetMarginStart(20)
	content.SetMarginEnd(20)
	content.SetMarginTop(20)
	content.SetMarginBottom(20)

	sectionTitle := gtk.NewLabel("Desktop")
	sectionTitle.AddCSSClass("section-title")
	sectionTitle.SetHAlign(gtk.AlignStart)
	content.Append(sectionTitle)

	// Wallpaper path with browse button
	wallpaperBox := gtk.NewBox(gtk.OrientationHorizontal, 8)
	wallpaperEntry := gtk.NewEntry()
	wallpaperEntry.SetText(m.settings.WallpaperPath)
	wallpaperEntry.SetHExpand(true)
	wallpaperEntry.ConnectChanged(func() {
		m.settings.WallpaperPath = wallpaperEntry.Text()
		m.saveSettings()
	})
	wallpaperBox.Append(wallpaperEntry)

	browseBtn := gtk.NewButton()
	browseBtn.SetLabel("Browse")
	browseBtn.ConnectClicked(func() {
		dialog := gtk.NewFileChooserNative(
			"Select Wallpaper",
			m.window,
			gtk.FileChooserActionOpen,
			"Select",
			"Cancel",
		)

		filter := gtk.NewFileFilter()
		filter.SetName("Images")
		filter.AddMIMEType("image/png")
		filter.AddMIMEType("image/jpeg")
		filter.AddMIMEType("image/webp")
		dialog.AddFilter(filter)

		dialog.ConnectResponse(func(response int) {
			if response == int(gtk.ResponseAccept) {
				file := dialog.File()
				if file != nil {
					path := file.Path()
					wallpaperEntry.SetText(path)
					m.settings.WallpaperPath = path
					m.saveSettings()
					m.applyWallpaper()
				}
			}
		})
		dialog.Show()
	})
	wallpaperBox.Append(browseBtn)
	content.Append(m.createSettingRow("Wallpaper", "Background image path", wallpaperBox))

	// Wallpaper mode
	modes := []string{"fill", "fit", "stretch", "center", "tile"}
	modeDropdown := gtk.NewDropDown(gtk.NewStringList([]string{"Fill", "Fit", "Stretch", "Center", "Tile"}), nil)
	for i, mode := range modes {
		if mode == m.settings.WallpaperMode {
			modeDropdown.SetSelected(uint(i))
			break
		}
	}
	modeDropdown.Connect("notify::selected", func() {
		idx := modeDropdown.Selected()
		if idx < uint(len(modes)) {
			m.settings.WallpaperMode = modes[idx]
			m.saveSettings()
			m.applyWallpaper()
		}
	})
	content.Append(m.createSettingRow("Wallpaper Mode", "How to display the wallpaper", modeDropdown))

	// Desktop icons toggle
	iconsSwitch := gtk.NewSwitch()
	iconsSwitch.SetActive(m.settings.ShowDesktopIcons)
	iconsSwitch.ConnectStateSet(func(state bool) bool {
		m.settings.ShowDesktopIcons = state
		m.saveSettings()
		return false
	})
	content.Append(m.createSettingRow("Show Desktop Icons", "Display icons on the desktop", iconsSwitch))

	scroll.SetChild(content)
	return scroll
}

func (m *RavenSettingsMenu) createPanelPage() *gtk.ScrolledWindow {
	scroll := gtk.NewScrolledWindow()
	scroll.SetPolicy(gtk.PolicyNever, gtk.PolicyAutomatic)

	content := gtk.NewBox(gtk.OrientationVertical, 16)
	content.SetMarginStart(20)
	content.SetMarginEnd(20)
	content.SetMarginTop(20)
	content.SetMarginBottom(20)

	sectionTitle := gtk.NewLabel("Panel")
	sectionTitle.AddCSSClass("section-title")
	sectionTitle.SetHAlign(gtk.AlignStart)
	content.Append(sectionTitle)

	// Panel position
	positions := []string{"top", "bottom", "left", "right"}
	posDropdown := gtk.NewDropDown(gtk.NewStringList([]string{"Top", "Bottom", "Left", "Right"}), nil)
	for i, pos := range positions {
		if pos == m.settings.PanelPosition {
			posDropdown.SetSelected(uint(i))
			break
		}
	}
	posDropdown.Connect("notify::selected", func() {
		idx := posDropdown.Selected()
		if idx < uint(len(positions)) {
			m.settings.PanelPosition = positions[idx]
			m.saveSettings()
		}
	})
	content.Append(m.createSettingRow("Panel Position", "Where to display the panel (requires restart)", posDropdown))

	// Panel height
	heightAdj := gtk.NewAdjustment(float64(m.settings.PanelHeight), 24, 64, 1, 4, 0)
	heightSpin := gtk.NewSpinButton(heightAdj, 1, 0)
	heightSpin.ConnectValueChanged(func() {
		m.settings.PanelHeight = int(heightSpin.Value())
		m.saveSettings()
	})
	content.Append(m.createSettingRow("Panel Height", "Height in pixels", heightSpin))

	// Show clock
	clockSwitch := gtk.NewSwitch()
	clockSwitch.SetActive(m.settings.ShowClock)
	clockSwitch.ConnectStateSet(func(state bool) bool {
		m.settings.ShowClock = state
		m.saveSettings()
		return false
	})
	content.Append(m.createSettingRow("Show Clock", "Display clock in the panel", clockSwitch))

	// Clock format
	formats := []string{"24h", "12h"}
	clockDropdown := gtk.NewDropDown(gtk.NewStringList([]string{"24-hour", "12-hour"}), nil)
	if m.settings.ClockFormat == "12h" {
		clockDropdown.SetSelected(1)
	}
	clockDropdown.Connect("notify::selected", func() {
		idx := clockDropdown.Selected()
		if idx < uint(len(formats)) {
			m.settings.ClockFormat = formats[idx]
			m.saveSettings()
		}
	})
	content.Append(m.createSettingRow("Clock Format", "Time display format", clockDropdown))

	// Show workspaces
	wsSwitch := gtk.NewSwitch()
	wsSwitch.SetActive(m.settings.ShowWorkspaces)
	wsSwitch.ConnectStateSet(func(state bool) bool {
		m.settings.ShowWorkspaces = state
		m.saveSettings()
		return false
	})
	content.Append(m.createSettingRow("Show Workspaces", "Display workspace indicator", wsSwitch))

	scroll.SetChild(content)
	return scroll
}

func (m *RavenSettingsMenu) createWindowsPage() *gtk.ScrolledWindow {
	scroll := gtk.NewScrolledWindow()
	scroll.SetPolicy(gtk.PolicyNever, gtk.PolicyAutomatic)

	content := gtk.NewBox(gtk.OrientationVertical, 16)
	content.SetMarginStart(20)
	content.SetMarginEnd(20)
	content.SetMarginTop(20)
	content.SetMarginBottom(20)

	sectionTitle := gtk.NewLabel("Windows")
	sectionTitle.AddCSSClass("section-title")
	sectionTitle.SetHAlign(gtk.AlignStart)
	content.Append(sectionTitle)

	// Border width
	borderAdj := gtk.NewAdjustment(float64(m.settings.BorderWidth), 0, 10, 1, 2, 0)
	borderSpin := gtk.NewSpinButton(borderAdj, 1, 0)
	borderSpin.ConnectValueChanged(func() {
		m.settings.BorderWidth = int(borderSpin.Value())
		m.saveSettings()
	})
	content.Append(m.createSettingRow("Border Width", "Window border thickness in pixels", borderSpin))

	// Gap size
	gapAdj := gtk.NewAdjustment(float64(m.settings.GapSize), 0, 32, 1, 4, 0)
	gapSpin := gtk.NewSpinButton(gapAdj, 1, 0)
	gapSpin.ConnectValueChanged(func() {
		m.settings.GapSize = int(gapSpin.Value())
		m.saveSettings()
	})
	content.Append(m.createSettingRow("Gap Size", "Space between windows in pixels", gapSpin))

	// Focus follows mouse
	focusSwitch := gtk.NewSwitch()
	focusSwitch.SetActive(m.settings.FocusFollowsMouse)
	focusSwitch.ConnectStateSet(func(state bool) bool {
		m.settings.FocusFollowsMouse = state
		m.saveSettings()
		return false
	})
	content.Append(m.createSettingRow("Focus Follows Mouse", "Focus window under cursor", focusSwitch))

	// Titlebar buttons
	buttonsEntry := gtk.NewEntry()
	buttonsEntry.SetText(m.settings.TitlebarButtons)
	buttonsEntry.ConnectChanged(func() {
		m.settings.TitlebarButtons = buttonsEntry.Text()
		m.saveSettings()
	})
	content.Append(m.createSettingRow("Titlebar Buttons", "Button order (close,minimize,maximize)", buttonsEntry))

	scroll.SetChild(content)
	return scroll
}

func (m *RavenSettingsMenu) createInputPage() *gtk.ScrolledWindow {
	scroll := gtk.NewScrolledWindow()
	scroll.SetPolicy(gtk.PolicyNever, gtk.PolicyAutomatic)

	content := gtk.NewBox(gtk.OrientationVertical, 16)
	content.SetMarginStart(20)
	content.SetMarginEnd(20)
	content.SetMarginTop(20)
	content.SetMarginBottom(20)

	sectionTitle := gtk.NewLabel("Input")
	sectionTitle.AddCSSClass("section-title")
	sectionTitle.SetHAlign(gtk.AlignStart)
	content.Append(sectionTitle)

	// Keyboard layout
	layouts := []string{"us", "gb", "de", "fr", "es", "it", "ru", "jp"}
	layoutDropdown := gtk.NewDropDown(gtk.NewStringList([]string{"US", "UK", "DE", "FR", "ES", "IT", "RU", "JP"}), nil)
	for i, layout := range layouts {
		if layout == m.settings.KeyboardLayout {
			layoutDropdown.SetSelected(uint(i))
			break
		}
	}
	layoutDropdown.Connect("notify::selected", func() {
		idx := layoutDropdown.Selected()
		if idx < uint(len(layouts)) {
			m.settings.KeyboardLayout = layouts[idx]
			m.saveSettings()
		}
	})
	content.Append(m.createSettingRow("Keyboard Layout", "Keyboard language layout", layoutDropdown))

	// Mouse speed
	speedAdj := gtk.NewAdjustment(m.settings.MouseSpeed*100, 0, 100, 5, 10, 0)
	speedScale := gtk.NewScale(gtk.OrientationHorizontal, speedAdj)
	speedScale.SetSizeRequest(200, -1)
	speedScale.SetDrawValue(true)
	speedScale.ConnectValueChanged(func() {
		m.settings.MouseSpeed = speedScale.Value() / 100
		m.saveSettings()
	})
	content.Append(m.createSettingRow("Mouse Speed", "Pointer acceleration", speedScale))

	// Touchpad natural scroll
	naturalSwitch := gtk.NewSwitch()
	naturalSwitch.SetActive(m.settings.TouchpadNaturalScroll)
	naturalSwitch.ConnectStateSet(func(state bool) bool {
		m.settings.TouchpadNaturalScroll = state
		m.saveSettings()
		return false
	})
	content.Append(m.createSettingRow("Natural Scrolling", "Reverse scroll direction on touchpad", naturalSwitch))

	// Tap to click
	tapSwitch := gtk.NewSwitch()
	tapSwitch.SetActive(m.settings.TouchpadTapToClick)
	tapSwitch.ConnectStateSet(func(state bool) bool {
		m.settings.TouchpadTapToClick = state
		m.saveSettings()
		return false
	})
	content.Append(m.createSettingRow("Tap to Click", "Tap touchpad to click", tapSwitch))

	scroll.SetChild(content)
	return scroll
}

func (m *RavenSettingsMenu) createPowerPage() *gtk.ScrolledWindow {
	scroll := gtk.NewScrolledWindow()
	scroll.SetPolicy(gtk.PolicyNever, gtk.PolicyAutomatic)

	content := gtk.NewBox(gtk.OrientationVertical, 16)
	content.SetMarginStart(20)
	content.SetMarginEnd(20)
	content.SetMarginTop(20)
	content.SetMarginBottom(20)

	sectionTitle := gtk.NewLabel("Power")
	sectionTitle.AddCSSClass("section-title")
	sectionTitle.SetHAlign(gtk.AlignStart)
	content.Append(sectionTitle)

	// Screen timeout
	screenTimeouts := []int{0, 60, 300, 600, 900, 1800}
	screenDropdown := gtk.NewDropDown(gtk.NewStringList([]string{"Never", "1 minute", "5 minutes", "10 minutes", "15 minutes", "30 minutes"}), nil)
	for i, timeout := range screenTimeouts {
		if timeout == m.settings.ScreenTimeout {
			screenDropdown.SetSelected(uint(i))
			break
		}
	}
	screenDropdown.Connect("notify::selected", func() {
		idx := screenDropdown.Selected()
		if idx < uint(len(screenTimeouts)) {
			m.settings.ScreenTimeout = screenTimeouts[idx]
			m.saveSettings()
		}
	})
	content.Append(m.createSettingRow("Screen Timeout", "Turn off display after inactivity", screenDropdown))

	// Suspend timeout
	suspendTimeouts := []int{0, 300, 900, 1800, 3600, 7200}
	suspendDropdown := gtk.NewDropDown(gtk.NewStringList([]string{"Never", "5 minutes", "15 minutes", "30 minutes", "1 hour", "2 hours"}), nil)
	for i, timeout := range suspendTimeouts {
		if timeout == m.settings.SuspendTimeout {
			suspendDropdown.SetSelected(uint(i))
			break
		}
	}
	suspendDropdown.Connect("notify::selected", func() {
		idx := suspendDropdown.Selected()
		if idx < uint(len(suspendTimeouts)) {
			m.settings.SuspendTimeout = suspendTimeouts[idx]
			m.saveSettings()
		}
	})
	content.Append(m.createSettingRow("Suspend Timeout", "Suspend system after inactivity", suspendDropdown))

	// Lid close action
	lidActions := []string{"suspend", "hibernate", "poweroff", "nothing"}
	lidDropdown := gtk.NewDropDown(gtk.NewStringList([]string{"Suspend", "Hibernate", "Power Off", "Do Nothing"}), nil)
	for i, action := range lidActions {
		if action == m.settings.LidCloseAction {
			lidDropdown.SetSelected(uint(i))
			break
		}
	}
	lidDropdown.Connect("notify::selected", func() {
		idx := lidDropdown.Selected()
		if idx < uint(len(lidActions)) {
			m.settings.LidCloseAction = lidActions[idx]
			m.saveSettings()
		}
	})
	content.Append(m.createSettingRow("Lid Close Action", "Action when laptop lid is closed", lidDropdown))

	scroll.SetChild(content)
	return scroll
}

func (m *RavenSettingsMenu) createSoundPage() *gtk.ScrolledWindow {
	scroll := gtk.NewScrolledWindow()
	scroll.SetPolicy(gtk.PolicyNever, gtk.PolicyAutomatic)

	content := gtk.NewBox(gtk.OrientationVertical, 16)
	content.SetMarginStart(20)
	content.SetMarginEnd(20)
	content.SetMarginTop(20)
	content.SetMarginBottom(20)

	sectionTitle := gtk.NewLabel("Sound")
	sectionTitle.AddCSSClass("section-title")
	sectionTitle.SetHAlign(gtk.AlignStart)
	content.Append(sectionTitle)

	// Master volume
	volumeAdj := gtk.NewAdjustment(float64(m.settings.MasterVolume), 0, 100, 5, 10, 0)
	volumeScale := gtk.NewScale(gtk.OrientationHorizontal, volumeAdj)
	volumeScale.SetSizeRequest(200, -1)
	volumeScale.SetDrawValue(true)
	volumeScale.ConnectValueChanged(func() {
		m.settings.MasterVolume = int(volumeScale.Value())
		m.saveSettings()
		// Apply volume immediately
		volume := strconv.Itoa(m.settings.MasterVolume)
		exec.Command("wpctl", "set-volume", "@DEFAULT_AUDIO_SINK@", volume+"%").Start()
	})
	content.Append(m.createSettingRow("Master Volume", "System audio volume", volumeScale))

	// Mute on lock
	muteSwitch := gtk.NewSwitch()
	muteSwitch.SetActive(m.settings.MuteOnLock)
	muteSwitch.ConnectStateSet(func(state bool) bool {
		m.settings.MuteOnLock = state
		m.saveSettings()
		return false
	})
	content.Append(m.createSettingRow("Mute on Lock", "Mute audio when screen is locked", muteSwitch))

	// Audio output test button
	testBtn := gtk.NewButton()
	testBtn.SetLabel("Test Audio")
	testBtn.ConnectClicked(func() {
		exec.Command("paplay", "/usr/share/sounds/freedesktop/stereo/bell.oga").Start()
	})
	content.Append(m.createSettingRow("Test Audio Output", "Play a test sound", testBtn))

	scroll.SetChild(content)
	return scroll
}

func (m *RavenSettingsMenu) createAboutPage() *gtk.ScrolledWindow {
	scroll := gtk.NewScrolledWindow()
	scroll.SetPolicy(gtk.PolicyNever, gtk.PolicyAutomatic)

	content := gtk.NewBox(gtk.OrientationVertical, 16)
	content.SetMarginStart(40)
	content.SetMarginEnd(40)
	content.SetMarginTop(40)
	content.SetMarginBottom(40)
	content.SetHAlign(gtk.AlignCenter)

	// Logo placeholder
	logo := gtk.NewLabel("RAVEN")
	logo.AddCSSClass("about-logo")
	content.Append(logo)

	// Title
	title := gtk.NewLabel("Raven Linux")
	title.AddCSSClass("about-title")
	title.SetMarginTop(16)
	content.Append(title)

	// Version
	version := gtk.NewLabel("Version 1.0.0")
	version.AddCSSClass("about-version")
	content.Append(version)

	// Description
	desc := gtk.NewLabel("A modern, lightweight Linux desktop environment\nbuilt with simplicity and performance in mind.")
	desc.AddCSSClass("about-description")
	desc.SetJustify(gtk.JustifyCenter)
	content.Append(desc)

	// System info section
	infoBox := gtk.NewBox(gtk.OrientationVertical, 8)
	infoBox.SetMarginTop(32)

	// Get system info
	hostname, _ := os.Hostname()
	kernel, _ := exec.Command("uname", "-r").Output()

	addInfoRow := func(label, value string) {
		row := gtk.NewBox(gtk.OrientationHorizontal, 12)
		row.SetHAlign(gtk.AlignCenter)

		labelWidget := gtk.NewLabel(label + ":")
		labelWidget.AddCSSClass("setting-description")
		row.Append(labelWidget)

		valueWidget := gtk.NewLabel(value)
		valueWidget.AddCSSClass("setting-label")
		row.Append(valueWidget)

		infoBox.Append(row)
	}

	addInfoRow("Hostname", hostname)
	addInfoRow("Kernel", strings.TrimSpace(string(kernel)))
	addInfoRow("Desktop", "Raven Shell")

	content.Append(infoBox)

	// Links section
	linksBox := gtk.NewBox(gtk.OrientationHorizontal, 16)
	linksBox.SetHAlign(gtk.AlignCenter)
	linksBox.SetMarginTop(32)

	websiteBtn := gtk.NewButton()
	websiteBtn.SetLabel("Website")
	websiteBtn.ConnectClicked(func() {
		exec.Command("xdg-open", "https://ravenlinux.org").Start()
	})
	linksBox.Append(websiteBtn)

	docsBtn := gtk.NewButton()
	docsBtn.SetLabel("Documentation")
	docsBtn.ConnectClicked(func() {
		exec.Command("xdg-open", "https://docs.ravenlinux.org").Start()
	})
	linksBox.Append(docsBtn)

	content.Append(linksBox)

	scroll.SetChild(content)
	return scroll
}

func (m *RavenSettingsMenu) applyWallpaper() {
	if m.settings.WallpaperPath == "" {
		return
	}
	// Use swaybg or similar tool to set wallpaper
	// Kill existing swaybg and start new one
	exec.Command("pkill", "swaybg").Run()
	mode := m.settings.WallpaperMode
	if mode == "" {
		mode = "fill"
	}
	exec.Command("swaybg", "-i", m.settings.WallpaperPath, "-m", mode).Start()
}
