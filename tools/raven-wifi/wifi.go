package main

import (
	"bufio"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"strconv"
	"strings"
	"time"
)

// Network represents a WiFi network
type Network struct {
	SSID      string
	BSSID     string
	Signal    int    // Signal strength percentage (0-100)
	Security  string // WPA2, WPA3, WEP, Open
	Frequency int    // MHz
	Connected bool
}

// ConnectionStatus represents current connection state
type ConnectionStatus struct {
	Connected bool
	SSID      string
	IP        string
	Interface string
}

// WiFiManager handles WiFi operations
type WiFiManager struct {
	iface   string
	backend string // "iwd" or "wpa"
}

// NewWiFiManager creates a new WiFi manager
func NewWiFiManager() *WiFiManager {
	wm := &WiFiManager{}
	wm.detectBackend()
	wm.detectInterface()
	return wm
}

func (wm *WiFiManager) detectBackend() {
	// Prefer iwd if available AND daemon is running
	if _, err := exec.LookPath("iwctl"); err == nil && isIWDRunning() && isDBusRunning() {
		wm.backend = "iwd"
		return
	}
	// Fall back to wpa_supplicant
	if _, err := exec.LookPath("wpa_cli"); err == nil {
		wm.backend = "wpa"
		return
	}
	wm.backend = "none"
}

// isIWDRunning checks if the iwd daemon is running
func isIWDRunning() bool {
	// Check if iwd process is running
	cmd := exec.Command("pgrep", "-x", "iwd")
	if err := cmd.Run(); err == nil {
		return true
	}
	// Alternative: check if iwd socket exists
	if _, err := os.Stat("/run/iwd"); err == nil {
		return true
	}
	return false
}

// isDBusRunning checks if D-Bus system bus is running
func isDBusRunning() bool {
	// Check if dbus-daemon process is running
	cmd := exec.Command("pgrep", "-x", "dbus-daemon")
	if err := cmd.Run(); err == nil {
		return true
	}
	// Alternative: check if D-Bus system socket exists
	if _, err := os.Stat("/run/dbus/system_bus_socket"); err == nil {
		return true
	}
	return false
}

func (wm *WiFiManager) detectInterface() {
	// Try to find wireless interface
	interfaces := []string{"wlan0", "wlp2s0", "wlp3s0", "wifi0"}

	for _, iface := range interfaces {
		if _, err := os.Stat("/sys/class/net/" + iface + "/wireless"); err == nil {
			wm.iface = iface
			return
		}
	}

	// Try to find any wireless interface
	matches, _ := filepath.Glob("/sys/class/net/*/wireless")
	if len(matches) > 0 {
		parts := strings.Split(matches[0], "/")
		if len(parts) >= 5 {
			wm.iface = parts[4]
			return
		}
	}

	wm.iface = "wlan0" // Default
}

// Scan scans for available WiFi networks
func (wm *WiFiManager) Scan() ([]Network, error) {
	switch wm.backend {
	case "iwd":
		return wm.scanIWD()
	case "wpa":
		return wm.scanWPA()
	default:
		return wm.scanIW() // Fallback to raw iw command
	}
}

func (wm *WiFiManager) scanIWD() ([]Network, error) {
	// Trigger scan
	exec.Command("iwctl", "station", wm.iface, "scan").Run()
	time.Sleep(2 * time.Second)

	// Get scan results
	cmd := exec.Command("iwctl", "station", wm.iface, "get-networks")
	output, err := cmd.Output()
	if err != nil {
		return nil, fmt.Errorf("scan failed: %v", err)
	}

	// Get current connection
	status, _ := wm.GetStatus()

	var networks []Network
	lines := strings.Split(string(output), "\n")

	// Skip header lines (first few lines are headers)
	for i, line := range lines {
		if i < 4 || len(strings.TrimSpace(line)) == 0 {
			continue
		}

		// Parse line - format varies, but generally: [connected] SSID Security Signal
		// Remove ANSI codes
		cleanLine := removeANSI(line)

		// Check if connected (starts with > or *)
		connected := strings.Contains(line, ">") || strings.HasPrefix(strings.TrimSpace(cleanLine), "*")

		// Extract fields
		fields := strings.Fields(cleanLine)
		if len(fields) < 2 {
			continue
		}

		// Handle connected marker
		startIdx := 0
		if fields[0] == ">" || fields[0] == "*" {
			startIdx = 1
		}

		if startIdx >= len(fields) {
			continue
		}

		ssid := fields[startIdx]

		// Skip hidden networks
		if ssid == "" || ssid == "[Hidden]" {
			continue
		}

		security := "Open"
		signal := 50 // Default

		// Try to parse security and signal from remaining fields
		for j := startIdx + 1; j < len(fields); j++ {
			field := fields[j]
			if strings.Contains(field, "psk") || strings.Contains(field, "wpa") {
				security = "WPA2"
			} else if strings.Contains(field, "open") {
				security = "Open"
			} else if strings.HasSuffix(field, "****") || strings.Contains(field, "*") {
				// Signal bars representation
				signal = len(strings.ReplaceAll(field, " ", "")) * 25
			}
		}

		// Check if this is the connected network
		if status.Connected && status.SSID == ssid {
			connected = true
		}

		networks = append(networks, Network{
			SSID:      ssid,
			Security:  security,
			Signal:    signal,
			Connected: connected,
		})
	}

	return networks, nil
}

