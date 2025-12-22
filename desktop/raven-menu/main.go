package main

import (
	"bufio"
	"os"
	"os/exec"
	"path/filepath"
	"sort"
	"strings"
	"unsafe"

	"github.com/diamondburned/gotk4/pkg/gdk/v4"
	"github.com/diamondburned/gotk4/pkg/gio/v2"
	"github.com/diamondburned/gotk4/pkg/gtk/v4"
)

/*
#cgo pkg-config: gtk4-layer-shell-0 gtk4
#include <gtk4-layer-shell.h>
#include <gtk/gtk.h>

void init_menu_layer_shell(GtkWidget *window) {
    gtk_layer_init_for_window(GTK_WINDOW(window));
    gtk_layer_set_layer(GTK_WINDOW(window), GTK_LAYER_SHELL_LAYER_OVERLAY);
    gtk_layer_set_keyboard_mode(GTK_WINDOW(window), GTK_LAYER_SHELL_KEYBOARD_MODE_EXCLUSIVE);
    gtk_layer_set_anchor(GTK_WINDOW(window), GTK_LAYER_SHELL_EDGE_TOP, TRUE);
    gtk_layer_set_anchor(GTK_WINDOW(window), GTK_LAYER_SHELL_EDGE_LEFT, TRUE);
    gtk_layer_set_anchor(GTK_WINDOW(window), GTK_LAYER_SHELL_EDGE_BOTTOM, TRUE);
    gtk_layer_set_margin(GTK_WINDOW(window), GTK_LAYER_SHELL_EDGE_TOP, 36);
    gtk_layer_set_margin(GTK_WINDOW(window), GTK_LAYER_SHELL_EDGE_LEFT, 0);
    gtk_layer_set_margin(GTK_WINDOW(window), GTK_LAYER_SHELL_EDGE_BOTTOM, 0);
}
*/
import "C"

// Application represents a launchable app
type Application struct {
	Name     string
	Exec     string
	Icon     string
	Comment  string
	Category string
}

// Category represents an app category
type Category struct {
	Name string
	Icon string
	Apps []Application
}

// RavenMenu is the start menu
type RavenMenu struct {
	app          *gtk.Application
	window       *gtk.Window
	searchEntry  *gtk.Entry
	categoryList *gtk.ListBox
	appList      *gtk.ListBox
	categories   []Category
	allApps      []Application
	currentCat   string
}

func main() {
	app := gtk.NewApplication("org.ravenlinux.menu", gio.ApplicationFlagsNone)

	menu := &RavenMenu{
		app: app,
	}

	app.ConnectActivate(func() {
		menu.activate()
	})

	if code := app.Run(os.Args); code > 0 {
		os.Exit(code)
	}
}

func (m *RavenMenu) activate() {
	m.window = gtk.NewWindow()
	m.window.SetTitle("Raven Menu")
	m.window.SetDefaultSize(350, 500)
	m.window.SetDecorated(false)

	// Load applications
	m.loadApplications()

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

	// Close when clicking outside (focus lost)
	focusController := gtk.NewEventControllerFocus()
	focusController.ConnectLeave(func() {
		// Don't close immediately - might be clicking on a button
	})
	m.window.AddController(focusController)

	m.window.SetApplication(m.app)
	m.window.Present()

	// Focus search entry
	m.searchEntry.GrabFocus()
}

func (m *RavenMenu) initLayerShell() {
	obj := m.window.Object
	if obj != nil {
		ptr := obj.Native()
		C.init_menu_layer_shell((*C.GtkWidget)(unsafe.Pointer(ptr)))
	}
}

