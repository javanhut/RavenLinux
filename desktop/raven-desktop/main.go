package main

import (
	"encoding/json"
	"os"
	"os/exec"
	"os/signal"
	"path/filepath"
	"strings"
	"syscall"
	"unsafe"

	"raven-desktop/fuzzy"

	"github.com/diamondburned/gotk4/pkg/gdk/v4"
	"github.com/diamondburned/gotk4/pkg/gio/v2"
	"github.com/diamondburned/gotk4/pkg/glib/v2"
	"github.com/diamondburned/gotk4/pkg/gtk/v4"
)

/*
#cgo pkg-config: gtk4-layer-shell-0 gtk4
#include <gtk4-layer-shell.h>
#include <gtk/gtk.h>

void init_desktop_layer_shell(GtkWidget *window) {
    gtk_layer_init_for_window(GTK_WINDOW(window));
    gtk_layer_set_layer(GTK_WINDOW(window), GTK_LAYER_SHELL_LAYER_BACKGROUND);
    gtk_layer_set_anchor(GTK_WINDOW(window), GTK_LAYER_SHELL_EDGE_TOP, TRUE);
    gtk_layer_set_anchor(GTK_WINDOW(window), GTK_LAYER_SHELL_EDGE_BOTTOM, TRUE);
    gtk_layer_set_anchor(GTK_WINDOW(window), GTK_LAYER_SHELL_EDGE_LEFT, TRUE);
    gtk_layer_set_anchor(GTK_WINDOW(window), GTK_LAYER_SHELL_EDGE_RIGHT, TRUE);
    gtk_layer_set_exclusive_zone(GTK_WINDOW(window), -1);
}
*/
import "C"

// DesktopIcon represents an icon on the desktop
type DesktopIcon struct {
	Name string `json:"name"`
	Exec string `json:"exec"`
	Icon string `json:"icon"`
	X    int    `json:"x"`
	Y    int    `json:"y"`
}

// PinnedAppsConfig holds the list of pinned desktop apps
type PinnedAppsConfig struct {
	PinnedApps []DesktopIcon `json:"pinned_apps"`
}

// RavenSettings holds shared settings
type RavenSettings struct {
	WallpaperPath    string `json:"wallpaper_path"`
	WallpaperMode    string `json:"wallpaper_mode"`
	ShowDesktopIcons bool   `json:"show_desktop_icons"`
}

// RavenDesktop is the desktop background with icons
type RavenDesktop struct {
	app            *gtk.Application
	window         *gtk.Window
	iconGrid       *gtk.FlowBox
	overlay        *gtk.Overlay
	bgBox          *gtk.Box
	bgPicture      *gtk.Picture
	icons          []DesktopIcon
	popover        *gtk.PopoverMenu
	settings       RavenSettings
	settingsPath   string
	pinnedAppsPath string
	fuzzyFinder    *fuzzy.Finder
}

var previewMode bool

func main() {
	// Check for preview/debug mode via environment variable
	// Usage: RAVEN_PREVIEW=1 ./raven-desktop
	if os.Getenv("RAVEN_PREVIEW") == "1" {
		previewMode = true
	}

	app := gtk.NewApplication("org.ravenlinux.desktop", gio.ApplicationFlagsNone)

	desktop := &RavenDesktop{
		app: app,
	}

	app.ConnectActivate(func() {
		desktop.activate()
	})

	if code := app.Run(os.Args); code > 0 {
		os.Exit(code)
	}
}

func (d *RavenDesktop) activate() {
	// Load shared settings
	d.loadSettings()

	d.window = gtk.NewWindow()
	d.window.SetTitle("Raven Desktop")

	if previewMode {
		// Preview mode: run as normal window for testing
		d.window.SetDecorated(true)
		d.window.SetDefaultSize(1280, 720)
	} else {
		d.window.SetDecorated(false)
	}

	// Load desktop icons
	d.loadIcons()

	// Apply CSS
	d.applyCSS()

	// Create UI
	content := d.createUI()
	d.window.SetChild(content)

	// Initialize layer shell for background (skip in preview mode)
	if !previewMode {
		d.initLayerShell()
	}

	// Set up right-click menu
	d.setupContextMenu()

	// Set up signal handler for fuzzy finder shortcut
	d.setupSignalHandler()

	d.window.SetApplication(d.app)
	d.window.Present()
}

