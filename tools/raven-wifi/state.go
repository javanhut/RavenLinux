package main

import (
	"fmt"
	"image"
	"sync"
	"time"

	"gioui.org/layout"
	"gioui.org/widget"
)

// AppState holds all application state for the Gio UI
type AppState struct {
	// WiFi backend
	wifi *WiFiManager

	// Network data (protected by mutex)
	mu          sync.Mutex
	networks    []Network
	currentSSID string // Currently connected SSID

	// UI state
	statusText     string
	scanning       bool
	connecting     bool
	connectingSSID string // Which network is being connected to

	// Widget state (must persist across frames)
	list          widget.List
	disconnectBtn widget.Clickable
	savedBtn      widget.Clickable
	refreshBtn    widget.Clickable
	networkClicks map[int]*widget.Clickable

	// Dialog state
	showPasswordDialog bool
	showSavedDialog    bool
	showErrorDialog    bool
	pendingSSID        string // SSID waiting for password
	passwordEditor     widget.Editor
	errorTitle         string
	errorMessage       string

	// Dialog buttons
	passwordOK     widget.Clickable
	passwordCancel widget.Clickable
	errorOK        widget.Clickable

	// Saved networks dialog
	savedNetworks     []string
	savedDeletes      map[string]*widget.Clickable
	savedClose        widget.Clickable
	forgetSSID        string
	showForgetConfirm bool
	forgetConfirmYes  widget.Clickable
	forgetConfirmNo   widget.Clickable

	// Config persistence
	config       *Config
	saveDebounce *SaveDebouncer
	lastSize     image.Point

	// Window control
	invalidate func() // Triggers window redraw
}

// NewAppState creates and initializes a new application state
func NewAppState() *AppState {
	cfg := LoadConfig()

	s := &AppState{
		wifi:          NewWiFiManager(),
		config:        cfg,
		networkClicks: make(map[int]*widget.Clickable),
		savedDeletes:  make(map[string]*widget.Clickable),
		list: widget.List{
			List: layout.List{Axis: layout.Vertical},
		},
	}

	s.saveDebounce = NewSaveDebouncer(cfg, 500*time.Millisecond)
	s.statusText = "Initializing..."

	return s
}

// getNetworkClickable returns a persistent clickable for a network index
func (s *AppState) getNetworkClickable(index int) *widget.Clickable {
	if s.networkClicks[index] == nil {
		s.networkClicks[index] = new(widget.Clickable)
	}
	return s.networkClicks[index]
}

// getSavedDeleteClickable returns a persistent clickable for saved network delete
func (s *AppState) getSavedDeleteClickable(ssid string) *widget.Clickable {
	if s.savedDeletes[ssid] == nil {
		s.savedDeletes[ssid] = new(widget.Clickable)
	}
	return s.savedDeletes[ssid]
}

// saveSizeDebounced requests a debounced save of window size
func (s *AppState) saveSizeDebounced() {
	if s.saveDebounce != nil {
		s.saveDebounce.RequestSave()
	}
}

// refreshNetworks performs a background network scan
func (s *AppState) refreshNetworks() {
	s.mu.Lock()
	if s.scanning {
		s.mu.Unlock()
		return
	}
	s.scanning = true
	s.statusText = "Scanning for networks..."
	s.mu.Unlock()

	if s.invalidate != nil {
		s.invalidate()
	}

	// Get current connection status
	status, _ := s.wifi.GetStatus()

	// Scan for networks
	networks, err := s.wifi.Scan()

	s.mu.Lock()
	s.scanning = false
	if err != nil {
		s.statusText = "Error: " + err.Error()
	} else {
		s.networks = networks
		s.currentSSID = status.SSID
		if status.Connected {
			s.statusText = fmt.Sprintf("Connected to: %s\nIP: %s", status.SSID, status.IP)
		} else {
			s.statusText = "Not connected"
			s.currentSSID = ""
		}
	}
	s.mu.Unlock()

	if s.invalidate != nil {
		s.invalidate()
	}
}

// handleNetworkClick handles a click on a network item
func (s *AppState) handleNetworkClick(net Network) {
	// Do nothing if already connected to this network
	if net.Connected {
		return
	}

	s.mu.Lock()
	// Don't allow multiple simultaneous connections
	if s.connecting {
		s.mu.Unlock()
		return
	}
	s.mu.Unlock()

	// Check if open network
	if net.Security == "" || net.Security == "Open" {
		s.connectToNetwork(net.SSID, "")
		return
	}

	// Secured network - check if password known
	if s.wifi.IsKnownNetwork(net.SSID) {
		// Connect with saved password
		s.connectToNetwork(net.SSID, "")
	} else {
		// Show password dialog
		s.mu.Lock()
		s.pendingSSID = net.SSID
		s.showPasswordDialog = true
		s.mu.Unlock()
		if s.invalidate != nil {
			s.invalidate()
		}
	}
}

// connectToNetwork attempts to connect to a network
func (s *AppState) connectToNetwork(ssid, password string) {
	s.mu.Lock()
	s.connecting = true
	s.connectingSSID = ssid
	s.statusText = "Connecting to " + ssid + "..."
	s.mu.Unlock()

	if s.invalidate != nil {
		s.invalidate()
	}

	// Actually connect
	err := s.wifi.Connect(ssid, password)

	s.mu.Lock()
	s.connecting = false
	s.connectingSSID = ""
	if err != nil {
		s.errorTitle = "Connection Failed"
		s.errorMessage = err.Error()
		s.showErrorDialog = true
		s.statusText = "Connection failed"
	} else {
		s.statusText = "Connected to " + ssid
	}
	s.mu.Unlock()

	if s.invalidate != nil {
		s.invalidate()
	}

	// Refresh to get updated status
	time.Sleep(1 * time.Second)
	s.refreshNetworks()
}

// onDisconnect handles the disconnect button
func (s *AppState) onDisconnect() {
	s.mu.Lock()
	s.statusText = "Disconnecting..."
	s.mu.Unlock()

	if s.invalidate != nil {
		s.invalidate()
	}

	err := s.wifi.Disconnect()

	s.mu.Lock()
	if err != nil {
		s.errorTitle = "Disconnect Failed"
		s.errorMessage = err.Error()
		s.showErrorDialog = true
	}
	s.mu.Unlock()

	if s.invalidate != nil {
		s.invalidate()
	}

	s.refreshNetworks()
}

// showSavedNetworks loads and displays saved networks
func (s *AppState) showSavedNetworks() {
	saved, err := s.wifi.GetSavedNetworks()

	s.mu.Lock()
	if err != nil {
		s.errorTitle = "Error"
		s.errorMessage = err.Error()
		s.showErrorDialog = true
		s.mu.Unlock()
		if s.invalidate != nil {
			s.invalidate()
		}
		return
	}

	s.savedNetworks = saved
	s.showSavedDialog = true
	s.mu.Unlock()

	if s.invalidate != nil {
		s.invalidate()
	}
}

// forgetNetwork removes a saved network
func (s *AppState) forgetNetwork(ssid string) {
	s.wifi.ForgetNetwork(ssid)

	// Refresh saved networks list
	s.showSavedNetworks()
}
