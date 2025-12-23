package main

import (
	"context"
	"os"
	"os/exec"
	"path/filepath"
	"sync"

	"raven-file-manager/pkg/clipboard"
	"raven-file-manager/pkg/config"
	"raven-file-manager/pkg/css"
	"raven-file-manager/pkg/fileview"
	"raven-file-manager/pkg/filter"
	"raven-file-manager/pkg/navigation"
	"raven-file-manager/pkg/preview"
	"raven-file-manager/pkg/search"

	"github.com/diamondburned/gotk4/pkg/gdk/v4"
	"github.com/diamondburned/gotk4/pkg/gio/v2"
	"github.com/diamondburned/gotk4/pkg/glib/v2"
	"github.com/diamondburned/gotk4/pkg/gtk/v4"
)

// FileManager is the main application struct
type FileManager struct {
	app    *gtk.Application
	window *gtk.Window

	// Settings
	settings config.Settings

	// Navigation state
	currentPath string
	history     *navigation.History

	// UI Components
	headerBar     *gtk.Box
	locationEntry *gtk.Entry
	searchEntry   *gtk.Entry
	backBtn       *gtk.Button
	forwardBtn    *gtk.Button
	upBtn         *gtk.Button
	homeBtn       *gtk.Button
	sidebarBox    *gtk.Box
	sidebarList   *gtk.ListBox
	mainPaned     *gtk.Paned
	contentPaned  *gtk.Paned
	fileListBox   *gtk.ListBox
	fileFlowBox   *gtk.FlowBox
	fileScroll    *gtk.ScrolledWindow
	previewPane   *gtk.Box
	statusBar     *gtk.Box
	statusLabel   *gtk.Label
	statusRight   *gtk.Label
	filterPanel   *gtk.Box

	// Search state
	searchActive        bool
	contentSearchActive bool

	// File state
	currentFiles  []fileview.FileEntry
	selectedFiles []fileview.FileEntry
	mu            sync.RWMutex

	// Components
	searchEngine *search.Engine
	previewPanel *preview.Panel
	filterState  *filter.State
	clipboard    *clipboard.Manager
}

func main() {
	app := gtk.NewApplication("org.ravenlinux.filemanager", gio.ApplicationFlagsNone)

	fm := &FileManager{
		app: app,
	}

	app.ConnectActivate(func() {
		fm.activate()
	})

	if code := app.Run(os.Args); code > 0 {
		os.Exit(code)
	}
}

func (fm *FileManager) activate() {
	// Load settings
	fm.settings = config.LoadSettings()

	// Initialize state
	fm.history = navigation.NewHistory()
	fm.filterState = filter.NewState()
	fm.searchEngine = search.NewEngine()
	fm.previewPanel = preview.NewPanel()
	fm.clipboard = clipboard.NewManager()

	// Set initial path
	home := os.Getenv("HOME")
	if home == "" {
		home = "/"
	}
	fm.currentPath = home

	// Create window
	fm.window = gtk.NewWindow()
	fm.window.SetTitle("Raven Files")
	fm.window.SetDefaultSize(fm.settings.WindowWidth, fm.settings.WindowHeight)

	// Apply CSS
	fm.applyCSS()

	// Create UI
	content := fm.createUI()
	fm.window.SetChild(content)

	// Setup keyboard shortcuts
	fm.setupKeyboardShortcuts()

	// Load initial directory
	fm.navigateTo(fm.currentPath)

	fm.window.SetApplication(fm.app)
	fm.window.Present()
}

func (fm *FileManager) applyCSS() {
	provider := gtk.NewCSSProvider()
	provider.LoadFromString(css.FileManagerCSS)
	display := gdk.DisplayGetDefault()
	gtk.StyleContextAddProviderForDisplay(display, provider, gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)
}

func (fm *FileManager) createUI() *gtk.Box {
	mainBox := gtk.NewBox(gtk.OrientationVertical, 0)

	// Header bar
	fm.headerBar = fm.createHeaderBar()
	mainBox.Append(fm.headerBar)

	// Filter panel
	fm.filterPanel = fm.createFilterPanel()
	fm.filterPanel.SetVisible(false)
	mainBox.Append(fm.filterPanel)

	// Main content
	contentBox := gtk.NewBox(gtk.OrientationHorizontal, 0)
	contentBox.SetVExpand(true)

	// Sidebar
	fm.sidebarBox = fm.createSidebar()
	contentBox.Append(fm.sidebarBox)

	// Content paned
	fm.contentPaned = gtk.NewPaned(gtk.OrientationHorizontal)
	fm.contentPaned.SetHExpand(true)

	// File area
	fileArea := fm.createFileArea()
	fm.contentPaned.SetStartChild(fileArea)

	// Preview pane
	fm.previewPane = fm.createPreviewPane()
	fm.contentPaned.SetEndChild(fm.previewPane)
	fm.contentPaned.SetPosition(800)
	fm.previewPane.SetVisible(fm.settings.ShowPreview)

	contentBox.Append(fm.contentPaned)
	mainBox.Append(contentBox)

	// Status bar
	fm.statusBar = fm.createStatusBar()
	if fm.settings.ShowStatusBar {
		mainBox.Append(fm.statusBar)
	}

	return mainBox
}

