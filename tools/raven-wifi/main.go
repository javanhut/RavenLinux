package main

import (
	"fmt"
	"log"
	"os"

	"gioui.org/app"
	"gioui.org/op"
	"gioui.org/unit"
)

func main() {
	// Check root privileges
	if os.Geteuid() != 0 {
		fmt.Fprintln(os.Stderr, "This tool requires root privileges. Please run with sudo:")
		fmt.Fprintln(os.Stderr, "  sudo -E raven-wifi")
		fmt.Fprintln(os.Stderr, "")
		fmt.Fprintln(os.Stderr, "Note: Use -E to preserve environment variables for GUI display")
		os.Exit(1)
	}

	// Check if XDG_RUNTIME_DIR is set (needed for Wayland/X11)
	if os.Getenv("XDG_RUNTIME_DIR") == "" {
		fmt.Fprintln(os.Stderr, "Error: XDG_RUNTIME_DIR is not set.")
		fmt.Fprintln(os.Stderr, "This is required for GUI display.")
		fmt.Fprintln(os.Stderr, "")
		fmt.Fprintln(os.Stderr, "Please run with:")
		fmt.Fprintln(os.Stderr, "  sudo -E raven-wifi")
		fmt.Fprintln(os.Stderr, "")
		fmt.Fprintln(os.Stderr, "The -E flag preserves your user environment.")
		os.Exit(1)
	}

	go func() {
		w := new(app.Window)
		if err := run(w); err != nil {
			log.Fatal(err)
		}
		os.Exit(0)
	}()

	app.Main()
}

func run(w *app.Window) error {
	// Create application state
	state := NewAppState()

	// Configure window
	w.Option(app.Title("Raven WiFi"))
	w.Option(app.Size(
		unit.Dp(float32(state.config.Window.Width)),
		unit.Dp(float32(state.config.Window.Height)),
	))
	w.Option(app.MinSize(unit.Dp(300), unit.Dp(400)))

	// Allow background goroutines to trigger redraws
	state.invalidate = w.Invalidate

	// Start initial scan in background
	go state.refreshNetworks()

	// Create theme
	th := NewDarkTheme()

	// Operations list for drawing
	var ops op.Ops

	// Main event loop
	for {
		switch e := w.Event().(type) {
		case app.DestroyEvent:
			// Save config on exit
			state.config.Save()
			return e.Err

		case app.FrameEvent:
			// Create graphics context
			gtx := app.NewContext(&ops, e)

			// Track size changes for config persistence
			newSize := e.Size
			if newSize != state.lastSize && state.lastSize.X > 0 {
				state.lastSize = newSize
				state.config.Window.Width = newSize.X
				state.config.Window.Height = newSize.Y
				state.saveSizeDebounced()
			} else if state.lastSize.X == 0 {
				// First frame - just record size
				state.lastSize = newSize
			}

			// Layout the UI
			state.Layout(gtx, th)

			// Render the frame
			e.Frame(gtx.Ops)
		}
	}
}