func (d *RavenDesktop) setupSignalHandler() {
	sigChan := make(chan os.Signal, 1)
	signal.Notify(sigChan, syscall.SIGUSR1, syscall.SIGUSR2)

	go func() {
		for sig := range sigChan {
			switch sig {
			case syscall.SIGUSR1:
				// Open fuzzy finder
				glib.IdleAdd(func() {
					d.showFuzzyFinder()
				})
			case syscall.SIGUSR2:
				// Open fuzzy finder in pin mode
				glib.IdleAdd(func() {
					d.showFuzzyFinderForPinning()
				})
			}
		}
	}()
}

func (d *RavenDesktop) loadSettings() {
	configDir := filepath.Join(os.Getenv("HOME"), ".config", "raven")
	d.settingsPath = filepath.Join(configDir, "settings.json")
	d.pinnedAppsPath = filepath.Join(configDir, "pinned-apps.json")

	// Ensure config directory exists
	os.MkdirAll(configDir, 0755)

	// Default settings
	d.settings = RavenSettings{
		WallpaperPath:    "",
		WallpaperMode:    "fill",
		ShowDesktopIcons: true,
	}

	data, err := os.ReadFile(d.settingsPath)
	if err == nil {
		json.Unmarshal(data, &d.settings)
	}
}

func (d *RavenDesktop) initLayerShell() {
	obj := d.window.Object
	if obj != nil {
		ptr := obj.Native()
		C.init_desktop_layer_shell((*C.GtkWidget)(unsafe.Pointer(ptr)))
	}
}

func (d *RavenDesktop) applyCSS() {
	css := `
		window {
			background-color: #0b0f14;
		}
		.desktop-bg {
			background-color: #0b0f14;
		}
		.desktop-icon {
			background-color: transparent;
			border: none;
			border-radius: 8px;
			padding: 8px;
			min-width: 80px;
			min-height: 90px;
		}
		.desktop-icon:hover {
			background-color: rgba(255, 255, 255, 0.1);
		}
		.desktop-icon:active {
			background-color: rgba(0, 150, 136, 0.3);
		}
		.desktop-icon:selected {
			background-color: rgba(0, 150, 136, 0.4);
		}
		.icon-label {
			color: #ffffff;
			font-size: 11px;
			text-shadow: 1px 1px 2px rgba(0, 0, 0, 0.8);
		}
		.icon-image {
			min-width: 48px;
			min-height: 48px;
		}
		popover {
			background-color: #1a2332;
			border: 1px solid #333;
		}
		popover modelbutton {
			padding: 8px 16px;
			color: #e0e0e0;
		}
		popover modelbutton:hover {
			background-color: rgba(255, 255, 255, 0.1);
		}
	`

	provider := gtk.NewCSSProvider()
	provider.LoadFromString(css)
	display := gdk.DisplayGetDefault()
	gtk.StyleContextAddProviderForDisplay(display, provider, gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)
}

func (d *RavenDesktop) createUI() *gtk.Overlay {
	d.overlay = gtk.NewOverlay()

	// Background
	d.bgBox = gtk.NewBox(gtk.OrientationVertical, 0)
	d.bgBox.AddCSSClass("desktop-bg")
	d.bgBox.SetHExpand(true)
	d.bgBox.SetVExpand(true)

	// Try to load wallpaper from settings first, then fallback
	wallpaperPaths := []string{}
	if d.settings.WallpaperPath != "" {
		wallpaperPaths = append(wallpaperPaths, d.settings.WallpaperPath)
	}
	wallpaperPaths = append(wallpaperPaths,
		"/usr/share/backgrounds/raven-wallpaper.png",
		"/usr/share/backgrounds/raven-sky.ppm",
		"/usr/share/backgrounds/default.png",
	)

	for _, path := range wallpaperPaths {
		if _, err := os.Stat(path); err == nil {
			d.bgPicture = gtk.NewPictureForFilename(path)
			d.bgPicture.SetContentFit(gtk.ContentFitCover)
			d.bgPicture.SetHExpand(true)
			d.bgPicture.SetVExpand(true)
			break
		}
	}

	if d.bgPicture != nil {
		d.bgBox.Append(d.bgPicture)
	}

	d.overlay.SetChild(d.bgBox)

	// Icon grid
	d.iconGrid = gtk.NewFlowBox()
	d.iconGrid.SetSelectionMode(gtk.SelectionSingle)
	d.iconGrid.SetActivateOnSingleClick(false) // Double-click to activate
	d.iconGrid.SetHomogeneous(true)
	d.iconGrid.SetRowSpacing(16)
	d.iconGrid.SetColumnSpacing(16)
	d.iconGrid.SetMarginStart(20)
	d.iconGrid.SetMarginEnd(20)
	d.iconGrid.SetMarginTop(50) // Leave space for panel
	d.iconGrid.SetMarginBottom(20)
	d.iconGrid.SetMaxChildrenPerLine(20)
	d.iconGrid.SetMinChildrenPerLine(1)
	d.iconGrid.SetVAlign(gtk.AlignStart)
	d.iconGrid.SetHAlign(gtk.AlignStart)
	d.iconGrid.SetCanFocus(true)

	// Add icons
	for i, icon := range d.icons {
		iconWidget := d.createIconWidget(icon, i)
		d.iconGrid.Append(iconWidget)
	}

	// Handle double-click
	d.iconGrid.ConnectChildActivated(func(child *gtk.FlowBoxChild) {
		idx := child.Index()
		if idx >= 0 && idx < len(d.icons) {
			d.launchApp(d.icons[idx].Exec)
		}
	})

	d.overlay.AddOverlay(d.iconGrid)

	return d.overlay
}

