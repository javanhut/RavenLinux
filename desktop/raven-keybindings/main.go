package main

import (
	"os"
	"unsafe"

	"github.com/diamondburned/gotk4/pkg/gdk/v4"
	"github.com/diamondburned/gotk4/pkg/gio/v2"
	"github.com/diamondburned/gotk4/pkg/gtk/v4"
)

/*
#cgo pkg-config: gtk4-layer-shell-0 gtk4
#include <gtk4-layer-shell.h>
#include <gtk/gtk.h>

void init_keybindings_layer_shell(GtkWidget *window) {
    gtk_layer_init_for_window(GTK_WINDOW(window));
    gtk_layer_set_layer(GTK_WINDOW(window), GTK_LAYER_SHELL_LAYER_OVERLAY);
    gtk_layer_set_keyboard_mode(GTK_WINDOW(window), GTK_LAYER_SHELL_KEYBOARD_MODE_EXCLUSIVE);
    gtk_layer_set_anchor(GTK_WINDOW(window), GTK_LAYER_SHELL_EDGE_TOP, TRUE);
    gtk_layer_set_anchor(GTK_WINDOW(window), GTK_LAYER_SHELL_EDGE_BOTTOM, TRUE);
    gtk_layer_set_anchor(GTK_WINDOW(window), GTK_LAYER_SHELL_EDGE_LEFT, TRUE);
    gtk_layer_set_anchor(GTK_WINDOW(window), GTK_LAYER_SHELL_EDGE_RIGHT, TRUE);
}
*/
import "C"

// KeyBinding represents a single keybinding
type KeyBinding struct {
	Keys        string
	Description string
	Category    string
}

// RavenKeybindings is the keybindings overlay application
type RavenKeybindings struct {
	app      *gtk.Application
	window   *gtk.Window
	bindings []KeyBinding
}

func main() {
	app := gtk.NewApplication("org.ravenlinux.keybindings", gio.ApplicationFlagsNone)

	kb := &RavenKeybindings{
		app: app,
	}

	app.ConnectActivate(func() {
		kb.activate()
	})

	if code := app.Run(os.Args); code > 0 {
		os.Exit(code)
	}
}

func (kb *RavenKeybindings) activate() {
	kb.window = gtk.NewWindow()
	kb.window.SetTitle("Keyboard Shortcuts")
	kb.window.SetDecorated(false)

	// Initialize keybindings data
	kb.initBindings()

	// Apply CSS
	kb.applyCSS()

	// Create UI
	content := kb.createUI()
	kb.window.SetChild(content)

	// Initialize layer shell
	kb.initLayerShell()

	// Close on Escape or any key press
	keyController := gtk.NewEventControllerKey()
	keyController.ConnectKeyPressed(func(keyval, keycode uint, state gdk.ModifierType) bool {
		kb.window.Close()
		return true
	})
	kb.window.AddController(keyController)

	// Close on click
	clickController := gtk.NewGestureClick()
	clickController.ConnectPressed(func(nPress int, x, y float64) {
		kb.window.Close()
	})
	kb.window.AddController(clickController)

	kb.window.SetApplication(kb.app)
	kb.window.Present()
}

func (kb *RavenKeybindings) initLayerShell() {
	obj := kb.window.Object
	if obj != nil {
		ptr := obj.Native()
		C.init_keybindings_layer_shell((*C.GtkWidget)(unsafe.Pointer(ptr)))
	}
}