func (wm *WiFiManager) scanWPA() ([]Network, error) {
	// Trigger scan
	exec.Command("wpa_cli", "-i", wm.iface, "scan").Run()
	time.Sleep(2 * time.Second)

	// Get results
	cmd := exec.Command("wpa_cli", "-i", wm.iface, "scan_results")
	output, err := cmd.Output()
	if err != nil {
		return nil, fmt.Errorf("scan failed: %v", err)
	}

	status, _ := wm.GetStatus()

	var networks []Network
	lines := strings.Split(string(output), "\n")

	for i, line := range lines {
		if i == 0 || len(strings.TrimSpace(line)) == 0 {
			continue // Skip header
		}

		fields := strings.Fields(line)
		if len(fields) < 5 {
			continue
		}

		bssid := fields[0]
		freq, _ := strconv.Atoi(fields[1])
		signalDBM, _ := strconv.Atoi(fields[2])
		flags := fields[3]
		ssid := strings.Join(fields[4:], " ")

		// Convert dBm to percentage
		signal := dbmToPercent(signalDBM)

		// Parse security
		security := "Open"
		if strings.Contains(flags, "WPA3") {
			security = "WPA3"
		} else if strings.Contains(flags, "WPA2") {
			security = "WPA2"
		} else if strings.Contains(flags, "WPA") {
			security = "WPA"
		} else if strings.Contains(flags, "WEP") {
			security = "WEP"
		}

		connected := status.Connected && status.SSID == ssid

		networks = append(networks, Network{
			SSID:      ssid,
			BSSID:     bssid,
			Signal:    signal,
			Security:  security,
			Frequency: freq,
			Connected: connected,
		})
	}

	return networks, nil
}

func (wm *WiFiManager) scanIW() ([]Network, error) {
	// Use raw iw command as fallback
	exec.Command("ip", "link", "set", wm.iface, "up").Run()

	cmd := exec.Command("iw", "dev", wm.iface, "scan")
	output, err := cmd.Output()
	if err != nil {
		return nil, fmt.Errorf("scan failed: %v", err)
	}

	status, _ := wm.GetStatus()

	var networks []Network
	var current *Network

	scanner := bufio.NewScanner(strings.NewReader(string(output)))
	for scanner.Scan() {
		line := strings.TrimSpace(scanner.Text())

		if strings.HasPrefix(line, "BSS ") {
			if current != nil && current.SSID != "" {
				networks = append(networks, *current)
			}
			current = &Network{}
			parts := strings.Fields(line)
			if len(parts) >= 2 {
				current.BSSID = strings.TrimSuffix(parts[1], "(on")
			}
		} else if current != nil {
			if strings.HasPrefix(line, "SSID:") {
				current.SSID = strings.TrimPrefix(line, "SSID: ")
				current.Connected = status.Connected && status.SSID == current.SSID
			} else if strings.HasPrefix(line, "signal:") {
				// Parse signal like "signal: -65.00 dBm"
				re := regexp.MustCompile(`-?\d+`)
				match := re.FindString(line)
				if match != "" {
					dbm, _ := strconv.Atoi(match)
					current.Signal = dbmToPercent(dbm)
				}
			} else if strings.HasPrefix(line, "freq:") {
				parts := strings.Fields(line)
				if len(parts) >= 2 {
					current.Frequency, _ = strconv.Atoi(parts[1])
				}
			} else if strings.Contains(line, "WPA") || strings.Contains(line, "RSN") {
				current.Security = "WPA2"
			} else if strings.Contains(line, "WEP") {
				current.Security = "WEP"
			}
		}
	}

	if current != nil && current.SSID != "" {
		networks = append(networks, *current)
	}

	// Mark open networks
	for i := range networks {
		if networks[i].Security == "" {
			networks[i].Security = "Open"
		}
	}

	return networks, nil
}

