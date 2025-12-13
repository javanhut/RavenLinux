package main

import (
	"fmt"
	"image/color"
	"os"
	"sort"
	"sync"
	"time"

	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/app"
	"fyne.io/fyne/v2/canvas"
	"fyne.io/fyne/v2/container"
	"fyne.io/fyne/v2/dialog"
	"fyne.io/fyne/v2/layout"
	"fyne.io/fyne/v2/theme"
	"fyne.io/fyne/v2/widget"
)

// WiFiApp holds the application state
type WiFiApp struct {
	window        fyne.Window
	wifi          *WiFiManager
	networkList   *widget.List
	networks      []Network
	statusLabel   *widget.Label
	connectBtn    *widget.Button
	selectedIndex int
	mu            sync.Mutex
	scanning      bool
}

func main() {
	if os.Geteuid() != 0 {
		fmt.Fprintln(os.Stderr, "This tool requires root privileges. Please run with sudo:")
		fmt.Fprintln(os.Stderr, "  sudo raven-wifi")
		os.Exit(1)
	}

	if os.Getenv("DISPLAY") == "" && os.Getenv("WAYLAND_DISPLAY") == "" {
		fmt.Fprintln(os.Stderr, "No graphical session detected (DISPLAY/WAYLAND_DISPLAY not set).")
		fmt.Fprintln(os.Stderr, "Use `wifi` for the TUI version, or start the desktop environment first.")
		os.Exit(1)
	}

	a := app.New()
	a.Settings().SetTheme(&ravenTheme{})

	w := a.NewWindow("Raven WiFi")
	w.Resize(fyne.NewSize(400, 500))
	w.CenterOnScreen()

	wifiApp := &WiFiApp{
		window:        w,
		wifi:          NewWiFiManager(),
		selectedIndex: -1,
	}

	w.SetContent(wifiApp.createUI())

	// Initial scan
	go wifiApp.refreshNetworks()

	w.ShowAndRun()
}

func (wa *WiFiApp) createUI() fyne.CanvasObject {
	// Header with title and refresh button
	title := widget.NewLabelWithStyle("WiFi Networks", fyne.TextAlignCenter, fyne.TextStyle{Bold: true})

	refreshBtn := widget.NewButtonWithIcon("", theme.ViewRefreshIcon(), func() {
		go wa.refreshNetworks()
	})

	header := container.NewBorder(nil, nil, nil, refreshBtn, title)

	// Status bar showing current connection
	wa.statusLabel = widget.NewLabel("Checking connection...")
	wa.statusLabel.Wrapping = fyne.TextWrapWord
	statusCard := widget.NewCard("", "", wa.statusLabel)

	// Network list
	wa.networkList = widget.NewList(
		func() int {
			wa.mu.Lock()
			defer wa.mu.Unlock()
			return len(wa.networks)
		},
		func() fyne.CanvasObject {
			return wa.createNetworkItem()
		},
		func(id widget.ListItemID, obj fyne.CanvasObject) {
			wa.updateNetworkItem(id, obj)
		},
	)

	wa.networkList.OnSelected = func(id widget.ListItemID) {
		wa.mu.Lock()
		wa.selectedIndex = id
		wa.mu.Unlock()
		wa.updateConnectButton()
	}

	// Connect button
	wa.connectBtn = widget.NewButtonWithIcon("Connect", theme.ConfirmIcon(), wa.onConnect)
	wa.connectBtn.Importance = widget.HighImportance
	wa.connectBtn.Disable()

	// Disconnect button
	disconnectBtn := widget.NewButtonWithIcon("Disconnect", theme.CancelIcon(), wa.onDisconnect)

	// Saved networks button
	savedBtn := widget.NewButtonWithIcon("Saved Networks", theme.StorageIcon(), wa.showSavedNetworks)

	buttonBar := container.NewHBox(
		layout.NewSpacer(),
		savedBtn,
		disconnectBtn,
		wa.connectBtn,
	)

	// Main layout
	content := container.NewBorder(
		container.NewVBox(header, statusCard),
		buttonBar,
		nil, nil,
		wa.networkList,
	)

	return container.NewPadded(content)
}