func (kb *RavenKeybindings) initBindings() {
	kb.bindings = []KeyBinding{
		// Applications
		{Keys: "Super + T", Description: "Open Terminal", Category: "Applications"},
		{Keys: "Super + M", Description: "Open Menu", Category: "Applications"},
		{Keys: "Super + S", Description: "Open Settings", Category: "Applications"},
		{Keys: "Super + F", Description: "Fuzzy Finder / Launcher", Category: "Applications"},
		{Keys: "Super + P", Description: "Power Menu", Category: "Applications"},
		{Keys: "Super + K", Description: "Show Keybindings (this)", Category: "Applications"},
		{Keys: "Super + W", Description: "WiFi Settings", Category: "Applications"},
		{Keys: "Super + Shift + E", Description: "File Manager", Category: "Applications"},

		// Window Management
		{Keys: "Super + Q", Description: "Close Window", Category: "Windows"},
		{Keys: "Super + V", Description: "Toggle Floating", Category: "Windows"},
		{Keys: "Super + Shift + F", Description: "Fullscreen", Category: "Windows"},
		{Keys: "Super + J", Description: "Toggle Split", Category: "Windows"},
		{Keys: "Super + R", Description: "Enter Resize Mode", Category: "Windows"},

		// Focus
		{Keys: "Super + Arrow Keys", Description: "Move Focus", Category: "Focus"},
		{Keys: "Super + H/J/K/L", Description: "Move Focus (Vim)", Category: "Focus"},
		{Keys: "Alt + Tab", Description: "Cycle Windows", Category: "Focus"},

		// Window Movement
		{Keys: "Super + Shift + Arrows", Description: "Move Window", Category: "Movement"},
		{Keys: "Super + Shift + H/J/K/L", Description: "Move Window (Vim)", Category: "Movement"},
		{Keys: "Super + Mouse Drag", Description: "Move Window", Category: "Movement"},
		{Keys: "Super + Right Click Drag", Description: "Resize Window", Category: "Movement"},

		// Workspaces
		{Keys: "Super + 1-0", Description: "Switch to Workspace 1-10", Category: "Workspaces"},
		{Keys: "Super + Shift + 1-0", Description: "Move Window to Workspace", Category: "Workspaces"},
		{Keys: "Super + Tab", Description: "Next Workspace", Category: "Workspaces"},
		{Keys: "Super + Shift + Tab", Description: "Previous Workspace", Category: "Workspaces"},
		{Keys: "Super + Scroll", Description: "Cycle Workspaces", Category: "Workspaces"},

		// Media
		{Keys: "Volume Keys", Description: "Adjust Volume", Category: "Media"},
		{Keys: "Brightness Keys", Description: "Adjust Brightness", Category: "Media"},
		{Keys: "Play/Pause", Description: "Media Play/Pause", Category: "Media"},
		{Keys: "Next/Prev", Description: "Media Next/Previous", Category: "Media"},

		// Screenshots
		{Keys: "Print", Description: "Screenshot Region to Clipboard", Category: "Screenshots"},
		{Keys: "Shift + Print", Description: "Screenshot Full to Clipboard", Category: "Screenshots"},
		{Keys: "Super + Print", Description: "Screenshot Region to File", Category: "Screenshots"},
		{Keys: "Super + Shift + Print", Description: "Screenshot Full to File", Category: "Screenshots"},

		// System
		{Keys: "Super + Escape", Description: "Lock Screen", Category: "System"},
		{Keys: "Super + Shift + Q", Description: "Exit Hyprland", Category: "System"},
	}
}

func (kb *RavenKeybindings) applyCSS() {
	css := `
		window {
			background-color: rgba(15, 23, 32, 0.92);
		}
		.overlay-container {
			padding: 40px;
		}
		.overlay-title {
			font-size: 28px;
			font-weight: bold;
			color: #00bfa5;
			margin-bottom: 8px;
		}
		.overlay-subtitle {
			font-size: 14px;
			color: #888;
			margin-bottom: 32px;
		}
		.category-title {
			font-size: 14px;
			font-weight: bold;
			color: #009688;
			margin-top: 16px;
			margin-bottom: 8px;
			padding-bottom: 4px;
			border-bottom: 1px solid rgba(0, 150, 136, 0.3);
		}
		.binding-row {
			padding: 6px 0;
		}
		.binding-keys {
			font-family: monospace;
			font-size: 13px;
			font-weight: bold;
			color: #e0e0e0;
			background-color: rgba(255, 255, 255, 0.1);
			padding: 4px 10px;
			border-radius: 4px;
			min-width: 180px;
		}
		.binding-description {
			font-size: 13px;
			color: #aaa;
			margin-left: 16px;
		}
		.columns-container {
			margin-top: 16px;
		}
		.column {
			padding: 0 24px;
		}
		.hint-text {
			font-size: 12px;
			color: #666;
			margin-top: 32px;
		}
	`

	provider := gtk.NewCSSProvider()
	provider.LoadFromString(css)
	display := gdk.DisplayGetDefault()
	gtk.StyleContextAddProviderForDisplay(display, provider, gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)
}