// Connect connects to a WiFi network
func (wm *WiFiManager) Connect(ssid, password string) error {
	switch wm.backend {
	case "iwd":
		return wm.connectIWD(ssid, password)
	case "wpa":
		return wm.connectWPA(ssid, password)
	default:
		return fmt.Errorf("no WiFi backend available")
	}
}

func (wm *WiFiManager) connectIWD(ssid, password string) error {
	if password != "" {
		// For networks requiring password, iwd will prompt or use stored credentials
		// We need to store the passphrase first
		pskPath := fmt.Sprintf("/var/lib/iwd/%s.psk", strings.ReplaceAll(ssid, " ", "_"))

		content := fmt.Sprintf("[Security]\nPassphrase=%s\n", password)
		if err := os.WriteFile(pskPath, []byte(content), 0600); err != nil {
			// Try without saving (will prompt)
		}
	}

	cmd := exec.Command("iwctl", "station", wm.iface, "connect", ssid)
	if password != "" {
		// iwctl might need password via stdin for unknown networks
		cmd.Stdin = strings.NewReader(password + "\n")
	}

	output, err := cmd.CombinedOutput()
	if err != nil {
		return fmt.Errorf("connect failed: %s", string(output))
	}

	// Wait for DHCP
	time.Sleep(2 * time.Second)
	wm.requestDHCP()

	return nil
}

func (wm *WiFiManager) connectWPA(ssid, password string) error {
	// Generate wpa_supplicant config
	var config string
	if password != "" {
		cmd := exec.Command("wpa_passphrase", ssid, password)
		output, err := cmd.Output()
		if err != nil {
			return fmt.Errorf("failed to generate passphrase: %v", err)
		}
		config = string(output)
	} else {
		config = fmt.Sprintf("network={\n\tssid=\"%s\"\n\tkey_mgmt=NONE\n}\n", ssid)
	}

	// Write config
	configPath := "/etc/wpa_supplicant/wpa_supplicant.conf"
	baseConfig := "ctrl_interface=/run/wpa_supplicant\nupdate_config=1\n\n"

	if err := os.WriteFile(configPath, []byte(baseConfig+config), 0600); err != nil {
		return fmt.Errorf("failed to write config: %v", err)
	}

	// Reconfigure wpa_supplicant
	exec.Command("wpa_cli", "-i", wm.iface, "reconfigure").Run()

	// Wait for connection
	time.Sleep(3 * time.Second)

	// Request DHCP
	wm.requestDHCP()

	return nil
}

func (wm *WiFiManager) requestDHCP() {
	// Try dhcpcd first
	if _, err := exec.LookPath("dhcpcd"); err == nil {
		exec.Command("dhcpcd", "-n", wm.iface).Run()
		return
	}

	// Try dhclient
	if _, err := exec.LookPath("dhclient"); err == nil {
		exec.Command("dhclient", wm.iface).Run()
		return
	}

	// Try udhcpc (busybox)
	if _, err := exec.LookPath("udhcpc"); err == nil {
		exec.Command("udhcpc", "-i", wm.iface, "-n", "-q").Run()
		return
	}

	// Try raven-dhcp (built-in)
	if _, err := exec.LookPath("raven-dhcp"); err == nil {
		exec.Command("raven-dhcp", "-i", wm.iface).Run()
	}
}

// Disconnect disconnects from current network
func (wm *WiFiManager) Disconnect() error {
	switch wm.backend {
	case "iwd":
		cmd := exec.Command("iwctl", "station", wm.iface, "disconnect")
		return cmd.Run()
	case "wpa":
		cmd := exec.Command("wpa_cli", "-i", wm.iface, "disconnect")
		return cmd.Run()
	default:
		exec.Command("ip", "link", "set", wm.iface, "down").Run()
		return nil
	}
}