func (fm *FileManager) createHeaderBar() *gtk.Box {
	header := gtk.NewBox(gtk.OrientationHorizontal, 8)
	header.AddCSSClass("header-bar")

	// Navigation buttons
	navBox := gtk.NewBox(gtk.OrientationHorizontal, 4)

	fm.backBtn = gtk.NewButton()
	fm.backBtn.SetIconName("go-previous-symbolic")
	fm.backBtn.AddCSSClass("nav-button")
	fm.backBtn.SetTooltipText("Back (Alt+Left)")
	fm.backBtn.ConnectClicked(func() { fm.goBack() })
	navBox.Append(fm.backBtn)

	fm.forwardBtn = gtk.NewButton()
	fm.forwardBtn.SetIconName("go-next-symbolic")
	fm.forwardBtn.AddCSSClass("nav-button")
	fm.forwardBtn.SetTooltipText("Forward (Alt+Right)")
	fm.forwardBtn.ConnectClicked(func() { fm.goForward() })
	navBox.Append(fm.forwardBtn)

	fm.upBtn = gtk.NewButton()
	fm.upBtn.SetIconName("go-up-symbolic")
	fm.upBtn.AddCSSClass("nav-button")
	fm.upBtn.SetTooltipText("Parent Directory (Alt+Up)")
	fm.upBtn.ConnectClicked(func() { fm.goUp() })
	navBox.Append(fm.upBtn)

	fm.homeBtn = gtk.NewButton()
	fm.homeBtn.SetIconName("go-home-symbolic")
	fm.homeBtn.AddCSSClass("nav-button")
	fm.homeBtn.SetTooltipText("Home (Alt+Home)")
	fm.homeBtn.ConnectClicked(func() { fm.goHome() })
	navBox.Append(fm.homeBtn)

	header.Append(navBox)

	// Location bar
	fm.locationEntry = gtk.NewEntry()
	fm.locationEntry.AddCSSClass("location-bar")
	fm.locationEntry.SetHExpand(true)
	fm.locationEntry.SetPlaceholderText("Enter path...")
	fm.locationEntry.ConnectActivate(func() {
		path := fm.locationEntry.Text()
		if path != "" {
			fm.navigateTo(path)
		}
	})
	header.Append(fm.locationEntry)

	// Search entry
	fm.searchEntry = gtk.NewEntry()
	fm.searchEntry.AddCSSClass("search-entry")
	fm.searchEntry.SetPlaceholderText("Search files...")
	fm.searchEntry.SetIconFromIconName(gtk.EntryIconPrimary, "system-search-symbolic")
	fm.searchEntry.ConnectChanged(func() {
		query := fm.searchEntry.Text()
		if query != "" {
			fm.searchActive = true
			fm.performSearch(query)
		} else {
			fm.searchActive = false
			fm.refresh()
		}
	})
	header.Append(fm.searchEntry)

	// Action buttons
	actionBox := gtk.NewBox(gtk.OrientationHorizontal, 4)

	filterBtn := gtk.NewButton()
	filterBtn.SetIconName("view-more-symbolic")
	filterBtn.AddCSSClass("nav-button")
	filterBtn.SetTooltipText("Filters")
	filterBtn.ConnectClicked(func() {
		fm.filterPanel.SetVisible(!fm.filterPanel.IsVisible())
	})
	actionBox.Append(filterBtn)

	previewBtn := gtk.NewToggleButton()
	previewBtn.SetIconName("view-dual-symbolic")
	previewBtn.AddCSSClass("nav-button")
	previewBtn.SetTooltipText("Toggle Preview (Ctrl+P)")
	previewBtn.SetActive(fm.settings.ShowPreview)
	previewBtn.ConnectClicked(func() {
		fm.togglePreview()
		previewBtn.SetActive(fm.settings.ShowPreview)
	})
	actionBox.Append(previewBtn)

	header.Append(actionBox)

	return header
}