func (wa *WiFiApp) createNetworkItem() fyne.CanvasObject {
	ssidLabel := widget.NewLabel("Network Name")
	ssidLabel.TextStyle = fyne.TextStyle{Bold: true}

	detailLabel := widget.NewLabel("Signal: 100%")
	detailLabel.TextStyle = fyne.TextStyle{}

	signalIcon := canvas.NewRectangle(color.NRGBA{R: 76, G: 175, B: 80, A: 255})
	signalIcon.SetMinSize(fyne.NewSize(8, 20))

	lockIcon := widget.NewIcon(theme.VisibilityIcon())

	leftContent := container.NewVBox(ssidLabel, detailLabel)
	rightContent := container.NewHBox(lockIcon, signalIcon)

	return container.NewBorder(nil, nil, nil, rightContent, leftContent)
}

func (wa *WiFiApp) updateNetworkItem(id widget.ListItemID, obj fyne.CanvasObject) {
	wa.mu.Lock()
	if id >= len(wa.networks) {
		wa.mu.Unlock()
		return
	}
	network := wa.networks[id]
	wa.mu.Unlock()

	borderContainer := obj.(*fyne.Container)
	leftContent := borderContainer.Objects[0].(*fyne.Container)
	rightContent := borderContainer.Objects[1].(*fyne.Container)

	ssidLabel := leftContent.Objects[0].(*widget.Label)
	detailLabel := leftContent.Objects[1].(*widget.Label)
	lockIcon := rightContent.Objects[0].(*widget.Icon)
	signalRect := rightContent.Objects[1].(*canvas.Rectangle)

	// Set SSID with connected indicator
	if network.Connected {
		ssidLabel.SetText(network.SSID + " (Connected)")
	} else {
		ssidLabel.SetText(network.SSID)
	}

	// Set details
	securityText := "Open"
	if network.Security != "" && network.Security != "Open" {
		securityText = network.Security
	}
	detailLabel.SetText(fmt.Sprintf("Signal: %d%% • %s", network.Signal, securityText))

	// Lock icon for secured networks
	if network.Security != "" && network.Security != "Open" {
		lockIcon.SetResource(theme.VisibilityOffIcon()) // Lock icon
	} else {
		lockIcon.SetResource(theme.VisibilityIcon()) // Open icon
	}

	// Signal strength color
	signalColor := wa.getSignalColor(network.Signal)
	signalRect.FillColor = signalColor
	signalRect.Refresh()
}

func (wa *WiFiApp) getSignalColor(signal int) color.Color {
	if signal >= 70 {
		return color.NRGBA{R: 76, G: 175, B: 80, A: 255} // Green
	} else if signal >= 40 {
		return color.NRGBA{R: 255, G: 193, B: 7, A: 255} // Yellow
	}
	return color.NRGBA{R: 244, G: 67, B: 54, A: 255} // Red
}

func (wa *WiFiApp) refreshNetworks() {
	wa.mu.Lock()
	if wa.scanning {
		wa.mu.Unlock()
		return
	}
	wa.scanning = true
	wa.mu.Unlock()

	wa.statusLabel.SetText("Scanning for networks...")

	// Get current connection status
	status, _ := wa.wifi.GetStatus()

	// Scan for networks
	networks, err := wa.wifi.Scan()
	if err != nil {
		wa.statusLabel.SetText("Error: " + err.Error())
		wa.mu.Lock()
		wa.scanning = false
		wa.mu.Unlock()
		return
	}

	// Sort by signal strength (strongest first), connected first
	sort.Slice(networks, func(i, j int) bool {
		if networks[i].Connected != networks[j].Connected {
			return networks[i].Connected
		}
		return networks[i].Signal > networks[j].Signal
	})

	wa.mu.Lock()
	wa.networks = networks
	wa.scanning = false
	wa.mu.Unlock()

	wa.networkList.Refresh()

	// Update status
	if status.Connected {
		wa.statusLabel.SetText(fmt.Sprintf("Connected to: %s\nIP: %s", status.SSID, status.IP))
	} else {
		wa.statusLabel.SetText("Not connected")
	}
}

