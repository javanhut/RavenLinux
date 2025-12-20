package main

import (
	"bufio"
	"image/color"
	"os"
	"os/exec"
	"path/filepath"
	"sort"
	"strings"

	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/app"
	"fyne.io/fyne/v2/canvas"
	"fyne.io/fyne/v2/container"
	"fyne.io/fyne/v2/theme"
	"fyne.io/fyne/v2/widget"
)

// Application represents a launchable application
type Application struct {
	Name    string
	Exec    string
	Icon    string
	Comment string
}

// LauncherApp holds the application state
type LauncherApp struct {
	window       fyne.Window
	searchEntry  *widget.Entry
	appList      *widget.List
	apps         []Application
	filteredApps []Application
	selectedIdx  int
}

func main() {
	// Try to detect display environment (same as raven-wifi)
	ensureDisplayEnvironment()

	a := app.New()
	a.Settings().SetTheme(&ravenTheme{})

	w := a.NewWindow("Raven Launcher")
	w.Resize(fyne.NewSize(500, 400))
	w.CenterOnScreen()

	launcher := &LauncherApp{
		window:      w,
		selectedIdx: 0,
	}

	// Load applications
	launcher.apps = launcher.discoverApplications()
	launcher.filteredApps = launcher.apps

	w.SetContent(launcher.createUI())

	// Focus search entry on start
	w.Canvas().Focus(launcher.searchEntry)

	// Close on Escape
	w.Canvas().SetOnTypedKey(func(key *fyne.KeyEvent) {
		switch key.Name {
		case fyne.KeyEscape:
			w.Close()
		case fyne.KeyReturn, fyne.KeyEnter:
			launcher.launchSelected()
		case fyne.KeyDown:
			launcher.selectNext()
		case fyne.KeyUp:
			launcher.selectPrev()
		}
	})

	w.ShowAndRun()
}

func (l *LauncherApp) createUI() fyne.CanvasObject {
	// Title
	title := widget.NewLabelWithStyle("Launch Application", fyne.TextAlignCenter, fyne.TextStyle{Bold: true})

	// Search entry
	l.searchEntry = widget.NewEntry()
	l.searchEntry.SetPlaceHolder("Type to search...")
	l.searchEntry.OnChanged = l.filterApps

	// Application list
	l.appList = widget.NewList(
		func() int {
			return len(l.filteredApps)
		},
		func() fyne.CanvasObject {
			nameLabel := widget.NewLabel("Application Name")
			nameLabel.TextStyle = fyne.TextStyle{Bold: true}
			descLabel := widget.NewLabel("Description")
			descLabel.TextStyle = fyne.TextStyle{}
			return container.NewVBox(nameLabel, descLabel)
		},
		func(id widget.ListItemID, obj fyne.CanvasObject) {
			if id >= len(l.filteredApps) {
				return
			}
			app := l.filteredApps[id]
			box := obj.(*fyne.Container)
			nameLabel := box.Objects[0].(*widget.Label)
			descLabel := box.Objects[1].(*widget.Label)
			nameLabel.SetText(app.Name)
			if app.Comment != "" {
				descLabel.SetText(app.Comment)
			} else {
				descLabel.SetText(app.Exec)
			}
		},
	)

	l.appList.OnSelected = func(id widget.ListItemID) {
		l.selectedIdx = id
	}

	l.appList.OnUnselected = func(id widget.ListItemID) {}

	// Double-click to launch
	l.appList.OnSelected = func(id widget.ListItemID) {
		l.selectedIdx = id
	}

	// Launch button
	launchBtn := widget.NewButtonWithIcon("Launch", theme.MediaPlayIcon(), l.launchSelected)
	launchBtn.Importance = widget.HighImportance

	// Cancel button
	cancelBtn := widget.NewButton("Cancel", func() {
		l.window.Close()
	})

	buttonBar := container.NewHBox(
		widget.NewLabel(""),
		cancelBtn,
		launchBtn,
	)

	// Hint text
	hint := widget.NewLabelWithStyle("Press Enter to launch, Escape to close", fyne.TextAlignCenter, fyne.TextStyle{Italic: true})
	hint.TextStyle.Italic = true

	// Main layout
	content := container.NewBorder(
		container.NewVBox(title, l.searchEntry),
		container.NewVBox(buttonBar, hint),
		nil, nil,
		l.appList,
	)

	return container.NewPadded(content)
}

