package fuzzy

import (
	"bufio"
	"os"
	"os/exec"
	"path/filepath"
	"sort"
	"strings"
	"sync"

	"github.com/diamondburned/gotk4/pkg/gdk/v4"
	"github.com/diamondburned/gotk4/pkg/gtk/v4"
)

// ResultType represents the type of search result
type ResultType int

const (
	ResultTypeApp ResultType = iota
	ResultTypeFile
	ResultTypeCommand
)

// Result represents a single search result
type Result struct {
	Name        string
	Description string
	Path        string
	Exec        string
	Icon        string
	Type        ResultType
	Score       int
}

// PinCallback is called when an app should be pinned to the desktop
type PinCallback func(name, exec, icon string)

// Finder provides a fuzzy search interface for apps, files, and commands
type Finder struct {
	parentWindow *gtk.Window
	window       *gtk.Window
	searchEntry  *gtk.Entry
	resultsList  *gtk.ListBox
	results      []Result
	allApps      []Result
	allCommands  []Result
	pinMode      bool
	mutex        sync.Mutex
	selectedIdx  int
	pinCallback  PinCallback
}

// New creates a new fuzzy finder instance
func New(parent *gtk.Window, pinCallback PinCallback) *Finder {
	f := &Finder{
		parentWindow: parent,
		pinCallback:  pinCallback,
	}
	f.buildIndex()
	f.createUI()
	return f
}

func (f *Finder) buildIndex() {
	f.allApps = []Result{}
	f.allCommands = []Result{}

	// Index applications from .desktop files
	appDirs := []string{
		"/usr/share/applications",
		"/usr/local/share/applications",
		filepath.Join(os.Getenv("HOME"), ".local/share/applications"),
	}

	for _, dir := range appDirs {
		files, _ := filepath.Glob(filepath.Join(dir, "*.desktop"))
		for _, file := range files {
			app := f.parseDesktopFile(file)
			if app.Name != "" && !app.isHidden() {
				f.allApps = append(f.allApps, app)
			}
		}
	}

	// Index commands from PATH
	pathEnv := os.Getenv("PATH")
	paths := strings.Split(pathEnv, ":")
	seenCommands := make(map[string]bool)

	for _, p := range paths {
		entries, err := os.ReadDir(p)
		if err != nil {
			continue
		}
		for _, entry := range entries {
			if entry.IsDir() {
				continue
			}
			name := entry.Name()
			if seenCommands[name] {
				continue
			}
			info, err := entry.Info()
			if err != nil {
				continue
			}
			// Check if executable
			if info.Mode()&0111 != 0 {
				seenCommands[name] = true
				f.allCommands = append(f.allCommands, Result{
					Name:        name,
					Description: "Command",
					Path:        filepath.Join(p, name),
					Exec:        name,
					Icon:        "application-x-executable",
					Type:        ResultTypeCommand,
				})
			}
		}
	}
}

func (f *Finder) parseDesktopFile(path string) Result {
	result := Result{
		Type: ResultTypeApp,
		Path: path,
	}

	file, err := os.Open(path)
	if err != nil {
		return result
	}
	defer file.Close()

	hidden := false
	noDisplay := false
	scanner := bufio.NewScanner(file)
	for scanner.Scan() {
		line := strings.TrimSpace(scanner.Text())
		if strings.HasPrefix(line, "Name=") && result.Name == "" {
			result.Name = strings.TrimPrefix(line, "Name=")
		} else if strings.HasPrefix(line, "Comment=") && result.Description == "" {
			result.Description = strings.TrimPrefix(line, "Comment=")
		} else if strings.HasPrefix(line, "Exec=") {
			execCmd := strings.TrimPrefix(line, "Exec=")
			// Remove field codes
			execCmd = strings.ReplaceAll(execCmd, "%f", "")
			execCmd = strings.ReplaceAll(execCmd, "%F", "")
			execCmd = strings.ReplaceAll(execCmd, "%u", "")
			execCmd = strings.ReplaceAll(execCmd, "%U", "")
			execCmd = strings.ReplaceAll(execCmd, "%d", "")
			execCmd = strings.ReplaceAll(execCmd, "%D", "")
			execCmd = strings.ReplaceAll(execCmd, "%n", "")
			execCmd = strings.ReplaceAll(execCmd, "%N", "")
			execCmd = strings.ReplaceAll(execCmd, "%i", "")
			execCmd = strings.ReplaceAll(execCmd, "%c", "")
			execCmd = strings.ReplaceAll(execCmd, "%k", "")
			result.Exec = strings.TrimSpace(execCmd)
		} else if strings.HasPrefix(line, "Icon=") {
			result.Icon = strings.TrimPrefix(line, "Icon=")
		} else if strings.HasPrefix(line, "Hidden=true") {
			hidden = true
		} else if strings.HasPrefix(line, "NoDisplay=true") {
			noDisplay = true
		}
	}

	if hidden || noDisplay {
		result.Name = "" // Mark as invalid
	}

	if result.Description == "" {
		result.Description = "Application"
	}

	return result
}