func (d *RavenDesktop) createIconWidget(icon DesktopIcon, index int) *gtk.Box {
	box := gtk.NewBox(gtk.OrientationVertical, 4)
	box.AddCSSClass("desktop-icon")
	box.SetHAlign(gtk.AlignCenter)
	box.SetFocusable(true)
	box.SetCanFocus(true)

	// Icon image
	var image *gtk.Image
	iconPaths := []string{
		"/usr/share/icons/hicolor/48x48/apps/" + icon.Icon + ".png",
		"/usr/share/icons/hicolor/48x48/apps/" + icon.Icon + ".svg",
		"/usr/share/pixmaps/" + icon.Icon + ".png",
		"/usr/share/icons/Adwaita/48x48/apps/" + icon.Icon + ".png",
	}

	found := false
	for _, path := range iconPaths {
		if _, err := os.Stat(path); err == nil {
			image = gtk.NewImageFromFile(path)
			found = true
			break
		}
	}

	if !found {
		// Use a themed icon or fallback
		image = gtk.NewImageFromIconName(icon.Icon)
	}

	image.SetPixelSize(48)
	image.AddCSSClass("icon-image")
	box.Append(image)

	// Icon label
	label := gtk.NewLabel(icon.Name)
	label.AddCSSClass("icon-label")
	label.SetEllipsize(3) // PANGO_ELLIPSIZE_END
	label.SetMaxWidthChars(12)
	label.SetLines(2)
	label.SetWrap(true)
	label.SetJustify(gtk.JustifyCenter)
	box.Append(label)

	// Add left-click handler for selection
	iconIdx := index
	leftClick := gtk.NewGestureClick()
	leftClick.SetButton(1) // Left click
	leftClick.ConnectPressed(func(nPress int, x, y float64) {
		// Select this icon in the FlowBox
		child := d.iconGrid.ChildAtIndex(iconIdx)
		if child != nil {
			d.iconGrid.SelectChild(child)
		}
		// Double-click to launch
		if nPress == 2 {
			d.launchApp(d.icons[iconIdx].Exec)
		}
	})
	box.AddController(leftClick)

	// Add right-click menu for unpinning
	iconName := icon.Name // Capture for closure
	rightClick := gtk.NewGestureClick()
	rightClick.SetButton(3) // Right click
	rightClick.ConnectPressed(func(nPress int, x, y float64) {
		d.showIconContextMenu(box, iconName, x, y)
	})
	box.AddController(rightClick)

	return box
}

func (d *RavenDesktop) showIconContextMenu(parent *gtk.Box, iconName string, x, y float64) {
	menu := gio.NewMenu()
	menu.Append("Unpin from Desktop", "app.unpin."+strings.ReplaceAll(iconName, " ", "_"))

	// Create action for unpinning this specific icon
	actionName := "unpin." + strings.ReplaceAll(iconName, " ", "_")
	unpinAction := gio.NewSimpleAction(actionName, nil)
	unpinAction.ConnectActivate(func(v *glib.Variant) {
		d.unpinApp(iconName)
	})
	d.app.AddAction(unpinAction)

	popover := gtk.NewPopoverMenuFromModel(menu)
	popover.SetParent(parent)
	popover.SetPosition(gtk.PosBottom)
	popover.Popup()
}