func (l *LauncherApp) filterApps(query string) {
	query = strings.ToLower(query)
	if query == "" {
		l.filteredApps = l.apps
	} else {
		l.filteredApps = nil
		for _, app := range l.apps {
			if strings.Contains(strings.ToLower(app.Name), query) ||
				strings.Contains(strings.ToLower(app.Exec), query) ||
				strings.Contains(strings.ToLower(app.Comment), query) {
				l.filteredApps = append(l.filteredApps, app)
			}
		}
	}
	l.selectedIdx = 0
	l.appList.Refresh()
	if len(l.filteredApps) > 0 {
		l.appList.Select(0)
	}
}

func (l *LauncherApp) selectNext() {
	if l.selectedIdx < len(l.filteredApps)-1 {
		l.selectedIdx++
		l.appList.Select(l.selectedIdx)
	}
}

func (l *LauncherApp) selectPrev() {
	if l.selectedIdx > 0 {
		l.selectedIdx--
		l.appList.Select(l.selectedIdx)
	}
}

func (l *LauncherApp) launchSelected() {
	if l.selectedIdx >= 0 && l.selectedIdx < len(l.filteredApps) {
		app := l.filteredApps[l.selectedIdx]
		l.launchApp(app)
	}
}

func (l *LauncherApp) launchApp(app Application) {
	// Parse the Exec field - remove field codes like %f, %F, %u, %U
	execCmd := app.Exec
	execCmd = strings.ReplaceAll(execCmd, "%f", "")
	execCmd = strings.ReplaceAll(execCmd, "%F", "")
	execCmd = strings.ReplaceAll(execCmd, "%u", "")
	execCmd = strings.ReplaceAll(execCmd, "%U", "")
	execCmd = strings.ReplaceAll(execCmd, "%i", "")
	execCmd = strings.ReplaceAll(execCmd, "%c", "")
	execCmd = strings.ReplaceAll(execCmd, "%k", "")
	execCmd = strings.TrimSpace(execCmd)

	parts := strings.Fields(execCmd)
	if len(parts) == 0 {
		return
	}

	cmd := exec.Command(parts[0], parts[1:]...)
	cmd.Stdout = nil
	cmd.Stderr = nil
	cmd.Start()

	l.window.Close()
}

func (l *LauncherApp) discoverApplications() []Application {
	var apps []Application

	// Standard .desktop file locations
	desktopDirs := []string{
		"/usr/share/applications",
		"/usr/local/share/applications",
		filepath.Join(os.Getenv("HOME"), ".local/share/applications"),
	}

	seen := make(map[string]bool)

	for _, dir := range desktopDirs {
		files, err := filepath.Glob(filepath.Join(dir, "*.desktop"))
		if err != nil {
			continue
		}

		for _, file := range files {
			app := parseDesktopFile(file)
			if app != nil && !seen[app.Name] {
				apps = append(apps, *app)
				seen[app.Name] = true
			}
		}
	}

	// Add built-in Raven tools
	ravenApps := []Application{
		{Name: "Raven WiFi", Exec: "raven-wifi", Comment: "WiFi network manager"},
		{Name: "Raven Installer", Exec: "raven-installer", Comment: "Install RavenLinux to disk"},
		{Name: "Terminal", Exec: "weston-terminal", Comment: "Open a terminal"},
		{Name: "File Manager", Exec: "weston-terminal -e ranger", Comment: "Browse files with ranger"},
		{Name: "System Monitor", Exec: "weston-terminal -e htop", Comment: "View system processes"},
		{Name: "Text Editor", Exec: "weston-terminal -e nvim", Comment: "Edit text files with Neovim"},
	}

	for _, app := range ravenApps {
		if !seen[app.Name] {
			apps = append(apps, app)
			seen[app.Name] = true
		}
	}

	// Sort alphabetically
	sort.Slice(apps, func(i, j int) bool {
		return strings.ToLower(apps[i].Name) < strings.ToLower(apps[j].Name)
	})

	return apps
}