func (r *Result) isHidden() bool {
	return r.Name == ""
}

func (f *Finder) createUI() {
	f.window = gtk.NewWindow()
	f.window.SetTitle("Fuzzy Finder")
	f.window.SetDefaultSize(600, 400)
	f.window.SetDecorated(false)
	f.window.SetModal(true)

	// Apply CSS
	css := `
		.fuzzy-window {
			background-color: rgba(26, 35, 50, 0.95);
			border-radius: 12px;
			border: 1px solid #333;
		}
		.fuzzy-entry {
			background-color: #0b0f14;
			color: #e0e0e0;
			border: 1px solid #333;
			border-radius: 8px;
			padding: 12px;
			font-size: 16px;
		}
		.fuzzy-entry:focus {
			border-color: #009688;
		}
		.fuzzy-list {
			background-color: transparent;
		}
		.fuzzy-list row {
			padding: 8px 12px;
			border-radius: 6px;
			margin: 2px 4px;
		}
		.fuzzy-list row:selected {
			background-color: rgba(0, 150, 136, 0.4);
		}
		.fuzzy-list row:hover {
			background-color: rgba(255, 255, 255, 0.1);
		}
		.result-name {
			color: #e0e0e0;
			font-weight: bold;
			font-size: 14px;
		}
		.result-desc {
			color: #888;
			font-size: 12px;
		}
		.result-type {
			color: #009688;
			font-size: 10px;
			padding: 2px 6px;
			border-radius: 4px;
			background-color: rgba(0, 150, 136, 0.2);
		}
	`
	provider := gtk.NewCSSProvider()
	provider.LoadFromString(css)
	display := gdk.DisplayGetDefault()
	gtk.StyleContextAddProviderForDisplay(display, provider, gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)

	// Main container
	mainBox := gtk.NewBox(gtk.OrientationVertical, 8)
	mainBox.AddCSSClass("fuzzy-window")
	mainBox.SetMarginTop(16)
	mainBox.SetMarginBottom(16)
	mainBox.SetMarginStart(16)
	mainBox.SetMarginEnd(16)

	// Search entry
	f.searchEntry = gtk.NewEntry()
	f.searchEntry.AddCSSClass("fuzzy-entry")
	f.searchEntry.SetPlaceholderText("Search apps, files, commands...")
	mainBox.Append(f.searchEntry)

	// Results list in scrolled window
	scrolled := gtk.NewScrolledWindow()
	scrolled.SetVExpand(true)
	scrolled.SetPolicy(gtk.PolicyNever, gtk.PolicyAutomatic)

	f.resultsList = gtk.NewListBox()
	f.resultsList.AddCSSClass("fuzzy-list")
	f.resultsList.SetSelectionMode(gtk.SelectionSingle)
	scrolled.SetChild(f.resultsList)
	mainBox.Append(scrolled)

	f.window.SetChild(mainBox)

	// Connect signals
	f.searchEntry.ConnectChanged(func() {
		f.search(f.searchEntry.Text())
	})

	// Handle keyboard navigation
	keyController := gtk.NewEventControllerKey()
	keyController.ConnectKeyPressed(func(keyval, keycode uint, state gdk.ModifierType) bool {
		switch keyval {
		case gdk.KEY_Escape:
			f.Hide()
			return true
		case gdk.KEY_Return, gdk.KEY_KP_Enter:
			f.activateSelected()
			return true
		case gdk.KEY_Down:
			f.selectNext()
			return true
		case gdk.KEY_Up:
			f.selectPrev()
			return true
		}
		return false
	})
	f.window.AddController(keyController)

	// Handle row activation
	f.resultsList.ConnectRowActivated(func(row *gtk.ListBoxRow) {
		idx := row.Index()
		if idx >= 0 && idx < len(f.results) {
			f.activateResult(f.results[idx])
		}
	})
}