func (m *RavenMenu) applyCSS() {
	css := `
		window {
			background-color: #0f1720;
			border-right: 1px solid #333;
		}
		.menu-header {
			background-color: #1a2332;
			padding: 12px;
		}
		.menu-header label {
			font-size: 18px;
			font-weight: bold;
			color: #00bfa5;
		}
		entry {
			background-color: #1a2332;
			border: 1px solid #333;
			border-radius: 6px;
			padding: 8px 12px;
			color: #e0e0e0;
			font-size: 14px;
		}
		entry:focus {
			border-color: #009688;
		}
		.category-list {
			background-color: transparent;
		}
		.category-list row {
			padding: 8px 12px;
			border-radius: 4px;
			margin: 2px 4px;
		}
		.category-list row:selected {
			background-color: #009688;
		}
		.category-list row:hover:not(:selected) {
			background-color: rgba(255, 255, 255, 0.05);
		}
		.category-list row label {
			color: #e0e0e0;
			font-size: 13px;
		}
		.app-list {
			background-color: transparent;
		}
		.app-list row {
			padding: 8px 12px;
			border-radius: 4px;
			margin: 2px 4px;
		}
		.app-list row:selected {
			background-color: #009688;
		}
		.app-list row:hover:not(:selected) {
			background-color: rgba(255, 255, 255, 0.05);
		}
		.app-name {
			color: #e0e0e0;
			font-size: 13px;
			font-weight: bold;
		}
		.app-desc {
			color: #888;
			font-size: 11px;
		}
		.section-title {
			color: #666;
			font-size: 11px;
			font-weight: bold;
			padding: 8px 12px 4px 12px;
		}
		.power-section {
			background-color: #1a2332;
			padding: 8px;
			border-top: 1px solid #333;
		}
		.power-button {
			background-color: transparent;
			border: none;
			padding: 6px 12px;
			color: #e0e0e0;
			font-size: 12px;
		}
		.power-button:hover {
			background-color: rgba(255, 255, 255, 0.1);
		}
		.power-button.shutdown:hover {
			background-color: rgba(244, 67, 54, 0.3);
		}
	`

	provider := gtk.NewCSSProvider()
	provider.LoadFromString(css)
	display := gdk.DisplayGetDefault()
	gtk.StyleContextAddProviderForDisplay(display, provider, gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)
}

func (m *RavenMenu) createUI() *gtk.Box {
	mainBox := gtk.NewBox(gtk.OrientationVertical, 0)

	// Header with title
	header := gtk.NewBox(gtk.OrientationHorizontal, 8)
	header.AddCSSClass("menu-header")
	titleLabel := gtk.NewLabel("Raven")
	header.Append(titleLabel)
	mainBox.Append(header)

	// Search entry
	searchBox := gtk.NewBox(gtk.OrientationHorizontal, 0)
	searchBox.SetMarginStart(8)
	searchBox.SetMarginEnd(8)
	searchBox.SetMarginTop(8)
	searchBox.SetMarginBottom(8)

	m.searchEntry = gtk.NewEntry()
	m.searchEntry.SetPlaceholderText("Search applications...")
	m.searchEntry.SetHExpand(true)
	m.searchEntry.ConnectChanged(func() {
		m.filterApps(m.searchEntry.Text())
	})
	searchBox.Append(m.searchEntry)
	mainBox.Append(searchBox)

	// Main content area with categories and apps
	contentBox := gtk.NewBox(gtk.OrientationHorizontal, 0)
	contentBox.SetVExpand(true)

	// Categories sidebar
	catScroll := gtk.NewScrolledWindow()
	catScroll.SetPolicy(gtk.PolicyNever, gtk.PolicyAutomatic)
	catScroll.SetSizeRequest(120, -1)

	m.categoryList = gtk.NewListBox()
	m.categoryList.AddCSSClass("category-list")
	m.categoryList.SetSelectionMode(gtk.SelectionSingle)
	m.categoryList.ConnectRowSelected(func(row *gtk.ListBoxRow) {
		if row != nil {
			idx := row.Index()
			if idx >= 0 && idx < len(m.categories) {
				m.currentCat = m.categories[idx].Name
				m.showCategory(m.currentCat)
			}
		}
	})

	// Add categories
	for _, cat := range m.categories {
		row := gtk.NewListBoxRow()
		label := gtk.NewLabel(cat.Name)
		label.SetHAlign(gtk.AlignStart)
		row.SetChild(label)
		m.categoryList.Append(row)
	}

	catScroll.SetChild(m.categoryList)
	contentBox.Append(catScroll)

	// Separator
	sep := gtk.NewSeparator(gtk.OrientationVertical)
	contentBox.Append(sep)

	// Apps list
	appScroll := gtk.NewScrolledWindow()
	appScroll.SetPolicy(gtk.PolicyNever, gtk.PolicyAutomatic)
	appScroll.SetHExpand(true)

	m.appList = gtk.NewListBox()
	m.appList.AddCSSClass("app-list")
	m.appList.SetSelectionMode(gtk.SelectionSingle)
	m.appList.ConnectRowActivated(func(row *gtk.ListBoxRow) {
		idx := row.Index()
		apps := m.getVisibleApps()
		if idx >= 0 && idx < len(apps) {
			m.launchApp(apps[idx].Exec)
		}
	})

	appScroll.SetChild(m.appList)
	contentBox.Append(appScroll)

	mainBox.Append(contentBox)

	// Power section at bottom
	powerBox := gtk.NewBox(gtk.OrientationHorizontal, 4)
	powerBox.AddCSSClass("power-section")
	powerBox.SetHAlign(gtk.AlignEnd)

	logoutBtn := gtk.NewButton()
	logoutBtn.SetLabel("Logout")
	logoutBtn.AddCSSClass("power-button")
	logoutBtn.ConnectClicked(func() {
		m.window.Close()
		// Use hyprctl for Hyprland, fallback to other methods
		exec.Command("sh", "-c", "hyprctl dispatch exit || pkill -TERM Hyprland || loginctl terminate-session self").Start()
	})
	powerBox.Append(logoutBtn)

	rebootBtn := gtk.NewButton()
	rebootBtn.SetLabel("Reboot")
	rebootBtn.AddCSSClass("power-button")
	rebootBtn.ConnectClicked(func() {
		m.window.Close()
		exec.Command("sh", "-c", "systemctl reboot || reboot").Start()
	})
	powerBox.Append(rebootBtn)

	shutdownBtn := gtk.NewButton()
	shutdownBtn.SetLabel("Shutdown")
	shutdownBtn.AddCSSClass("power-button")
	shutdownBtn.AddCSSClass("shutdown")
	shutdownBtn.ConnectClicked(func() {
		m.window.Close()
		exec.Command("sh", "-c", "systemctl poweroff || poweroff").Start()
	})
	powerBox.Append(shutdownBtn)

	mainBox.Append(powerBox)

	// Show all apps initially
	m.showCategory("All")

	// Select first category
	if firstRow := m.categoryList.RowAtIndex(0); firstRow != nil {
		m.categoryList.SelectRow(firstRow)
	}

	return mainBox
}