func (fm *FileManager) createSidebar() *gtk.Box {
	sidebar := gtk.NewBox(gtk.OrientationVertical, 0)
	sidebar.AddCSSClass("sidebar")
	sidebar.SetSizeRequest(fm.settings.SidebarWidth, -1)

	scroll := gtk.NewScrolledWindow()
	scroll.SetPolicy(gtk.PolicyNever, gtk.PolicyAutomatic)
	scroll.SetVExpand(true)

	sidebarContent := gtk.NewBox(gtk.OrientationVertical, 0)

	placesLabel := gtk.NewLabel("Places")
	placesLabel.AddCSSClass("sidebar-section")
	placesLabel.SetHAlign(gtk.AlignStart)
	sidebarContent.Append(placesLabel)

	fm.sidebarList = gtk.NewListBox()
	fm.sidebarList.AddCSSClass("sidebar-list")
	fm.sidebarList.SetSelectionMode(gtk.SelectionSingle)
	fm.sidebarList.ConnectRowActivated(func(row *gtk.ListBoxRow) {
		idx := row.Index()
		if idx >= 0 && idx < len(fm.settings.Bookmarks) {
			fm.navigateTo(fm.settings.Bookmarks[idx].Path)
		}
	})

	for _, bookmark := range fm.settings.Bookmarks {
		row := fm.createSidebarRow(bookmark.Name, bookmark.Icon)
		fm.sidebarList.Append(row)
	}

	sidebarContent.Append(fm.sidebarList)
	scroll.SetChild(sidebarContent)
	sidebar.Append(scroll)

	return sidebar
}

func (fm *FileManager) createSidebarRow(name, iconName string) *gtk.ListBoxRow {
	row := gtk.NewListBoxRow()

	box := gtk.NewBox(gtk.OrientationHorizontal, 8)
	box.SetMarginStart(8)
	box.SetMarginEnd(8)
	box.SetMarginTop(4)
	box.SetMarginBottom(4)

	icon := gtk.NewImageFromIconName(iconName)
	icon.AddCSSClass("sidebar-item-icon")
	box.Append(icon)

	label := gtk.NewLabel(name)
	label.AddCSSClass("sidebar-item")
	label.SetHAlign(gtk.AlignStart)
	box.Append(label)

	row.SetChild(box)
	return row
}

func (fm *FileManager) createFileArea() *gtk.Box {
	fileArea := gtk.NewBox(gtk.OrientationVertical, 0)
	fileArea.SetHExpand(true)
	fileArea.SetVExpand(true)

	fm.fileScroll = gtk.NewScrolledWindow()
	fm.fileScroll.SetPolicy(gtk.PolicyAutomatic, gtk.PolicyAutomatic)
	fm.fileScroll.SetVExpand(true)

	fm.createListView()

	fileArea.Append(fm.fileScroll)

	return fileArea
}

func (fm *FileManager) createListView() {
	fm.fileListBox = gtk.NewListBox()
	fm.fileListBox.AddCSSClass("file-list")
	fm.fileListBox.SetSelectionMode(gtk.SelectionMultiple)
	fm.fileListBox.SetActivateOnSingleClick(false)

	fm.fileListBox.ConnectRowActivated(func(row *gtk.ListBoxRow) {
		idx := row.Index()
		if idx >= 0 && idx < len(fm.currentFiles) {
			fm.openFile(fm.currentFiles[idx])
		}
	})

	fm.fileListBox.ConnectSelectedRowsChanged(func() {
		fm.onSelectionChanged()
	})

	// Add gesture for double-click handling
	gesture := gtk.NewGestureClick()
	gesture.SetButton(1) // Left mouse button
	gesture.ConnectPressed(func(nPress int, x, y float64) {
		if nPress == 2 { // Double-click
			row := fm.fileListBox.RowAtY(int(y))
			if row != nil {
				idx := row.Index()
				if idx >= 0 && idx < len(fm.currentFiles) {
					fm.openFile(fm.currentFiles[idx])
				}
			}
		}
	})
	fm.fileListBox.AddController(gesture)

	fm.fileScroll.SetChild(fm.fileListBox)
}

func (fm *FileManager) createPreviewPane() *gtk.Box {
	previewBox := gtk.NewBox(gtk.OrientationVertical, 0)
	previewBox.AddCSSClass("preview-pane")
	previewBox.SetSizeRequest(fm.settings.PreviewSize, -1)

	header := gtk.NewBox(gtk.OrientationHorizontal, 8)
	header.AddCSSClass("preview-header")

	titleLabel := gtk.NewLabel("Preview")
	titleLabel.AddCSSClass("preview-title")
	titleLabel.SetHAlign(gtk.AlignStart)
	header.Append(titleLabel)

	previewBox.Append(header)

	scroll := gtk.NewScrolledWindow()
	scroll.SetPolicy(gtk.PolicyAutomatic, gtk.PolicyAutomatic)
	scroll.SetVExpand(true)

	content := gtk.NewBox(gtk.OrientationVertical, 8)
	content.AddCSSClass("preview-content")

	placeholder := gtk.NewLabel("Select a file to preview")
	placeholder.AddCSSClass("status-text")
	content.Append(placeholder)

	scroll.SetChild(content)
	previewBox.Append(scroll)

	fm.previewPanel.ContentBox = content

	return previewBox
}