// Show displays the fuzzy finder window
func (f *Finder) Show(pinMode bool) {
	f.pinMode = pinMode
	f.searchEntry.SetText("")
	f.selectedIdx = 0

	if pinMode {
		f.searchEntry.SetPlaceholderText("Search app to pin...")
	} else {
		f.searchEntry.SetPlaceholderText("Search apps, files, commands...")
	}

	// Show initial results (all apps)
	f.search("")

	if f.parentWindow != nil {
		f.window.SetTransientFor(f.parentWindow)
	}
	f.window.Present()
	f.searchEntry.GrabFocus()
}

// Hide hides the fuzzy finder window
func (f *Finder) Hide() {
	f.window.Hide()
}

func (f *Finder) search(query string) {
	f.mutex.Lock()
	defer f.mutex.Unlock()

	f.results = []Result{}
	query = strings.ToLower(strings.TrimSpace(query))

	// Search applications
	for _, app := range f.allApps {
		score := f.fuzzyMatch(query, app.Name)
		if score > 0 {
			appCopy := app
			appCopy.Score = score + 100 // Boost apps
			f.results = append(f.results, appCopy)
		}
	}

	// If not in pin mode, also search files and commands
	if !f.pinMode {
		// Search commands
		for _, cmd := range f.allCommands {
			score := f.fuzzyMatch(query, cmd.Name)
			if score > 0 {
				cmdCopy := cmd
				cmdCopy.Score = score
				f.results = append(f.results, cmdCopy)
			}
		}

		// Search files if query is not empty
		if query != "" {
			f.searchFiles(query)
		}
	}

	// Sort by score
	sort.Slice(f.results, func(i, j int) bool {
		return f.results[i].Score > f.results[j].Score
	})

	// Limit results
	if len(f.results) > 50 {
		f.results = f.results[:50]
	}

	f.updateResultsList()
}

func (f *Finder) fuzzyMatch(query, target string) int {
	if query == "" {
		return 1 // Show all when query is empty
	}

	target = strings.ToLower(target)
	query = strings.ToLower(query)

	// Exact match
	if target == query {
		return 100
	}

	// Prefix match
	if strings.HasPrefix(target, query) {
		return 80
	}

	// Contains match
	if strings.Contains(target, query) {
		return 60
	}

	// Fuzzy character match
	queryIdx := 0
	for i := 0; i < len(target) && queryIdx < len(query); i++ {
		if target[i] == query[queryIdx] {
			queryIdx++
		}
	}

	if queryIdx == len(query) {
		return 40
	}

	return 0
}

func (f *Finder) searchFiles(query string) {
	// Search in common directories
	searchDirs := []string{
		os.Getenv("HOME"),
		filepath.Join(os.Getenv("HOME"), "Documents"),
		filepath.Join(os.Getenv("HOME"), "Downloads"),
		filepath.Join(os.Getenv("HOME"), "Pictures"),
		filepath.Join(os.Getenv("HOME"), "Videos"),
		filepath.Join(os.Getenv("HOME"), "Music"),
		filepath.Join(os.Getenv("HOME"), "Desktop"),
	}

	for _, dir := range searchDirs {
		entries, err := os.ReadDir(dir)
		if err != nil {
			continue
		}

		for _, entry := range entries {
			name := entry.Name()
			if strings.HasPrefix(name, ".") {
				continue // Skip hidden files
			}

			score := f.fuzzyMatch(query, name)
			if score > 0 {
				path := filepath.Join(dir, name)
				icon := "text-x-generic"
				if entry.IsDir() {
					icon = "folder"
				} else {
					// Determine icon by extension
					ext := strings.ToLower(filepath.Ext(name))
					switch ext {
					case ".pdf":
						icon = "application-pdf"
					case ".png", ".jpg", ".jpeg", ".gif", ".webp":
						icon = "image-x-generic"
					case ".mp3", ".wav", ".flac", ".ogg":
						icon = "audio-x-generic"
					case ".mp4", ".mkv", ".avi", ".webm":
						icon = "video-x-generic"
					case ".zip", ".tar", ".gz", ".7z", ".rar":
						icon = "package-x-generic"
					case ".go", ".py", ".js", ".rs", ".c", ".cpp", ".h":
						icon = "text-x-script"
					}
				}

				f.results = append(f.results, Result{
					Name:        name,
					Description: dir,
					Path:        path,
					Exec:        "xdg-open " + path,
					Icon:        icon,
					Type:        ResultTypeFile,
					Score:       score,
				})
			}
		}
	}
}