func (wa *WiFiApp) updateConnectButton() {
	wa.mu.Lock()
	idx := wa.selectedIndex
	var network *Network
	if idx >= 0 && idx < len(wa.networks) {
		network = &wa.networks[idx]
	}
	wa.mu.Unlock()

	if network == nil {
		wa.connectBtn.Disable()
		return
	}

	if network.Connected {
		wa.connectBtn.SetText("Connected")
		wa.connectBtn.Disable()
	} else {
		wa.connectBtn.SetText("Connect")
		wa.connectBtn.Enable()
	}
}

func (wa *WiFiApp) onConnect() {
	wa.mu.Lock()
	idx := wa.selectedIndex
	if idx < 0 || idx >= len(wa.networks) {
		wa.mu.Unlock()
		return
	}
	network := wa.networks[idx]
	wa.mu.Unlock()

	if network.Connected {
		return
	}

	// Check if we have a saved password
	if network.Security != "" && network.Security != "Open" {
		// Check if already known
		if wa.wifi.IsKnownNetwork(network.SSID) {
			wa.connectToNetwork(network.SSID, "")
		} else {
			// Show password dialog
			wa.showPasswordDialog(network.SSID)
		}
	} else {
		// Open network, connect directly
		wa.connectToNetwork(network.SSID, "")
	}
}

func (wa *WiFiApp) showPasswordDialog(ssid string) {
	passwordEntry := widget.NewPasswordEntry()
	passwordEntry.SetPlaceHolder("Enter WiFi password")

	items := []*widget.FormItem{
		widget.NewFormItem("Password", passwordEntry),
	}

	dialog.ShowForm("Connect to "+ssid, "Connect", "Cancel", items, func(confirmed bool) {
		if confirmed && passwordEntry.Text != "" {
			wa.connectToNetwork(ssid, passwordEntry.Text)
		}
	}, wa.window)
}

func (wa *WiFiApp) connectToNetwork(ssid, password string) {
	wa.statusLabel.SetText("Connecting to " + ssid + "...")
	wa.connectBtn.Disable()

	go func() {
		err := wa.wifi.Connect(ssid, password)
		if err != nil {
			dialog.ShowError(fmt.Errorf("Failed to connect: %v", err), wa.window)
			wa.statusLabel.SetText("Connection failed")
			wa.connectBtn.Enable()
			return
		}

		// Wait a moment for connection to establish
		time.Sleep(2 * time.Second)
		wa.refreshNetworks()
	}()
}

func (wa *WiFiApp) onDisconnect() {
	wa.statusLabel.SetText("Disconnecting...")

	go func() {
		err := wa.wifi.Disconnect()
		if err != nil {
			dialog.ShowError(fmt.Errorf("Failed to disconnect: %v", err), wa.window)
		}
		time.Sleep(1 * time.Second)
		wa.refreshNetworks()
	}()
}

func (wa *WiFiApp) showSavedNetworks() {
	saved, err := wa.wifi.GetSavedNetworks()
	if err != nil {
		dialog.ShowError(err, wa.window)
		return
	}

	if len(saved) == 0 {
		dialog.ShowInformation("Saved Networks", "No saved networks found.", wa.window)
		return
	}

	// Create list of saved networks with delete buttons
	var items []fyne.CanvasObject
	for _, ssid := range saved {
		ssidCopy := ssid
		label := widget.NewLabel(ssid)
		deleteBtn := widget.NewButtonWithIcon("", theme.DeleteIcon(), func() {
			dialog.ShowConfirm("Forget Network",
				"Forget '"+ssidCopy+"'?",
				func(confirmed bool) {
					if confirmed {
						wa.wifi.ForgetNetwork(ssidCopy)
						wa.showSavedNetworks() // Refresh
					}
				}, wa.window)
		})
		items = append(items, container.NewBorder(nil, nil, nil, deleteBtn, label))
	}

	content := container.NewVBox(items...)
	scrollContent := container.NewVScroll(content)
	scrollContent.SetMinSize(fyne.NewSize(300, 200))

	dialog.ShowCustom("Saved Networks", "Close", scrollContent, wa.window)
}

// Raven theme
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
