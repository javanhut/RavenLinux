package main

import (
	"os"
	"os/exec"
	"time"
	"unsafe"

	"github.com/diamondburned/gotk4/pkg/gdk/v4"
	"github.com/diamondburned/gotk4/pkg/gio/v2"
	"github.com/diamondburned/gotk4/pkg/gtk/v4"
)

/*
#cgo pkg-config: gtk4-layer-shell-0 gtk4
#include <gtk4-layer-shell.h>
#include <gtk/gtk.h>

void init_layer_shell(GtkWidget *window) {
    gtk_layer_init_for_window(GTK_WINDOW(window));
    gtk_layer_set_layer(GTK_WINDOW(window), GTK_LAYER_SHELL_LAYER_TOP);
    gtk_layer_auto_exclusive_zone_enable(GTK_WINDOW(window));
    gtk_layer_set_anchor(GTK_WINDOW(window), GTK_LAYER_SHELL_EDGE_TOP, TRUE);
    gtk_layer_set_anchor(GTK_WINDOW(window), GTK_LAYER_SHELL_EDGE_LEFT, TRUE);
    gtk_layer_set_anchor(GTK_WINDOW(window), GTK_LAYER_SHELL_EDGE_RIGHT, TRUE);
    gtk_layer_set_margin(GTK_WINDOW(window), GTK_LAYER_SHELL_EDGE_TOP, 0);
}

GtkWidget* get_native_widget(void* obj) {
    return GTK_WIDGET(obj);
}
*/
import "C"

const (
	panelHeight = 32
)

// RavenPanel represents the main panel/taskbar
type RavenPanel struct {
	app        *gtk.Application
	window     *gtk.Window
	clockLabel *gtk.Label
}

func main() {
	app := gtk.NewApplication("org.ravenlinux.shell", gio.ApplicationFlagsNone)

	panel := &RavenPanel{
		app: app,
	}

	app.ConnectActivate(func() {
		panel.activate()
	})

	if code := app.Run(os.Args); code > 0 {
		os.Exit(code)
	}
}

func (p *RavenPanel) activate() {
	// Create the panel window
	p.window = gtk.NewWindow()
	p.window.SetTitle("Raven Panel")
	p.window.SetDefaultSize(-1, panelHeight)
	p.window.SetDecorated(false)

	// Apply dark theme CSS
	p.applyCSS()

	// Create the panel content
	content := p.createPanelContent()
	p.window.SetChild(content)

	// Initialize layer shell for proper panel behavior
	p.initLayerShell()

	// Start clock update
	go p.updateClock()

	p.window.SetApplication(p.app)
	p.window.Present()
}

func (p *RavenPanel) initLayerShell() {
	// The layer shell init needs to happen before the window is shown
	// Get the native GObject pointer and convert to GtkWidget
	obj := p.window.Object
	if obj != nil {
		ptr := obj.Native()
		C.init_layer_shell((*C.GtkWidget)(unsafe.Pointer(ptr)))
	}
}

func (p *RavenPanel) applyCSS() {
	css := `
		window {
			background-color: #0f1720;
		}
		box {
			background-color: #0f1720;
		}
		button {
			background-color: transparent;
			border: none;
			border-radius: 4px;
			padding: 4px 12px;
			color: #e0e0e0;
			font-size: 13px;
			min-height: 24px;
		}
		button:hover {
			background-color: rgba(255, 255, 255, 0.1);
		}
		button:active {
			background-color: rgba(0, 150, 136, 0.3);
		}
		label {
			color: #e0e0e0;
			font-size: 13px;
		}
		.start-button {
			background-color: #009688;
			font-weight: bold;
			padding: 4px 16px;
		}
		.start-button:hover {
			background-color: #00a896;
		}
		.clock {
			font-weight: bold;
			padding: 0 12px;
		}
		separator {
			background-color: #333;
			min-width: 1px;
			margin: 4px 8px;
		}
	`

	provider := gtk.NewCSSProvider()
	provider.LoadFromString(css)

	display := gdk.DisplayGetDefault()
	gtk.StyleContextAddProviderForDisplay(display, provider, gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)
}