func (f *Finder) updateResultsList() {
	// Clear existing rows
	for {
		row := f.resultsList.RowAtIndex(0)
		if row == nil {
			break
		}
		f.resultsList.Remove(row)
	}

	// Add new rows
	for _, result := range f.results {
		row := f.createResultRow(result)
		f.resultsList.Append(row)
	}

	// Select first row
	if len(f.results) > 0 {
		f.selectedIdx = 0
		firstRow := f.resultsList.RowAtIndex(0)
		if firstRow != nil {
			f.resultsList.SelectRow(firstRow)
		}
	}
}

func (f *Finder) createResultRow(result Result) *gtk.Box {
	row := gtk.NewBox(gtk.OrientationHorizontal, 12)
	row.SetMarginTop(4)
	row.SetMarginBottom(4)

	// Icon
	image := gtk.NewImageFromIconName(result.Icon)
	image.SetPixelSize(32)
	row.Append(image)

	// Text container
	textBox := gtk.NewBox(gtk.OrientationVertical, 2)
	textBox.SetHExpand(true)

	// Name
	nameLabel := gtk.NewLabel(result.Name)
	nameLabel.AddCSSClass("result-name")
	nameLabel.SetHAlign(gtk.AlignStart)
	textBox.Append(nameLabel)

	// Description
	descLabel := gtk.NewLabel(result.Description)
	descLabel.AddCSSClass("result-desc")
	descLabel.SetHAlign(gtk.AlignStart)
	descLabel.SetEllipsize(3) // PANGO_ELLIPSIZE_END
	textBox.Append(descLabel)

	row.Append(textBox)

	// Type badge
	var typeText string
	switch result.Type {
	case ResultTypeApp:
		typeText = "APP"
	case ResultTypeFile:
		typeText = "FILE"
	case ResultTypeCommand:
		typeText = "CMD"
	}
	typeLabel := gtk.NewLabel(typeText)
	typeLabel.AddCSSClass("result-type")
	row.Append(typeLabel)

	return row
}

func (f *Finder) selectNext() {
	if len(f.results) == 0 {
		return
	}
	f.selectedIdx++
	if f.selectedIdx >= len(f.results) {
		f.selectedIdx = 0
	}
	row := f.resultsList.RowAtIndex(f.selectedIdx)
	if row != nil {
		f.resultsList.SelectRow(row)
	}
}

func (f *Finder) selectPrev() {
	if len(f.results) == 0 {
		return
	}
	f.selectedIdx--
	if f.selectedIdx < 0 {
		f.selectedIdx = len(f.results) - 1
	}
	row := f.resultsList.RowAtIndex(f.selectedIdx)
	if row != nil {
		f.resultsList.SelectRow(row)
	}
}

func (f *Finder) activateSelected() {
	if f.selectedIdx >= 0 && f.selectedIdx < len(f.results) {
		f.activateResult(f.results[f.selectedIdx])
	}
}

func (f *Finder) activateResult(result Result) {
	f.Hide()

	if f.pinMode && result.Type == ResultTypeApp {
		// Pin the app to desktop via callback
		if f.pinCallback != nil {
			f.pinCallback(result.Name, result.Exec, result.Icon)
		}
	} else {
		// Launch the app/command/file
		go func() {
			exec.Command("sh", "-c", result.Exec).Start()
		}()
	}
}
