package main

import (
	"encoding/json"
	"os"
	"os/exec"
	"path/filepath"
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
	Name string
	Exec string
	Icon string
	X    int
	Y    int
}

// RavenSettings holds shared settings
type RavenSettings struct {
	WallpaperPath    string `json:"wallpaper_path"`
	WallpaperMode    string `json:"wallpaper_mode"`
	ShowDesktopIcons bool   `json:"show_desktop_icons"`
}

// RavenDesktop is the desktop background with icons
type RavenDesktop struct {
	app          *gtk.Application
	window       *gtk.Window
	iconGrid     *gtk.FlowBox
	icons        []DesktopIcon
	popover      *gtk.PopoverMenu
	settings     RavenSettings
	settingsPath string
}

func main() {
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
	d.window.SetDecorated(false)

	// Load desktop icons
	d.loadIcons()

	// Apply CSS
	d.applyCSS()

	// Create UI
	content := d.createUI()
	d.window.SetChild(content)

	// Initialize layer shell for background
	d.initLayerShell()

	// Set up right-click menu
	d.setupContextMenu()

	d.window.SetApplication(d.app)
	d.window.Present()
}

func (d *RavenDesktop) loadSettings() {
	d.settingsPath = filepath.Join(os.Getenv("HOME"), ".config", "raven", "settings.json")

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
	overlay := gtk.NewOverlay()

	// Background
	bg := gtk.NewBox(gtk.OrientationVertical, 0)
	bg.AddCSSClass("desktop-bg")
	bg.SetHExpand(true)
	bg.SetVExpand(true)

	// Try to load wallpaper from settings first, then fallback
	wallpaperPaths := []string{}
	if d.settings.WallpaperPath != "" {
		wallpaperPaths = append(wallpaperPaths, d.settings.WallpaperPath)
	}
	wallpaperPaths = append(wallpaperPaths,
		"/usr/share/backgrounds/raven-wallpaper.png",
		"/usr/share/backgrounds/default.png",
	)

	var picture *gtk.Picture
	for _, path := range wallpaperPaths {
		if _, err := os.Stat(path); err == nil {
			picture = gtk.NewPictureForFilename(path)
			picture.SetContentFit(gtk.ContentFitCover)
			picture.SetHExpand(true)
			picture.SetVExpand(true)
			break
		}
	}

	if picture != nil {
		bg.Append(picture)
	}

	overlay.SetChild(bg)

	// Icon grid
	d.iconGrid = gtk.NewFlowBox()
	d.iconGrid.SetSelectionMode(gtk.SelectionSingle)
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

	// Add icons
	for _, icon := range d.icons {
		iconWidget := d.createIconWidget(icon)
		d.iconGrid.Append(iconWidget)
	}

	// Handle double-click
	d.iconGrid.ConnectChildActivated(func(child *gtk.FlowBoxChild) {
		idx := child.Index()
		if idx >= 0 && idx < len(d.icons) {
			d.launchApp(d.icons[idx].Exec)
		}
	})

	overlay.AddOverlay(d.iconGrid)

	return overlay
}

func (d *RavenDesktop) createIconWidget(icon DesktopIcon) *gtk.Box {
	box := gtk.NewBox(gtk.OrientationVertical, 4)
	box.AddCSSClass("desktop-icon")
	box.SetHAlign(gtk.AlignCenter)

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

	return box
}

func (d *RavenDesktop) setupContextMenu() {
	// Create menu model
	menu := gio.NewMenu()

	// Add menu items
	section1 := gio.NewMenu()
	section1.Append("Open Terminal", "app.terminal")
	section1.Append("Open File Manager", "app.files")
	menu.AppendSection("", section1)

	section2 := gio.NewMenu()
	section2.Append("Raven Settings", "app.settings")
	section2.Append("Change Wallpaper", "app.wallpaper")
	menu.AppendSection("", section2)

	section3 := gio.NewMenu()
	section3.Append("Refresh Desktop", "app.refresh")
	menu.AppendSection("", section3)

	// Create actions using simpler approach
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

	settingsAction := gio.NewSimpleAction("settings", nil)
	settingsAction.ConnectActivate(func(v *glib.Variant) {
		d.launchApp("raven-settings-menu")
	})
	d.app.AddAction(settingsAction)

	wallpaperAction := gio.NewSimpleAction("wallpaper", nil)
	wallpaperAction.ConnectActivate(func(v *glib.Variant) {
		d.launchApp("waypaper || raven-settings-menu")
	})
	d.app.AddAction(wallpaperAction)

	refreshAction := gio.NewSimpleAction("refresh", nil)
	refreshAction.ConnectActivate(func(v *glib.Variant) {
		d.loadIcons()
		// Refresh grid
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

func (d *RavenDesktop) loadIcons() {
	d.icons = []DesktopIcon{
		{Name: "Terminal", Exec: "raven-terminal", Icon: "utilities-terminal"},
		{Name: "Files", Exec: "raven-terminal -e ranger", Icon: "system-file-manager"},
		{Name: "WiFi", Exec: "raven-wifi", Icon: "network-wireless"},
		{Name: "Installer", Exec: "raven-installer", Icon: "system-software-install"},
		{Name: "App Launcher", Exec: "raven-launcher", Icon: "system-search"},
		{Name: "System Monitor", Exec: "raven-terminal -e htop", Icon: "utilities-system-monitor"},
	}

	// Load from ~/Desktop if exists
	desktopDir := filepath.Join(os.Getenv("HOME"), "Desktop")
	if _, err := os.Stat(desktopDir); err == nil {
		files, _ := filepath.Glob(filepath.Join(desktopDir, "*.desktop"))
		for _, file := range files {
			// Parse desktop file and add icon
			// Simplified - just use filename
			name := filepath.Base(file)
			name = name[:len(name)-8] // Remove .desktop
			d.icons = append(d.icons, DesktopIcon{
				Name: name,
				Exec: "gtk-launch " + name,
				Icon: "application-x-executable",
			})
		}
	}
}

func (d *RavenDesktop) launchApp(cmd string) {
	go func() {
		exec.Command("sh", "-c", cmd).Start()
	}()
}