// GetStatus returns current connection status
func (wm *WiFiManager) GetStatus() (ConnectionStatus, error) {
	status := ConnectionStatus{Interface: wm.iface}

	// Check if interface is connected using iw
	cmd := exec.Command("iw", "dev", wm.iface, "link")
	output, err := cmd.Output()
	if err == nil {
		outputStr := string(output)
		if !strings.Contains(outputStr, "Not connected") {
			// Parse SSID
			lines := strings.Split(outputStr, "\n")
			for _, line := range lines {
				if strings.Contains(line, "SSID:") {
					status.SSID = strings.TrimSpace(strings.TrimPrefix(line, "\tSSID:"))
					status.Connected = true
					break
				}
			}
		}
	}

	// Get IP address
	cmd = exec.Command("ip", "-4", "addr", "show", wm.iface)
	output, err = cmd.Output()
	if err == nil {
		re := regexp.MustCompile(`inet (\d+\.\d+\.\d+\.\d+)`)
		matches := re.FindStringSubmatch(string(output))
		if len(matches) >= 2 {
			status.IP = matches[1]
		}
	}

	return status, nil
}

// GetSavedNetworks returns list of saved network SSIDs
func (wm *WiFiManager) GetSavedNetworks() ([]string, error) {
	var networks []string

	switch wm.backend {
	case "iwd":
		// iwd stores networks in /var/lib/iwd/
		files, err := filepath.Glob("/var/lib/iwd/*.psk")
		if err == nil {
			for _, f := range files {
				name := filepath.Base(f)
				name = strings.TrimSuffix(name, ".psk")
				name = strings.ReplaceAll(name, "_", " ")
				networks = append(networks, name)
			}
		}
		// Also check .open files
		files, _ = filepath.Glob("/var/lib/iwd/*.open")
		for _, f := range files {
			name := filepath.Base(f)
			name = strings.TrimSuffix(name, ".open")
			name = strings.ReplaceAll(name, "_", " ")
			networks = append(networks, name)
		}

	case "wpa":
		// Parse wpa_supplicant.conf
		data, err := os.ReadFile("/etc/wpa_supplicant/wpa_supplicant.conf")
		if err == nil {
			re := regexp.MustCompile(`ssid="([^"]+)"`)
			matches := re.FindAllStringSubmatch(string(data), -1)
			for _, m := range matches {
				if len(m) >= 2 {
					networks = append(networks, m[1])
				}
			}
		}
	}

	return networks, nil
}

// IsKnownNetwork checks if a network is already saved
func (wm *WiFiManager) IsKnownNetwork(ssid string) bool {
	saved, _ := wm.GetSavedNetworks()
	for _, s := range saved {
		if s == ssid {
			return true
		}
	}
	return false
}

// ForgetNetwork removes a saved network
func (wm *WiFiManager) ForgetNetwork(ssid string) error {
	switch wm.backend {
	case "iwd":
		// Remove iwd config files
		safeName := strings.ReplaceAll(ssid, " ", "_")
		os.Remove(fmt.Sprintf("/var/lib/iwd/%s.psk", safeName))
		os.Remove(fmt.Sprintf("/var/lib/iwd/%s.open", safeName))
		return nil

	case "wpa":
		// Remove from wpa_supplicant.conf
		data, err := os.ReadFile("/etc/wpa_supplicant/wpa_supplicant.conf")
		if err != nil {
			return err
		}

		// Remove network block for this SSID
		content := string(data)
		re := regexp.MustCompile(`(?s)network=\{[^}]*ssid="` + regexp.QuoteMeta(ssid) + `"[^}]*\}`)
		content = re.ReplaceAllString(content, "")

		return os.WriteFile("/etc/wpa_supplicant/wpa_supplicant.conf", []byte(content), 0600)
	}

	return nil
}

// Helper functions

func dbmToPercent(dbm int) int {
	// Convert dBm to percentage (rough approximation)
	// -30 dBm = 100%, -90 dBm = 0%
	if dbm >= -30 {
		return 100
	}
	if dbm <= -90 {
		return 0
	}
	return (dbm + 90) * 100 / 60
}

func removeANSI(s string) string {
	re := regexp.MustCompile(`\x1b\[[0-9;]*m`)
	return re.ReplaceAllString(s, "")
}