func (m *RavenMenu) getVisibleApps() []Application {
	if m.currentCat == "All" || m.currentCat == "" {
		return m.allApps
	}
	for _, cat := range m.categories {
		if cat.Name == m.currentCat {
			return cat.Apps
		}
	}
	return m.allApps
}

func (m *RavenMenu) showCategory(name string) {
	// Clear existing
	for {
		row := m.appList.RowAtIndex(0)
		if row == nil {
			break
		}
		m.appList.Remove(row)
	}

	var apps []Application
	if name == "All" {
		apps = m.allApps
	} else {
		for _, cat := range m.categories {
			if cat.Name == name {
				apps = cat.Apps
				break
			}
		}
	}

	for _, app := range apps {
		row := m.createAppRow(app)
		m.appList.Append(row)
	}
}

func (m *RavenMenu) createAppRow(app Application) *gtk.ListBoxRow {
	row := gtk.NewListBoxRow()

	box := gtk.NewBox(gtk.OrientationVertical, 2)
	box.SetMarginTop(4)
	box.SetMarginBottom(4)

	nameLabel := gtk.NewLabel(app.Name)
	nameLabel.AddCSSClass("app-name")
	nameLabel.SetHAlign(gtk.AlignStart)
	box.Append(nameLabel)

	if app.Comment != "" {
		descLabel := gtk.NewLabel(app.Comment)
		descLabel.AddCSSClass("app-desc")
		descLabel.SetHAlign(gtk.AlignStart)
		descLabel.SetEllipsize(3) // PANGO_ELLIPSIZE_END
		box.Append(descLabel)
	}

	row.SetChild(box)
	return row
}

func (m *RavenMenu) filterApps(query string) {
	query = strings.ToLower(strings.TrimSpace(query))

	// Clear list
	for {
		row := m.appList.RowAtIndex(0)
		if row == nil {
			break
		}
		m.appList.Remove(row)
	}

	if query == "" {
		m.showCategory(m.currentCat)
		return
	}

	// Filter all apps
	for _, app := range m.allApps {
		if strings.Contains(strings.ToLower(app.Name), query) ||
			strings.Contains(strings.ToLower(app.Comment), query) {
			row := m.createAppRow(app)
			m.appList.Append(row)
		}
	}
}

func (m *RavenMenu) launchApp(cmd string) {
	// Clean exec command
	cmd = strings.ReplaceAll(cmd, "%f", "")
	cmd = strings.ReplaceAll(cmd, "%F", "")
	cmd = strings.ReplaceAll(cmd, "%u", "")
	cmd = strings.ReplaceAll(cmd, "%U", "")
	cmd = strings.TrimSpace(cmd)

	go func() {
		exec.Command("sh", "-c", cmd).Start()
	}()

	m.window.Close()
}