func (p *RavenPanel) createPanelContent() *gtk.Box {
	// Main horizontal box
	mainBox := gtk.NewBox(gtk.OrientationHorizontal, 0)
	mainBox.SetHomogeneous(false)

	// Left section: Start button + App launchers
	leftBox := gtk.NewBox(gtk.OrientationHorizontal, 4)
	leftBox.SetMarginStart(4)

	// Start/Menu button
	startButton := gtk.NewButton()
	startButton.SetLabel("Raven")
	startButton.AddCSSClass("start-button")
	startButton.ConnectClicked(func() {
		p.showMenu()
	})
	leftBox.Append(startButton)

	// Separator
	sep1 := gtk.NewSeparator(gtk.OrientationVertical)
	leftBox.Append(sep1)

	// Quick launch buttons
	apps := []struct {
		name string
		cmd  string
	}{
		{"Terminal", "raven-terminal"},
		{"Files", "raven-terminal -e ranger"},
		{"WiFi", "raven-wifi"},
	}

	for _, app := range apps {
		btn := p.createAppButton(app.name, app.cmd)
		leftBox.Append(btn)
	}

	mainBox.Append(leftBox)

	// Center section: Window list (spacer for now)
	centerBox := gtk.NewBox(gtk.OrientationHorizontal, 4)
	centerBox.SetHExpand(true)
	mainBox.Append(centerBox)

	// Right section: System tray + Clock
	rightBox := gtk.NewBox(gtk.OrientationHorizontal, 4)
	rightBox.SetMarginEnd(8)

	// Separator
	sep2 := gtk.NewSeparator(gtk.OrientationVertical)
	rightBox.Append(sep2)

	// Clock
	p.clockLabel = gtk.NewLabel("")
	p.clockLabel.AddCSSClass("clock")
	p.updateClockLabel()
	rightBox.Append(p.clockLabel)

	// Separator
	sep3 := gtk.NewSeparator(gtk.OrientationVertical)
	rightBox.Append(sep3)

	// Power button
	powerBtn := gtk.NewButton()
	powerBtn.SetLabel("Power")
	powerBtn.ConnectClicked(func() {
		p.showPowerMenu()
	})
	rightBox.Append(powerBtn)

	mainBox.Append(rightBox)

	return mainBox
}

func (p *RavenPanel) createAppButton(name, cmd string) *gtk.Button {
	btn := gtk.NewButton()
	btn.SetLabel(name)
	btn.ConnectClicked(func() {
		p.launchApp(cmd)
	})
	return btn
}

func (p *RavenPanel) launchApp(cmd string) {
	go func() {
		exec.Command("sh", "-c", cmd).Start()
	}()
}

func (p *RavenPanel) showMenu() {
	// Launch raven-menu (start menu)
	p.launchApp("raven-menu")
}

func (p *RavenPanel) showPowerMenu() {
	// Simple power menu - launch terminal with options
	p.launchApp(`raven-terminal -e 'echo "=== Power Options ===" && echo "1) Logout" && echo "2) Reboot" && echo "3) Shutdown" && echo "4) Cancel" && read -p "Choice [1-4]: " c && case $c in 1) pkill raven-compositor;; 2) reboot;; 3) poweroff;; *) echo "Cancelled";; esac'`)
}

func (p *RavenPanel) updateClock() {
	ticker := time.NewTicker(time.Second)
	for range ticker.C {
		if p.clockLabel != nil {
			// GTK operations must be done on main thread
			// Use GLib idle add for thread safety
			p.updateClockLabel()
		}
	}
}

func (p *RavenPanel) updateClockLabel() {
	now := time.Now()
	p.clockLabel.SetText(now.Format("Mon Jan 2  3:04 PM"))
}