func (fm *FileManager) createFilterPanel() *gtk.Box {
	panel := gtk.NewBox(gtk.OrientationHorizontal, 12)
	panel.AddCSSClass("filter-panel")

	typeLabel := gtk.NewLabel("Type:")
	typeLabel.AddCSSClass("filter-label")
	panel.Append(typeLabel)

	typeStrings := []string{"All Files", "Documents", "Images", "Videos", "Audio", "Archives", "Code"}
	typeModel := gtk.NewStringList(typeStrings)
	typeDropdown := gtk.NewDropDown(typeModel, nil)
	typeDropdown.AddCSSClass("filter-dropdown")
	typeDropdown.Connect("notify::selected", func() {
		idx := typeDropdown.Selected()
		if idx == 0 {
			fm.filterState.FileTypes = nil
		} else {
			fm.filterState.FileTypes = []string{typeStrings[idx]}
		}
		fm.refresh()
	})
	panel.Append(typeDropdown)

	spacer := gtk.NewBox(gtk.OrientationHorizontal, 0)
	spacer.SetHExpand(true)
	panel.Append(spacer)

	clearBtn := gtk.NewButton()
	clearBtn.SetLabel("Clear Filters")
	clearBtn.AddCSSClass("filter-clear")
	clearBtn.ConnectClicked(func() {
		fm.filterState = filter.NewState()
		fm.refresh()
		fm.filterPanel.SetVisible(false)
	})
	panel.Append(clearBtn)

	return panel
}

func (fm *FileManager) createStatusBar() *gtk.Box {
	status := gtk.NewBox(gtk.OrientationHorizontal, 8)
	status.AddCSSClass("status-bar")

	fm.statusLabel = gtk.NewLabel("")
	fm.statusLabel.AddCSSClass("status-text")
	fm.statusLabel.SetHAlign(gtk.AlignStart)
	status.Append(fm.statusLabel)

	spacer := gtk.NewBox(gtk.OrientationHorizontal, 0)
	spacer.SetHExpand(true)
	status.Append(spacer)

	fm.statusRight = gtk.NewLabel("")
	fm.statusRight.AddCSSClass("status-text-right")
	fm.statusRight.SetHAlign(gtk.AlignEnd)
	status.Append(fm.statusRight)

	return status
}

func (fm *FileManager) setupKeyboardShortcuts() {
	keyController := gtk.NewEventControllerKey()
	keyController.ConnectKeyPressed(func(keyval, keycode uint, state gdk.ModifierType) bool {
		ctrl := state&gdk.ControlMask != 0
		shift := state&gdk.ShiftMask != 0
		alt := state&gdk.ModifierType(8) != 0

		switch keyval {
		case gdk.KEY_f:
			if ctrl && shift {
				fm.contentSearchActive = true
				fm.searchEntry.GrabFocus()
				return true
			} else if ctrl {
				fm.searchEntry.GrabFocus()
				return true
			}
		case gdk.KEY_l:
			if ctrl {
				fm.locationEntry.GrabFocus()
				fm.locationEntry.SelectRegion(0, -1)
				return true
			}
		case gdk.KEY_h:
			if ctrl {
				fm.filterState.ShowHidden = !fm.filterState.ShowHidden
				fm.refresh()
				return true
			}
		case gdk.KEY_n:
			if ctrl && shift {
				fm.showNewFolderDialog()
				return true
			}
		case gdk.KEY_F2:
			fm.renameSelected()
			return true
		case gdk.KEY_F5:
			fm.refresh()
			return true
		case gdk.KEY_Delete:
			if shift {
				fm.permanentDelete()
			} else {
				fm.trashSelected()
			}
			return true
		case gdk.KEY_c:
			if ctrl {
				fm.copySelected()
				return true
			}
		case gdk.KEY_x:
			if ctrl {
				fm.cutSelected()
				return true
			}
		case gdk.KEY_v:
			if ctrl {
				fm.paste()
				return true
			}
		case gdk.KEY_a:
			if ctrl {
				fm.fileListBox.SelectAll()
				return true
			}
		case gdk.KEY_Return, gdk.KEY_KP_Enter:
			fm.openSelected()
			return true
		case gdk.KEY_BackSpace:
			fm.goBack()
			return true
		case gdk.KEY_Left:
			if alt {
				fm.goBack()
				return true
			}
		case gdk.KEY_Right:
			if alt {
				fm.goForward()
				return true
			}
		case gdk.KEY_Up:
			if alt {
				fm.goUp()
				return true
			}
		case gdk.KEY_Home:
			if alt {
				fm.goHome()
				return true
			}
		case gdk.KEY_Escape:
			if fm.searchActive {
				fm.searchEntry.SetText("")
				fm.searchActive = false
				fm.contentSearchActive = false
				fm.refresh()
			} else {
				fm.fileListBox.UnselectAll()
			}
			return true
		case gdk.KEY_p:
			if ctrl {
				fm.togglePreview()
				return true
			}
		}

		return false
	})
	fm.window.AddController(keyController)
}