func (d *RavenDesktop) setupContextMenu() {
	// Create menu model
	menu := gio.NewMenu()

	// Add menu items
	section1 := gio.NewMenu()
	section1.Append("Open Terminal", "app.terminal")
	section1.Append("Open File Manager", "app.files")
	section1.Append("Open Fuzzy Finder", "app.fuzzy")
	menu.AppendSection("", section1)

	section2 := gio.NewMenu()
	section2.Append("Pin Application...", "app.pin")
	section2.Append("Change Wallpaper...", "app.wallpaper")
	section2.Append("Raven Settings", "app.settings")
	menu.AppendSection("", section2)

	section3 := gio.NewMenu()
	section3.Append("Refresh Desktop", "app.refresh")
	menu.AppendSection("", section3)

	// Create actions
	termAction := gio.NewSimpleAction("terminal", nil)
	termAction.ConnectActivate(func(v *glib.Variant) {
		d.launchApp("raven-terminal")
	})
	d.app.AddAction(termAction)

	filesAction := gio.NewSimpleAction("files", nil)
	filesAction.ConnectActivate(func(v *glib.Variant) {
		d.launchApp("raven-terminal -e ranger")
	})
	d.app.AddAction(filesAction)

	fuzzyAction := gio.NewSimpleAction("fuzzy", nil)
	fuzzyAction.ConnectActivate(func(v *glib.Variant) {
		d.showFuzzyFinder()
	})
	d.app.AddAction(fuzzyAction)

	pinAction := gio.NewSimpleAction("pin", nil)
	pinAction.ConnectActivate(func(v *glib.Variant) {
		d.showFuzzyFinderForPinning()
	})
	d.app.AddAction(pinAction)

	settingsAction := gio.NewSimpleAction("settings", nil)
	settingsAction.ConnectActivate(func(v *glib.Variant) {
		d.launchApp("raven-settings-menu")
	})
	d.app.AddAction(settingsAction)

	wallpaperAction := gio.NewSimpleAction("wallpaper", nil)
	wallpaperAction.ConnectActivate(func(v *glib.Variant) {
		d.showWallpaperChooser()
	})
	d.app.AddAction(wallpaperAction)

	refreshAction := gio.NewSimpleAction("refresh", nil)
	refreshAction.ConnectActivate(func(v *glib.Variant) {
		d.loadIcons()
		d.refreshIconGrid()
	})
	d.app.AddAction(refreshAction)

	// Set up right-click handler
	gestureClick := gtk.NewGestureClick()
	gestureClick.SetButton(3) // Right click
	gestureClick.ConnectPressed(func(nPress int, x, y float64) {
		d.popover = gtk.NewPopoverMenuFromModel(menu)
		d.popover.SetParent(d.window)
		rect := gdk.NewRectangle(int(x), int(y), 1, 1)
		d.popover.SetPointingTo(&rect)
		d.popover.Popup()
	})
	d.window.AddController(gestureClick)
}

func (d *RavenDesktop) showWallpaperChooser() {
	dialog := gtk.NewFileChooserNative(
		"Select Wallpaper",
		d.window,
		gtk.FileChooserActionOpen,
		"Select",
		"Cancel",
	)

	// Add image filter
	filter := gtk.NewFileFilter()
	filter.SetName("Images")
	filter.AddMIMEType("image/png")
	filter.AddMIMEType("image/jpeg")
	filter.AddMIMEType("image/jpg")
	filter.AddMIMEType("image/webp")
	filter.AddPattern("*.png")
	filter.AddPattern("*.jpg")
	filter.AddPattern("*.jpeg")
	filter.AddPattern("*.webp")
	dialog.AddFilter(filter)

	// Set default location
	picturesDir := filepath.Join(os.Getenv("HOME"), "Pictures")
	if _, err := os.Stat(picturesDir); err == nil {
		picturesFile := gio.NewFileForPath(picturesDir)
		dialog.SetCurrentFolder(picturesFile)
	}

	dialog.ConnectResponse(func(response int) {
		if response == int(gtk.ResponseAccept) {
			file := dialog.File()
			if file != nil {
				path := file.Path()
				d.setWallpaper(path)
			}
		}
	})

	dialog.Show()
}

func (d *RavenDesktop) setWallpaper(path string) {
	// Update settings
	d.settings.WallpaperPath = path

	// Save settings
	data, err := json.MarshalIndent(d.settings, "", "  ")
	if err == nil {
		os.WriteFile(d.settingsPath, data, 0644)
	}

	// Update the background picture
	if d.bgPicture != nil {
		d.bgBox.Remove(d.bgPicture)
	}

	d.bgPicture = gtk.NewPictureForFilename(path)
	d.bgPicture.SetContentFit(gtk.ContentFitCover)
	d.bgPicture.SetHExpand(true)
	d.bgPicture.SetVExpand(true)
	d.bgBox.Append(d.bgPicture)
}

