package main

import (
	"os"
	"os/exec"
	"unsafe"

	"github.com/diamondburned/gotk4/pkg/gdk/v4"
	"github.com/diamondburned/gotk4/pkg/gio/v2"
	"github.com/diamondburned/gotk4/pkg/gtk/v4"
)

/*
#cgo pkg-config: gtk4-layer-shell-0 gtk4
#include <gtk4-layer-shell.h>
#include <gtk/gtk.h>

void init_power_layer_shell(GtkWidget *window) {
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

// PowerOption represents a power menu option
type PowerOption struct {
	Name        string
	Icon        string
	Description string
	Action      func()
}

// RavenPower is the power menu application
type RavenPower struct {
	app     *gtk.Application
	window  *gtk.Window
	options []PowerOption
}

func main() {
	app := gtk.NewApplication("org.ravenlinux.power", gio.ApplicationFlagsNone)

	pm := &RavenPower{
		app: app,
	}

	app.ConnectActivate(func() {
		pm.activate()
	})

	if code := app.Run(os.Args); code > 0 {
		os.Exit(code)
	}
}

func (pm *RavenPower) activate() {
	pm.window = gtk.NewWindow()
	pm.window.SetTitle("Power Menu")
	pm.window.SetDecorated(false)

	// Initialize options
	pm.initOptions()

	// Apply CSS
	pm.applyCSS()

	// Create UI
	content := pm.createUI()
	pm.window.SetChild(content)

	// Initialize layer shell
	pm.initLayerShell()

	// Close on Escape
	keyController := gtk.NewEventControllerKey()
	keyController.ConnectKeyPressed(func(keyval, keycode uint, state gdk.ModifierType) bool {
		if keyval == gdk.KEY_Escape {
			pm.window.Close()
			return true
		}
		return false
	})
	pm.window.AddController(keyController)

	pm.window.SetApplication(pm.app)
	pm.window.Present()
}

func (pm *RavenPower) initLayerShell() {
	obj := pm.window.Object
	if obj != nil {
		ptr := obj.Native()
		C.init_power_layer_shell((*C.GtkWidget)(unsafe.Pointer(ptr)))
	}
}

func (pm *RavenPower) initOptions() {
	pm.options = []PowerOption{
		{
			Name:        "Lock",
			Icon:        "system-lock-screen-symbolic",
			Description: "Lock the screen",
			Action: func() {
				pm.window.Close()
				exec.Command("sh", "-c", "hyprlock || swaylock || loginctl lock-session").Start()
			},
		},
		{
			Name:        "Logout",
			Icon:        "system-log-out-symbolic",
			Description: "End session",
			Action: func() {
				pm.window.Close()
				exec.Command("hyprctl", "dispatch", "exit").Start()
			},
		},
		{
			Name:        "Suspend",
			Icon:        "system-suspend-symbolic",
			Description: "Sleep the computer",
			Action: func() {
				pm.window.Close()
				exec.Command("systemctl", "suspend").Start()
			},
		},
		{
			Name:        "Hibernate",
			Icon:        "system-hibernate-symbolic",
			Description: "Hibernate to disk",
			Action: func() {
				pm.window.Close()
				exec.Command("systemctl", "hibernate").Start()
			},
		},
		{
			Name:        "Reboot",
			Icon:        "system-reboot-symbolic",
			Description: "Restart the computer",
			Action: func() {
				pm.window.Close()
				exec.Command("sh", "-c", "systemctl reboot || reboot").Start()
			},
		},
		{
			Name:        "Shutdown",
			Icon:        "system-shutdown-symbolic",
			Description: "Power off the computer",
			Action: func() {
				pm.window.Close()
				exec.Command("sh", "-c", "systemctl poweroff || poweroff").Start()
			},
		},
	}
}

func (pm *RavenPower) applyCSS() {
	css := `
		window {
			background-color: rgba(15, 23, 32, 0.92);
		}
		.power-container {
			padding: 40px;
		}
		.power-title {
			font-size: 24px;
			font-weight: bold;
			color: #e0e0e0;
			margin-bottom: 32px;
		}
		.power-options {
			margin: 0;
		}
		.power-button {
			background-color: rgba(255, 255, 255, 0.05);
			border: 1px solid rgba(255, 255, 255, 0.1);
			border-radius: 12px;
			padding: 24px 32px;
			margin: 8px;
			min-width: 120px;
			min-height: 100px;
			transition: all 150ms ease;
		}
		.power-button:hover {
			background-color: rgba(255, 255, 255, 0.1);
			border-color: rgba(0, 150, 136, 0.5);
		}
		.power-button:active {
			background-color: rgba(0, 150, 136, 0.2);
		}
		.power-button-shutdown {
			border-color: rgba(244, 67, 54, 0.3);
		}
		.power-button-shutdown:hover {
			background-color: rgba(244, 67, 54, 0.2);
			border-color: rgba(244, 67, 54, 0.5);
		}
		.power-button-reboot {
			border-color: rgba(255, 152, 0, 0.3);
		}
		.power-button-reboot:hover {
			background-color: rgba(255, 152, 0, 0.2);
			border-color: rgba(255, 152, 0, 0.5);
		}
		.power-icon {
			font-size: 32px;
			color: #e0e0e0;
			margin-bottom: 8px;
		}
		.power-label {
			font-size: 14px;
			font-weight: 500;
			color: #e0e0e0;
		}
		.power-desc {
			font-size: 11px;
			color: #888;
			margin-top: 4px;
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

func (pm *RavenPower) createUI() *gtk.Box {
	mainBox := gtk.NewBox(gtk.OrientationVertical, 0)
	mainBox.AddCSSClass("power-container")
	mainBox.SetHAlign(gtk.AlignCenter)
	mainBox.SetVAlign(gtk.AlignCenter)

	// Title
	title := gtk.NewLabel("Power Menu")
	title.AddCSSClass("power-title")
	mainBox.Append(title)

	// Options grid
	optionsBox := gtk.NewBox(gtk.OrientationHorizontal, 16)
	optionsBox.AddCSSClass("power-options")
	optionsBox.SetHAlign(gtk.AlignCenter)

	for _, opt := range pm.options {
		btn := pm.createPowerButton(opt)
		optionsBox.Append(btn)
	}

	mainBox.Append(optionsBox)

	// Hint
	hint := gtk.NewLabel("Press Escape to cancel")
	hint.AddCSSClass("hint-text")
	mainBox.Append(hint)

	return mainBox
}

func (pm *RavenPower) createPowerButton(opt PowerOption) *gtk.Button {
	btn := gtk.NewButton()
	btn.AddCSSClass("power-button")

	// Add specific class for shutdown/reboot
	if opt.Name == "Shutdown" {
		btn.AddCSSClass("power-button-shutdown")
	} else if opt.Name == "Reboot" {
		btn.AddCSSClass("power-button-reboot")
	}

	contentBox := gtk.NewBox(gtk.OrientationVertical, 4)
	contentBox.SetHAlign(gtk.AlignCenter)

	// Icon
	icon := gtk.NewImageFromIconName(opt.Icon)
	icon.SetPixelSize(48)
	icon.AddCSSClass("power-icon")
	contentBox.Append(icon)

	// Label
	label := gtk.NewLabel(opt.Name)
	label.AddCSSClass("power-label")
	contentBox.Append(label)

	// Description
	desc := gtk.NewLabel(opt.Description)
	desc.AddCSSClass("power-desc")
	contentBox.Append(desc)

	btn.SetChild(contentBox)

	// Connect click handler
	action := opt.Action
	btn.ConnectClicked(func() {
		action()
	})

	return btn
}