// Navigation methods
func (fm *FileManager) goHome() {
	home := os.Getenv("HOME")
	if home == "" {
		home = "/"
	}
	fm.navigateTo(home)
}

func (fm *FileManager) goUp() {
	parent := fileview.GetParentPath(fm.currentPath)
	if parent != fm.currentPath {
		fm.navigateTo(parent)
	}
}

func (fm *FileManager) goBack() {
	if path := fm.history.Back(); path != "" {
		fm.loadDirectory(path)
		fm.currentPath = path
		fm.updateLocationBar()
	}
}

func (fm *FileManager) goForward() {
	if path := fm.history.Forward(); path != "" {
		fm.loadDirectory(path)
		fm.currentPath = path
		fm.updateLocationBar()
	}
}

func (fm *FileManager) navigateTo(path string) {
	fm.history.Push(path)
	fm.currentPath = path
	fm.loadDirectory(path)
	fm.updateLocationBar()
}

func (fm *FileManager) refresh() {
	fm.loadDirectory(fm.currentPath)
}

func (fm *FileManager) loadDirectory(path string) {
	go func() {
		entries, err := fileview.ReadDirectory(path)
		if err != nil {
			glib.IdleAdd(func() {
				fm.showError("Failed to read directory: " + err.Error())
			})
			return
		}

		filtered := fm.filterState.ApplyFilters(entries)
		sorted := fileview.SortEntries(filtered, fm.settings.SortBy, fm.settings.SortDescending)

		glib.IdleAdd(func() {
			fm.updateFileList(sorted)
			fm.updateStatusBar()
		})
	}()
}

func (fm *FileManager) updateLocationBar() {
	if fm.locationEntry != nil {
		fm.locationEntry.SetText(fm.currentPath)
	}
	fm.updateNavButtons()
}

func (fm *FileManager) updateNavButtons() {
	if fm.backBtn != nil {
		fm.backBtn.SetSensitive(fm.history.CanGoBack())
	}
	if fm.forwardBtn != nil {
		fm.forwardBtn.SetSensitive(fm.history.CanGoForward())
	}
	if fm.upBtn != nil {
		fm.upBtn.SetSensitive(fm.currentPath != "/")
	}
}

func (fm *FileManager) updateFileList(entries []fileview.FileEntry) {
	fm.mu.Lock()
	fm.currentFiles = entries
	fm.mu.Unlock()

	// Remove old list and create new one for better performance
	fm.fileScroll.SetChild(nil)

	fm.fileListBox = gtk.NewListBox()
	fm.fileListBox.AddCSSClass("file-list")
	fm.fileListBox.SetSelectionMode(gtk.SelectionMultiple)
	fm.fileListBox.SetActivateOnSingleClick(false)

	fm.fileListBox.ConnectRowActivated(func(row *gtk.ListBoxRow) {
		idx := row.Index()
		if idx >= 0 && idx < len(fm.currentFiles) {
			fm.openFile(fm.currentFiles[idx])
		}
	})

	fm.fileListBox.ConnectSelectedRowsChanged(func() {
		fm.onSelectionChanged()
	})

	// Add gesture for double-click
	gesture := gtk.NewGestureClick()
	gesture.SetButton(1)
	gesture.ConnectPressed(func(nPress int, x, y float64) {
		if nPress == 2 {
			row := fm.fileListBox.RowAtY(int(y))
			if row != nil {
				idx := row.Index()
				if idx >= 0 && idx < len(fm.currentFiles) {
					fm.openFile(fm.currentFiles[idx])
				}
			}
		}
	})
	fm.fileListBox.AddController(gesture)

	// Add all entries
	for _, entry := range entries {
		row := fm.createFileListRow(entry)
		fm.fileListBox.Append(row)
	}

	fm.fileScroll.SetChild(fm.fileListBox)
}