func (kb *RavenKeybindings) createUI() *gtk.Box {
	mainBox := gtk.NewBox(gtk.OrientationVertical, 0)
	mainBox.AddCSSClass("overlay-container")
	mainBox.SetHAlign(gtk.AlignCenter)
	mainBox.SetVAlign(gtk.AlignCenter)

	// Title
	title := gtk.NewLabel("Keyboard Shortcuts")
	title.AddCSSClass("overlay-title")
	mainBox.Append(title)

	// Subtitle
	subtitle := gtk.NewLabel("Press any key to close")
	subtitle.AddCSSClass("overlay-subtitle")
	mainBox.Append(subtitle)

	// Create columns container
	columnsBox := gtk.NewBox(gtk.OrientationHorizontal, 0)
	columnsBox.AddCSSClass("columns-container")
	columnsBox.SetHAlign(gtk.AlignCenter)

	// Group bindings by category
	categories := []string{"Applications", "Windows", "Focus", "Movement", "Workspaces", "Media", "Screenshots", "System"}
	categoryBindings := make(map[string][]KeyBinding)

	for _, binding := range kb.bindings {
		categoryBindings[binding.Category] = append(categoryBindings[binding.Category], binding)
	}

	// Split categories into columns
	leftCategories := []string{"Applications", "Windows", "Focus", "Movement"}
	rightCategories := []string{"Workspaces", "Media", "Screenshots", "System"}

	// Left column
	leftColumn := gtk.NewBox(gtk.OrientationVertical, 0)
	leftColumn.AddCSSClass("column")
	for _, cat := range leftCategories {
		if bindings, ok := categoryBindings[cat]; ok && len(bindings) > 0 {
			leftColumn.Append(kb.createCategorySection(cat, bindings))
		}
	}
	columnsBox.Append(leftColumn)

	// Separator
	sep := gtk.NewSeparator(gtk.OrientationVertical)
	sep.SetMarginStart(24)
	sep.SetMarginEnd(24)
	columnsBox.Append(sep)

	// Right column
	rightColumn := gtk.NewBox(gtk.OrientationVertical, 0)
	rightColumn.AddCSSClass("column")
	for _, cat := range rightCategories {
		if bindings, ok := categoryBindings[cat]; ok && len(bindings) > 0 {
			rightColumn.Append(kb.createCategorySection(cat, bindings))
		}
	}
	columnsBox.Append(rightColumn)

	mainBox.Append(columnsBox)

	// Hint
	hint := gtk.NewLabel("Tip: Use Super + K anytime to show this overlay")
	hint.AddCSSClass("hint-text")
	mainBox.Append(hint)

	// Ensure unused categories variable is used
	_ = categories

	return mainBox
}

func (kb *RavenKeybindings) createCategorySection(category string, bindings []KeyBinding) *gtk.Box {
	section := gtk.NewBox(gtk.OrientationVertical, 0)

	// Category title
	catTitle := gtk.NewLabel(category)
	catTitle.AddCSSClass("category-title")
	catTitle.SetHAlign(gtk.AlignStart)
	section.Append(catTitle)

	// Bindings
	for _, binding := range bindings {
		row := gtk.NewBox(gtk.OrientationHorizontal, 0)
		row.AddCSSClass("binding-row")

		keysLabel := gtk.NewLabel(binding.Keys)
		keysLabel.AddCSSClass("binding-keys")
		keysLabel.SetHAlign(gtk.AlignStart)
		row.Append(keysLabel)

		descLabel := gtk.NewLabel(binding.Description)
		descLabel.AddCSSClass("binding-description")
		descLabel.SetHAlign(gtk.AlignStart)
		row.Append(descLabel)

		section.Append(row)
	}

	return section
}