func parseDesktopFile(path string) *Application {
	file, err := os.Open(path)
	if err != nil {
		return nil
	}
	defer file.Close()

	app := &Application{}
	inDesktopEntry := false

	scanner := bufio.NewScanner(file)
	for scanner.Scan() {
		line := strings.TrimSpace(scanner.Text())

		if line == "[Desktop Entry]" {
			inDesktopEntry = true
			continue
		}

		if strings.HasPrefix(line, "[") {
			inDesktopEntry = false
			continue
		}

		if !inDesktopEntry {
			continue
		}

		if strings.HasPrefix(line, "Name=") {
			app.Name = strings.TrimPrefix(line, "Name=")
		} else if strings.HasPrefix(line, "Exec=") {
			app.Exec = strings.TrimPrefix(line, "Exec=")
		} else if strings.HasPrefix(line, "Icon=") {
			app.Icon = strings.TrimPrefix(line, "Icon=")
		} else if strings.HasPrefix(line, "Comment=") {
			app.Comment = strings.TrimPrefix(line, "Comment=")
		} else if strings.HasPrefix(line, "NoDisplay=true") {
			return nil
		} else if strings.HasPrefix(line, "Hidden=true") {
			return nil
		} else if strings.HasPrefix(line, "Type=") && strings.TrimPrefix(line, "Type=") != "Application" {
			return nil
		}
	}

	if app.Name == "" || app.Exec == "" {
		return nil
	}

	return app
}

// ensureDisplayEnvironment tries to detect and set up display environment variables
func ensureDisplayEnvironment() {
	if os.Getenv("WAYLAND_DISPLAY") != "" || os.Getenv("DISPLAY") != "" {
		return
	}

	// Try common XDG_RUNTIME_DIR locations for Wayland sockets
	runtimeDirs := []string{
		os.Getenv("XDG_RUNTIME_DIR"),
		"/run/user/0",
		"/run/user/1000",
	}

	for _, runtimeDir := range runtimeDirs {
		if runtimeDir == "" {
			continue
		}

		matches, _ := filepath.Glob(filepath.Join(runtimeDir, "wayland-*"))
		for _, match := range matches {
			if strings.HasSuffix(match, ".lock") {
				continue
			}
			info, err := os.Stat(match)
			if err != nil {
				continue
			}
			if info.Mode()&os.ModeSocket != 0 {
				os.Setenv("WAYLAND_DISPLAY", filepath.Base(match))
				if os.Getenv("XDG_RUNTIME_DIR") == "" {
					os.Setenv("XDG_RUNTIME_DIR", runtimeDir)
				}
				return
			}
		}
	}

	// Fallback: try X socket
	if _, err := os.Stat("/tmp/.X11-unix/X0"); err == nil {
		os.Setenv("DISPLAY", ":0")
	}
}

// Raven theme - matches raven-wifi
type ravenTheme struct{}

func (t *ravenTheme) Color(name fyne.ThemeColorName, variant fyne.ThemeVariant) color.Color {
	switch name {
	case theme.ColorNameBackground:
		return color.NRGBA{R: 30, G: 30, B: 35, A: 255}
	case theme.ColorNameForeground:
		return color.NRGBA{R: 230, G: 230, B: 230, A: 255}
	case theme.ColorNamePrimary:
		return color.NRGBA{R: 0, G: 150, B: 136, A: 255} // Teal
	case theme.ColorNameButton:
		return color.NRGBA{R: 50, G: 50, B: 55, A: 255}
	case theme.ColorNameInputBackground:
		return color.NRGBA{R: 40, G: 40, B: 45, A: 255}
	case theme.ColorNamePlaceHolder:
		return color.NRGBA{R: 128, G: 128, B: 128, A: 255}
	}
	return theme.DefaultTheme().Color(name, variant)
}

func (t *ravenTheme) Font(style fyne.TextStyle) fyne.Resource {
	return theme.DefaultTheme().Font(style)
}

func (t *ravenTheme) Icon(name fyne.ThemeIconName) fyne.Resource {
	return theme.DefaultTheme().Icon(name)
}

func (t *ravenTheme) Size(name fyne.ThemeSizeName) float32 {
	switch name {
	case theme.SizeNamePadding:
		return 8
	case theme.SizeNameText:
		return 14
	}
	return theme.DefaultTheme().Size(name)
}

// Dummy canvas rectangle for spacing
var _ fyne.CanvasObject = (*canvas.Rectangle)(nil)