func (fm *FileManager) createFileListRow(entry fileview.FileEntry) *gtk.ListBoxRow {
	row := gtk.NewListBoxRow()
	row.AddCSSClass("file-row")
	row.SetActivatable(true)

	box := gtk.NewBox(gtk.OrientationHorizontal, 8)
	box.SetMarginStart(8)
	box.SetMarginEnd(8)
	box.SetMarginTop(6)
	box.SetMarginBottom(6)

	iconName := fileview.GetFileIcon(entry)
	icon := gtk.NewImageFromIconName(iconName)
	icon.SetPixelSize(20)
	if entry.IsDir {
		icon.AddCSSClass("file-icon-folder")
	}
	box.Append(icon)

	nameLabel := gtk.NewLabel(entry.Name)
	nameLabel.AddCSSClass("file-name")
	if entry.IsDir {
		nameLabel.AddCSSClass("file-name-folder")
	}
	if entry.IsHidden {
		nameLabel.SetOpacity(0.6)
	}
	nameLabel.SetHAlign(gtk.AlignStart)
	nameLabel.SetHExpand(true)
	nameLabel.SetEllipsize(3)
	nameLabel.SetMaxWidthChars(50)
	box.Append(nameLabel)

	if !entry.IsDir {
		sizeLabel := gtk.NewLabel(fileview.HumanizeSize(entry.Size))
		sizeLabel.AddCSSClass("file-size")
		sizeLabel.SetWidthChars(10)
		box.Append(sizeLabel)
	}

	dateLabel := gtk.NewLabel(fileview.FormatDate(entry.ModTime))
	dateLabel.AddCSSClass("file-date")
	dateLabel.SetWidthChars(12)
	box.Append(dateLabel)

	row.SetChild(box)
	return row
}

func (fm *FileManager) updateStatusBar() {
	if fm.statusLabel == nil {
		return
	}

	fm.mu.RLock()
	defer fm.mu.RUnlock()

	var dirCount, fileCount int
	for _, entry := range fm.currentFiles {
		if entry.IsDir {
			dirCount++
		} else {
			fileCount++
		}
	}

	statusText := ""
	if dirCount > 0 && fileCount > 0 {
		statusText = fileview.Pluralize(dirCount, "folder", "folders") + ", " + fileview.Pluralize(fileCount, "file", "files")
	} else if dirCount > 0 {
		statusText = fileview.Pluralize(dirCount, "folder", "folders")
	} else if fileCount > 0 {
		statusText = fileview.Pluralize(fileCount, "file", "files")
	} else {
		statusText = "Empty folder"
	}

	fm.statusLabel.SetText(statusText)

	freeSpace, totalSpace := fileview.GetDiskSpace(fm.currentPath)
	if totalSpace > 0 {
		fm.statusRight.SetText(fileview.HumanizeSize(freeSpace) + " free of " + fileview.HumanizeSize(totalSpace))
	}
}

func (fm *FileManager) onSelectionChanged() {
	fm.mu.Lock()
	defer fm.mu.Unlock()

	fm.selectedFiles = nil

	if fm.fileListBox != nil {
		selectedRows := fm.fileListBox.SelectedRows()
		for _, row := range selectedRows {
			idx := row.Index()
			if idx >= 0 && idx < len(fm.currentFiles) {
				fm.selectedFiles = append(fm.selectedFiles, fm.currentFiles[idx])
			}
		}
	}

	if len(fm.selectedFiles) == 1 {
		entry := fm.selectedFiles[0]
		fm.previewPanel.ShowPreview(filepath.Join(fm.currentPath, entry.Name), entry)
	}

	if len(fm.selectedFiles) > 0 {
		var totalSize int64
		for _, f := range fm.selectedFiles {
			totalSize += f.Size
		}
		fm.statusLabel.SetText(fileview.Pluralize(len(fm.selectedFiles), "item", "items") + " selected (" + fileview.HumanizeSize(totalSize) + ")")
	} else {
		fm.updateStatusBar()
	}
}

func (fm *FileManager) openFile(entry fileview.FileEntry) {
	if entry.IsDir {
		fm.navigateTo(entry.Path)
		return
	}

	config.AddRecentFile(&fm.settings, entry.Path)

	go func() {
		exec.Command("xdg-open", entry.Path).Start()
	}()
}

func (fm *FileManager) openSelected() {
	fm.mu.RLock()
	if len(fm.selectedFiles) == 0 {
		fm.mu.RUnlock()
		return
	}
	entry := fm.selectedFiles[0]
	fm.mu.RUnlock()

	fm.openFile(entry)
}