func (m *RavenMenu) loadApplications() {
	// Define categories
	categoryMap := map[string]*Category{
		"All":         {Name: "All", Apps: []Application{}},
		"System":      {Name: "System", Apps: []Application{}},
		"Utilities":   {Name: "Utilities", Apps: []Application{}},
		"Development": {Name: "Development", Apps: []Application{}},
		"Network":     {Name: "Network", Apps: []Application{}},
		"Graphics":    {Name: "Graphics", Apps: []Application{}},
		"Multimedia":  {Name: "Multimedia", Apps: []Application{}},
		"Office":      {Name: "Office", Apps: []Application{}},
		"Other":       {Name: "Other", Apps: []Application{}},
	}

	// Add built-in Raven apps
	ravenApps := []Application{
		{Name: "Raven Terminal", Exec: "raven-terminal", Comment: "Terminal emulator", Category: "System"},
		{Name: "Raven WiFi", Exec: "raven-wifi", Comment: "WiFi network manager", Category: "Network"},
		{Name: "Raven Installer", Exec: "raven-installer", Comment: "Install RavenLinux", Category: "System"},
		{Name: "Raven Launcher", Exec: "raven-launcher", Comment: "Application launcher", Category: "Utilities"},
		{Name: "File Manager", Exec: "raven-terminal -e ranger", Comment: "Browse files with ranger", Category: "System"},
		{Name: "System Monitor", Exec: "raven-terminal -e htop", Comment: "Process viewer", Category: "System"},
		{Name: "Text Editor", Exec: "raven-terminal -e nvim", Comment: "Neovim text editor", Category: "Development"},
	}

	for _, app := range ravenApps {
		m.allApps = append(m.allApps, app)
		if cat, ok := categoryMap[app.Category]; ok {
			cat.Apps = append(cat.Apps, app)
		}
	}

	// Load .desktop files
	desktopDirs := []string{
		"/usr/share/applications",
		"/usr/local/share/applications",
		filepath.Join(os.Getenv("HOME"), ".local/share/applications"),
	}

	seen := make(map[string]bool)
	for _, app := range ravenApps {
		seen[app.Name] = true
	}

	for _, dir := range desktopDirs {
		files, _ := filepath.Glob(filepath.Join(dir, "*.desktop"))
		for _, file := range files {
			app := parseDesktopFile(file)
			if app != nil && !seen[app.Name] {
				seen[app.Name] = true
				m.allApps = append(m.allApps, *app)

				cat := mapCategory(app.Category)
				if c, ok := categoryMap[cat]; ok {
					c.Apps = append(c.Apps, *app)
				} else {
					categoryMap["Other"].Apps = append(categoryMap["Other"].Apps, *app)
				}
			}
		}
	}

	// Sort apps
	sort.Slice(m.allApps, func(i, j int) bool {
		return strings.ToLower(m.allApps[i].Name) < strings.ToLower(m.allApps[j].Name)
	})

	// Build categories list
	categoryOrder := []string{"All", "System", "Utilities", "Network", "Development", "Graphics", "Multimedia", "Office", "Other"}
	for _, name := range categoryOrder {
		if cat, ok := categoryMap[name]; ok {
			if name == "All" || len(cat.Apps) > 0 {
				sort.Slice(cat.Apps, func(i, j int) bool {
					return strings.ToLower(cat.Apps[i].Name) < strings.ToLower(cat.Apps[j].Name)
				})
				m.categories = append(m.categories, *cat)
			}
		}
	}

	// Set All category apps
	categoryMap["All"].Apps = m.allApps
}

func mapCategory(cat string) string {
	cat = strings.ToLower(cat)
	switch {
	case strings.Contains(cat, "system"), strings.Contains(cat, "settings"):
		return "System"
	case strings.Contains(cat, "utility"), strings.Contains(cat, "accessories"):
		return "Utilities"
	case strings.Contains(cat, "development"), strings.Contains(cat, "programming"):
		return "Development"
	case strings.Contains(cat, "network"), strings.Contains(cat, "internet"):
		return "Network"
	case strings.Contains(cat, "graphics"), strings.Contains(cat, "image"):
		return "Graphics"
	case strings.Contains(cat, "audio"), strings.Contains(cat, "video"), strings.Contains(cat, "multimedia"):
		return "Multimedia"
	case strings.Contains(cat, "office"):
		return "Office"
	default:
		return "Other"
	}
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

		if strings.HasPrefix(line, "Name=") && app.Name == "" {
			app.Name = strings.TrimPrefix(line, "Name=")
		} else if strings.HasPrefix(line, "Exec=") {
			app.Exec = strings.TrimPrefix(line, "Exec=")
		} else if strings.HasPrefix(line, "Icon=") {
			app.Icon = strings.TrimPrefix(line, "Icon=")
		} else if strings.HasPrefix(line, "Comment=") && app.Comment == "" {
			app.Comment = strings.TrimPrefix(line, "Comment=")
		} else if strings.HasPrefix(line, "Categories=") {
			app.Category = strings.TrimPrefix(line, "Categories=")
		} else if line == "NoDisplay=true" || line == "Hidden=true" {
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