func (d *RavenDesktop) showFuzzyFinder() {
	if d.fuzzyFinder == nil {
		d.fuzzyFinder = fuzzy.New(d.window, func(name, exec, icon string) {
			d.pinApp(DesktopIcon{Name: name, Exec: exec, Icon: icon})
		})
	}
	d.fuzzyFinder.Show(false)
}

func (d *RavenDesktop) showFuzzyFinderForPinning() {
	if d.fuzzyFinder == nil {
		d.fuzzyFinder = fuzzy.New(d.window, func(name, exec, icon string) {
			d.pinApp(DesktopIcon{Name: name, Exec: exec, Icon: icon})
		})
	}
	d.fuzzyFinder.Show(true)
}

func (d *RavenDesktop) loadIcons() {
	d.icons = []DesktopIcon{}

	// Load pinned apps from config
	d.loadPinnedApps()

	// Also load from ~/Desktop if exists (for .desktop files placed there)
	desktopDir := filepath.Join(os.Getenv("HOME"), "Desktop")
	if _, err := os.Stat(desktopDir); err == nil {
		files, _ := filepath.Glob(filepath.Join(desktopDir, "*.desktop"))
		for _, file := range files {
			icon := parseDesktopFile(file)
			if icon.Name != "" {
				d.icons = append(d.icons, icon)
			}
		}
	}
}

func (d *RavenDesktop) loadPinnedApps() {
	data, err := os.ReadFile(d.pinnedAppsPath)
	if err != nil {
		return
	}

	var config PinnedAppsConfig
	if err := json.Unmarshal(data, &config); err != nil {
		return
	}

	d.icons = append(d.icons, config.PinnedApps...)
}

func (d *RavenDesktop) savePinnedApps() error {
	config := PinnedAppsConfig{
		PinnedApps: d.icons,
	}

	data, err := json.MarshalIndent(config, "", "  ")
	if err != nil {
		return err
	}

	return os.WriteFile(d.pinnedAppsPath, data, 0644)
}

func (d *RavenDesktop) pinApp(icon DesktopIcon) {
	// Check if already pinned
	for _, existing := range d.icons {
		if existing.Name == icon.Name {
			return
		}
	}

	d.icons = append(d.icons, icon)
	d.savePinnedApps()
	d.refreshIconGrid()
}

func (d *RavenDesktop) unpinApp(name string) {
	newIcons := []DesktopIcon{}
	for _, icon := range d.icons {
		if icon.Name != name {
			newIcons = append(newIcons, icon)
		}
	}
	d.icons = newIcons
	d.savePinnedApps()
	d.refreshIconGrid()
}

func (d *RavenDesktop) refreshIconGrid() {
	// Remove all children
	for {
		child := d.iconGrid.ChildAtIndex(0)
		if child == nil {
			break
		}
		d.iconGrid.Remove(child)
	}

	// Re-add icons
	for i, icon := range d.icons {
		iconWidget := d.createIconWidget(icon, i)
		d.iconGrid.Append(iconWidget)
	}
}

// parseDesktopFile reads a .desktop file and extracts icon info
func parseDesktopFile(path string) DesktopIcon {
	icon := DesktopIcon{}

	data, err := os.ReadFile(path)
	if err != nil {
		return icon
	}

	lines := strings.Split(string(data), "\n")
	for _, line := range lines {
		line = strings.TrimSpace(line)
		if strings.HasPrefix(line, "Name=") {
			icon.Name = strings.TrimPrefix(line, "Name=")
		} else if strings.HasPrefix(line, "Exec=") {
			exec := strings.TrimPrefix(line, "Exec=")
			// Remove field codes like %f, %u, etc.
			exec = strings.ReplaceAll(exec, "%f", "")
			exec = strings.ReplaceAll(exec, "%F", "")
			exec = strings.ReplaceAll(exec, "%u", "")
			exec = strings.ReplaceAll(exec, "%U", "")
			icon.Exec = strings.TrimSpace(exec)
		} else if strings.HasPrefix(line, "Icon=") {
			icon.Icon = strings.TrimPrefix(line, "Icon=")
		}
	}

	return icon
}

func (d *RavenDesktop) launchApp(cmd string) {
	go func() {
		exec.Command("sh", "-c", cmd).Start()
	}()
}