func (fm *FileManager) togglePreview() {
	fm.settings.ShowPreview = !fm.settings.ShowPreview
	if fm.previewPane != nil {
		fm.previewPane.SetVisible(fm.settings.ShowPreview)
	}
	config.SaveSettings(fm.settings)
}

// Search
func (fm *FileManager) performSearch(query string) {
	if query == "" {
		fm.searchActive = false
		fm.refresh()
		return
	}

	fm.searchActive = true

	go func() {
		ctx := context.Background()
		var results []search.Result

		if fm.contentSearchActive {
			results = fm.searchEngine.ContentSearch(ctx, fm.currentPath, query, fm.settings.SearchContentMax, 100)
		} else {
			results = fm.searchEngine.Search(ctx, fm.currentPath, query, 200)
		}

		entries := make([]fileview.FileEntry, len(results))
		for i, r := range results {
			entries[i] = r.Entry
		}

		glib.IdleAdd(func() {
			fm.updateFileList(entries)
			if len(results) == 0 {
				fm.statusLabel.SetText("No matches found")
			} else {
				fm.statusLabel.SetText(fileview.Pluralize(len(results), "match", "matches") + " found")
			}
		})
	}()
}

// Clipboard operations
func (fm *FileManager) copySelected() {
	fm.mu.RLock()
	files := make([]string, len(fm.selectedFiles))
	for i, f := range fm.selectedFiles {
		files[i] = f.Path
	}
	fm.mu.RUnlock()

	if len(files) > 0 {
		fm.clipboard.Copy(files)
	}
}

func (fm *FileManager) cutSelected() {
	fm.mu.RLock()
	files := make([]string, len(fm.selectedFiles))
	for i, f := range fm.selectedFiles {
		files[i] = f.Path
	}
	fm.mu.RUnlock()

	if len(files) > 0 {
		fm.clipboard.Cut(files)
	}
}

func (fm *FileManager) paste() {
	if !fm.clipboard.HasFiles() {
		return
	}

	go func() {
		err := fm.clipboard.Paste(fm.currentPath)
		glib.IdleAdd(func() {
			if err != nil {
				fm.showError("Paste failed: " + err.Error())
			}
			fm.refresh()
		})
	}()
}

func (fm *FileManager) trashSelected() {
	fm.mu.RLock()
	files := make([]fileview.FileEntry, len(fm.selectedFiles))
	copy(files, fm.selectedFiles)
	fm.mu.RUnlock()

	if len(files) == 0 {
		return
	}

	go func() {
		err := clipboard.TrashFiles(files)
		glib.IdleAdd(func() {
			if err != nil {
				fm.showError("Trash failed: " + err.Error())
			}
			fm.refresh()
		})
	}()
}

func (fm *FileManager) permanentDelete() {
	fm.mu.RLock()
	files := make([]fileview.FileEntry, len(fm.selectedFiles))
	copy(files, fm.selectedFiles)
	fm.mu.RUnlock()

	if len(files) == 0 {
		return
	}

	go func() {
		err := clipboard.DeleteFiles(files)
		glib.IdleAdd(func() {
			if err != nil {
				fm.showError("Delete failed: " + err.Error())
			}
			fm.refresh()
		})
	}()
}

// Dialogs
func (fm *FileManager) showNewFolderDialog() {
	dialog := gtk.NewDialog()
	dialog.SetTitle("New Folder")
	dialog.SetTransientFor(fm.window)
	dialog.SetModal(true)
	dialog.SetDefaultSize(400, -1)

	content := dialog.ContentArea()
	content.SetMarginTop(16)
	content.SetMarginBottom(16)
	content.SetMarginStart(16)
	content.SetMarginEnd(16)
	content.SetSpacing(12)

	label := gtk.NewLabel("Folder name:")
	label.SetHAlign(gtk.AlignStart)
	content.Append(label)

	entry := gtk.NewEntry()
	entry.SetPlaceholderText("Enter folder name...")
	entry.SetText("New Folder")
	entry.SelectRegion(0, -1)
	content.Append(entry)

	buttonBox := gtk.NewBox(gtk.OrientationHorizontal, 8)
	buttonBox.SetHAlign(gtk.AlignEnd)
	buttonBox.SetMarginTop(16)

	cancelBtn := gtk.NewButton()
	cancelBtn.SetLabel("Cancel")
	cancelBtn.AddCSSClass("cancel")
	cancelBtn.ConnectClicked(func() { dialog.Destroy() })
	buttonBox.Append(cancelBtn)

	createBtn := gtk.NewButton()
	createBtn.SetLabel("Create")
	createBtn.ConnectClicked(func() {
		name := entry.Text()
		if name != "" {
			path := filepath.Join(fm.currentPath, name)
			if err := os.Mkdir(path, 0755); err != nil {
				fm.showError("Failed to create folder: " + err.Error())
			} else {
				fm.refresh()
			}
		}
		dialog.Destroy()
	})
	buttonBox.Append(createBtn)

	content.Append(buttonBox)

	entry.ConnectActivate(func() {
		name := entry.Text()
		if name != "" {
			path := filepath.Join(fm.currentPath, name)
			if err := os.Mkdir(path, 0755); err != nil {
				fm.showError("Failed to create folder: " + err.Error())
			} else {
				fm.refresh()
			}
		}
		dialog.Destroy()
	})

	dialog.Present()
	entry.GrabFocus()
}

func (fm *FileManager) renameSelected() {
	fm.mu.RLock()
	if len(fm.selectedFiles) != 1 {
		fm.mu.RUnlock()
		return
	}
	file := fm.selectedFiles[0]
	fm.mu.RUnlock()

	dialog := gtk.NewDialog()
	dialog.SetTitle("Rename")
	dialog.SetTransientFor(fm.window)
	dialog.SetModal(true)
	dialog.SetDefaultSize(400, -1)

	content := dialog.ContentArea()
	content.SetMarginTop(16)
	content.SetMarginBottom(16)
	content.SetMarginStart(16)
	content.SetMarginEnd(16)
	content.SetSpacing(12)

	label := gtk.NewLabel("New name:")
	label.SetHAlign(gtk.AlignStart)
	content.Append(label)

	entry := gtk.NewEntry()
	entry.SetText(file.Name)
	if !file.IsDir {
		ext := filepath.Ext(file.Name)
		nameWithoutExt := file.Name[:len(file.Name)-len(ext)]
		entry.SelectRegion(0, len(nameWithoutExt))
	} else {
		entry.SelectRegion(0, -1)
	}
	content.Append(entry)

	buttonBox := gtk.NewBox(gtk.OrientationHorizontal, 8)
	buttonBox.SetHAlign(gtk.AlignEnd)
	buttonBox.SetMarginTop(16)

	cancelBtn := gtk.NewButton()
	cancelBtn.SetLabel("Cancel")
	cancelBtn.AddCSSClass("cancel")
	cancelBtn.ConnectClicked(func() { dialog.Destroy() })
	buttonBox.Append(cancelBtn)

	renameBtn := gtk.NewButton()
	renameBtn.SetLabel("Rename")
	renameBtn.ConnectClicked(func() {
		newName := entry.Text()
		if newName != "" && newName != file.Name {
			oldPath := file.Path
			newPath := filepath.Join(filepath.Dir(oldPath), newName)
			if err := os.Rename(oldPath, newPath); err != nil {
				fm.showError("Failed to rename: " + err.Error())
			} else {
				fm.refresh()
			}
		}
		dialog.Destroy()
	})
	buttonBox.Append(renameBtn)

	content.Append(buttonBox)

	entry.ConnectActivate(func() {
		newName := entry.Text()
		if newName != "" && newName != file.Name {
			oldPath := file.Path
			newPath := filepath.Join(filepath.Dir(oldPath), newName)
			if err := os.Rename(oldPath, newPath); err != nil {
				fm.showError("Failed to rename: " + err.Error())
			} else {
				fm.refresh()
			}
		}
		dialog.Destroy()
	})

	dialog.Present()
	entry.GrabFocus()
}

func (fm *FileManager) showError(message string) {
	dialog := gtk.NewDialog()
	dialog.SetTitle("Error")
	dialog.SetTransientFor(fm.window)
	dialog.SetModal(true)

	content := dialog.ContentArea()
	content.SetMarginTop(16)
	content.SetMarginBottom(16)
	content.SetMarginStart(16)
	content.SetMarginEnd(16)
	content.SetSpacing(12)

	icon := gtk.NewImageFromIconName("dialog-error-symbolic")
	icon.SetPixelSize(48)
	content.Append(icon)

	label := gtk.NewLabel(message)
	label.SetWrap(true)
	content.Append(label)

	buttonBox := gtk.NewBox(gtk.OrientationHorizontal, 8)
	buttonBox.SetHAlign(gtk.AlignEnd)
	buttonBox.SetMarginTop(16)

	okBtn := gtk.NewButton()
	okBtn.SetLabel("OK")
	okBtn.ConnectClicked(func() { dialog.Destroy() })
	buttonBox.Append(okBtn)

	content.Append(buttonBox)
	dialog.Present()
}
