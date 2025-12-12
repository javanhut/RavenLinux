package main

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"sort"
	"strconv"
	"strings"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
)

// Styles
var (
	titleStyle = lipgloss.NewStyle().
			Bold(true).
			Foreground(lipgloss.Color("#00BCD4")).
			MarginBottom(1)

	selectedStyle = lipgloss.NewStyle().
			Bold(true).
			Foreground(lipgloss.Color("#000")).
			Background(lipgloss.Color("#00BCD4")).
			Padding(0, 1)

	normalStyle = lipgloss.NewStyle().
			Foreground(lipgloss.Color("#FFF")).
			Padding(0, 1)

	connectedStyle = lipgloss.NewStyle().
			Foreground(lipgloss.Color("#4CAF50")).
			Padding(0, 1)

	signalGood = lipgloss.NewStyle().Foreground(lipgloss.Color("#4CAF50"))
	signalMed  = lipgloss.NewStyle().Foreground(lipgloss.Color("#FFC107"))
	signalBad  = lipgloss.NewStyle().Foreground(lipgloss.Color("#F44336"))

	statusStyle = lipgloss.NewStyle().
			Foreground(lipgloss.Color("#888")).
			MarginTop(1)

	errorStyle = lipgloss.NewStyle().
			Foreground(lipgloss.Color("#F44336")).
			Bold(true)

	successStyle = lipgloss.NewStyle().
			Foreground(lipgloss.Color("#4CAF50")).
			Bold(true)

	helpStyle = lipgloss.NewStyle().
			Foreground(lipgloss.Color("#666")).
			MarginTop(1)
)

// Network represents a WiFi network
type Network struct {
	SSID      string
	Signal    int
	Security  string
	Connected bool
}

// App state
type state int

const (
	stateScanning state = iota
	stateList
	statePassword
	stateConnecting
	stateSuccess
	stateError
)

type model struct {
	state       state
	networks    []Network
	cursor      int
	iface       string
	password    string
	message     string
	currentSSID string
}

// Messages
type scanDoneMsg struct {
	networks []Network
	err      error
}

type connectDoneMsg struct {
	success bool
	err     error
}

func main() {
	// Check if running as root
	if os.Geteuid() != 0 {
		fmt.Println("This tool requires root privileges. Please run with sudo:")
		fmt.Println("  sudo wifi")
		os.Exit(1)
	}

	p := tea.NewProgram(initialModel(), tea.WithAltScreen())
	if _, err := p.Run(); err != nil {
		fmt.Printf("Error: %v\n", err)
		os.Exit(1)
	}
}

func initialModel() model {
	m := model{
		state: stateScanning,
		iface: detectInterface(),
	}
	return m
}

func (m model) Init() tea.Cmd {
	return scanNetworks(m.iface)
}

func (m model) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.KeyMsg:
		return m.handleKey(msg)

	case scanDoneMsg:
		if msg.err != nil {
			m.state = stateError
			m.message = msg.err.Error()
			return m, nil
		}
		m.networks = msg.networks
		m.state = stateList
		return m, nil

	case connectDoneMsg:
		if msg.err != nil || !msg.success {
			m.state = stateError
			if msg.err != nil {
				m.message = msg.err.Error()
			} else {
				m.message = "Connection failed. Check password and try again."
			}
			return m, nil
		}
		m.state = stateSuccess
		m.message = fmt.Sprintf("Connected to %s!", m.currentSSID)
		return m, nil
	}

	return m, nil
}

func (m model) handleKey(msg tea.KeyMsg) (tea.Model, tea.Cmd) {
	switch m.state {
	case stateList:
		switch msg.String() {
		case "q", "ctrl+c", "esc":
			return m, tea.Quit
		case "up", "k":
			if m.cursor > 0 {
				m.cursor--
			}
		case "down", "j":
			if m.cursor < len(m.networks)-1 {
				m.cursor++
			}
		case "enter":
			if len(m.networks) > 0 {
				net := m.networks[m.cursor]
				if net.Connected {
					return m, nil // Already connected
				}
				m.currentSSID = net.SSID
				if net.Security != "" && net.Security != "Open" {
					// Check if we have saved credentials
					if isKnownNetwork(net.SSID) {
						m.state = stateConnecting
						m.message = "Connecting..."
						return m, connectToNetwork(m.iface, net.SSID, "")
					}
					m.state = statePassword
					m.password = ""
				} else {
					m.state = stateConnecting
					m.message = "Connecting..."
					return m, connectToNetwork(m.iface, net.SSID, "")
				}
			}
		case "r":
			m.state = stateScanning
			return m, scanNetworks(m.iface)
		case "d":
			// Disconnect
			disconnect(m.iface)
			m.state = stateScanning
			return m, scanNetworks(m.iface)
		}

	case statePassword:
		switch msg.String() {
		case "esc":
			m.state = stateList
			m.password = ""
		case "enter":
			if m.password != "" {
				m.state = stateConnecting
				m.message = "Connecting..."
				return m, connectToNetwork(m.iface, m.currentSSID, m.password)
			}
		case "backspace":
			if len(m.password) > 0 {
				m.password = m.password[:len(m.password)-1]
			}
		case "ctrl+c":
			return m, tea.Quit
		default:
			// Add character to password
			if len(msg.String()) == 1 {
				m.password += msg.String()
			}
		}

	case stateSuccess:
		switch msg.String() {
		case "enter", "q", "esc":
			return m, tea.Quit
		case "s":
			// Save network (already saved by connect)
			return m, tea.Quit
		}

	case stateError:
		switch msg.String() {
		case "enter", "r":
			m.state = stateScanning
			return m, scanNetworks(m.iface)
		case "q", "esc", "ctrl+c":
			return m, tea.Quit
		}
	}

	return m, nil
}

func (m model) View() string {
	var s strings.Builder

	// Header
	s.WriteString(titleStyle.Render("🌐 Raven WiFi"))
	s.WriteString("\n\n")

	switch m.state {
	case stateScanning:
		s.WriteString("Scanning for networks...\n")
		s.WriteString("Please wait.\n")

	case stateList:
		if len(m.networks) == 0 {
			s.WriteString("No networks found.\n")
			s.WriteString(helpStyle.Render("Press 'r' to rescan, 'q' to quit"))
		} else {
			// Current connection status
			for _, net := range m.networks {
				if net.Connected {
					s.WriteString(successStyle.Render(fmt.Sprintf("✓ Connected to: %s", net.SSID)))
					s.WriteString("\n\n")
					break
				}
			}

			s.WriteString("Available Networks:\n\n")

			for i, net := range m.networks {
				// Signal indicator
				signal := getSignalBars(net.Signal)

				// Lock icon for secured networks
				lock := "🔓"
				if net.Security != "" && net.Security != "Open" {
					lock = "🔒"
				}

				// Format line
				line := fmt.Sprintf("%s %s %s", signal, lock, net.SSID)

				if net.Connected {
					line += " (connected)"
				}

				if i == m.cursor {
					s.WriteString(selectedStyle.Render("> " + line))
				} else if net.Connected {
					s.WriteString(connectedStyle.Render("  " + line))
				} else {
					s.WriteString(normalStyle.Render("  " + line))
				}
				s.WriteString("\n")
			}

			s.WriteString(helpStyle.Render("\n↑/↓: Navigate • Enter: Connect • r: Rescan • d: Disconnect • q: Quit"))
		}

	case statePassword:
		s.WriteString(fmt.Sprintf("Connect to: %s\n\n", m.currentSSID))
		s.WriteString("Password: ")
		// Show password as dots
		s.WriteString(strings.Repeat("•", len(m.password)))
		s.WriteString("█\n")
		s.WriteString(helpStyle.Render("\nEnter: Connect • Esc: Cancel"))

	case stateConnecting:
		s.WriteString(fmt.Sprintf("Connecting to %s...\n", m.currentSSID))
		s.WriteString("Please wait.\n")

	case stateSuccess:
		s.WriteString(successStyle.Render("✓ " + m.message))
		s.WriteString("\n\n")
		s.WriteString("Network saved for future connections.\n")
		s.WriteString(helpStyle.Render("\nPress Enter or 'q' to exit"))

	case stateError:
		s.WriteString(errorStyle.Render("✗ Error: " + m.message))
		s.WriteString("\n")
		s.WriteString(helpStyle.Render("\nPress 'r' to retry, 'q' to quit"))
	}

	return s.String()
}

func getSignalBars(signal int) string {
	bars := ""
	if signal >= 75 {
		bars = signalGood.Render("████")
	} else if signal >= 50 {
		bars = signalMed.Render("███") + "░"
	} else if signal >= 25 {
		bars = signalBad.Render("██") + "░░"
	} else {
		bars = signalBad.Render("█") + "░░░"
	}
	return bars
}

// WiFi operations

func detectInterface() string {
	interfaces := []string{"wlan0", "wlp2s0", "wlp3s0", "wifi0"}
	for _, iface := range interfaces {
		if _, err := os.Stat("/sys/class/net/" + iface + "/wireless"); err == nil {
			return iface
		}
	}

	// Find any wireless interface
	matches, _ := filepath.Glob("/sys/class/net/*/wireless")
	if len(matches) > 0 {
		parts := strings.Split(matches[0], "/")
		if len(parts) >= 5 {
			return parts[4]
		}
	}

	return "wlan0"
}

func scanNetworks(iface string) tea.Cmd {
	return func() tea.Msg {
		// Ensure interface is up
		exec.Command("ip", "link", "set", iface, "up").Run()
		exec.Command("rfkill", "unblock", "wifi").Run()

		var networks []Network

		// Try iwd first
		if _, err := exec.LookPath("iwctl"); err == nil {
			networks, _ = scanWithIWD(iface)
		}

		// Fallback to raw iw scan
		if len(networks) == 0 {
			networks, _ = scanWithIW(iface)
		}

		// Get current connection
		currentSSID := getCurrentSSID(iface)

		// Mark connected network
		for i := range networks {
			if networks[i].SSID == currentSSID {
				networks[i].Connected = true
			}
		}

		// Sort: connected first, then by signal
		sort.Slice(networks, func(i, j int) bool {
			if networks[i].Connected != networks[j].Connected {
				return networks[i].Connected
			}
			return networks[i].Signal > networks[j].Signal
		})

		// Remove duplicates
		seen := make(map[string]bool)
		unique := []Network{}
		for _, n := range networks {
			if n.SSID != "" && !seen[n.SSID] {
				seen[n.SSID] = true
				unique = append(unique, n)
			}
		}

		return scanDoneMsg{networks: unique}
	}
}

func scanWithIWD(iface string) ([]Network, error) {
	// Trigger scan
	exec.Command("iwctl", "station", iface, "scan").Run()
	time.Sleep(3 * time.Second)

	// Get networks
	cmd := exec.Command("iwctl", "station", iface, "get-networks")
	output, err := cmd.Output()
	if err != nil {
		return nil, err
	}

	var networks []Network
	lines := strings.Split(string(output), "\n")

	for _, line := range lines {
		// Remove ANSI codes
		clean := regexp.MustCompile(`\x1b\[[0-9;]*m`).ReplaceAllString(line, "")
		clean = strings.TrimSpace(clean)

		if clean == "" || strings.HasPrefix(clean, "Available") || strings.HasPrefix(clean, "---") {
			continue
		}

		// Parse: [>] SSID Security Signal
		fields := strings.Fields(clean)
		if len(fields) < 2 {
			continue
		}

		startIdx := 0
		if fields[0] == ">" || fields[0] == "*" {
			startIdx = 1
		}

		if startIdx >= len(fields) {
			continue
		}

		ssid := fields[startIdx]
		security := "Open"
		signal := 50

		// Check remaining fields for security and signal
		for j := startIdx + 1; j < len(fields); j++ {
			f := fields[j]
			if strings.Contains(strings.ToLower(f), "psk") || strings.Contains(strings.ToLower(f), "wpa") {
				security = "WPA2"
			}
			// Signal bars (****)
			if strings.Contains(f, "*") {
				signal = strings.Count(f, "*") * 25
			}
		}

		if ssid != "" && ssid != "[Hidden]" {
			networks = append(networks, Network{
				SSID:     ssid,
				Signal:   signal,
				Security: security,
			})
		}
	}

	return networks, nil
}

func scanWithIW(iface string) ([]Network, error) {
	cmd := exec.Command("iw", "dev", iface, "scan")
	output, err := cmd.Output()
	if err != nil {
		return nil, err
	}

	var networks []Network
	var current *Network

	lines := strings.Split(string(output), "\n")
	for _, line := range lines {
		line = strings.TrimSpace(line)

		if strings.HasPrefix(line, "BSS ") {
			if current != nil && current.SSID != "" {
				networks = append(networks, *current)
			}
			current = &Network{Security: "Open"}
		} else if current != nil {
			if strings.HasPrefix(line, "SSID:") {
				current.SSID = strings.TrimPrefix(line, "SSID: ")
			} else if strings.HasPrefix(line, "signal:") {
				re := regexp.MustCompile(`-?\d+`)
				match := re.FindString(line)
				if match != "" {
					dbm, _ := strconv.Atoi(match)
					current.Signal = dbmToPercent(dbm)
				}
			} else if strings.Contains(line, "WPA") || strings.Contains(line, "RSN") {
				current.Security = "WPA2"
			}
		}
	}

	if current != nil && current.SSID != "" {
		networks = append(networks, *current)
	}

	return networks, nil
}

func getCurrentSSID(iface string) string {
	cmd := exec.Command("iw", "dev", iface, "link")
	output, _ := cmd.Output()

	for _, line := range strings.Split(string(output), "\n") {
		if strings.Contains(line, "SSID:") {
			return strings.TrimSpace(strings.TrimPrefix(line, "\tSSID:"))
		}
	}
	return ""
}

func connectToNetwork(iface, ssid, password string) tea.Cmd {
	return func() tea.Msg {
		var err error

		// Try iwd first
		if _, e := exec.LookPath("iwctl"); e == nil {
			err = connectWithIWD(iface, ssid, password)
		} else {
			err = connectWithWPA(iface, ssid, password)
		}

		if err != nil {
			return connectDoneMsg{success: false, err: err}
		}

		// Wait for connection
		time.Sleep(3 * time.Second)

		// Request DHCP
		requestDHCP(iface)

		// Wait for IP
		time.Sleep(2 * time.Second)

		// Check if connected
		currentSSID := getCurrentSSID(iface)
		if currentSSID == ssid {
			return connectDoneMsg{success: true}
		}

		return connectDoneMsg{success: false, err: fmt.Errorf("connection verification failed")}
	}
}

func connectWithIWD(iface, ssid, password string) error {
	if password != "" {
		// Save credentials for iwd
		safeName := strings.ReplaceAll(ssid, " ", "_")
		pskPath := fmt.Sprintf("/var/lib/iwd/%s.psk", safeName)

		content := fmt.Sprintf("[Security]\nPassphrase=%s\n", password)
		os.MkdirAll("/var/lib/iwd", 0755)
		os.WriteFile(pskPath, []byte(content), 0600)
	}

	cmd := exec.Command("iwctl", "station", iface, "connect", ssid)
	output, err := cmd.CombinedOutput()
	if err != nil {
		return fmt.Errorf("%s: %s", err, string(output))
	}
	return nil
}

func connectWithWPA(iface, ssid, password string) error {
	var config string

	if password != "" {
		cmd := exec.Command("wpa_passphrase", ssid, password)
		output, err := cmd.Output()
		if err != nil {
			return err
		}
		config = string(output)
	} else {
		config = fmt.Sprintf("network={\n\tssid=\"%s\"\n\tkey_mgmt=NONE\n}\n", ssid)
	}

	configPath := "/etc/wpa_supplicant/wpa_supplicant.conf"
	baseConfig := "ctrl_interface=/run/wpa_supplicant\nupdate_config=1\n\n"

	os.MkdirAll("/etc/wpa_supplicant", 0755)
	if err := os.WriteFile(configPath, []byte(baseConfig+config), 0600); err != nil {
		return err
	}

	// Kill existing wpa_supplicant
	exec.Command("killall", "wpa_supplicant").Run()
	time.Sleep(500 * time.Millisecond)

	// Start wpa_supplicant
	cmd := exec.Command("wpa_supplicant", "-B", "-i", iface, "-c", configPath)
	if err := cmd.Run(); err != nil {
		return err
	}

	return nil
}

func requestDHCP(iface string) {
	// Kill any existing DHCP clients
	exec.Command("killall", "dhcpcd").Run()
	exec.Command("killall", "dhclient").Run()

	// Try dhcpcd first
	if _, err := exec.LookPath("dhcpcd"); err == nil {
		exec.Command("dhcpcd", "-n", iface).Run()
		return
	}

	// Try dhclient
	if _, err := exec.LookPath("dhclient"); err == nil {
		exec.Command("dhclient", iface).Run()
		return
	}

	// Try udhcpc
	if _, err := exec.LookPath("udhcpc"); err == nil {
		exec.Command("udhcpc", "-i", iface, "-n", "-q").Run()
	}
}

func disconnect(iface string) {
	if _, err := exec.LookPath("iwctl"); err == nil {
		exec.Command("iwctl", "station", iface, "disconnect").Run()
	} else {
		exec.Command("wpa_cli", "-i", iface, "disconnect").Run()
	}
}

func isKnownNetwork(ssid string) bool {
	safeName := strings.ReplaceAll(ssid, " ", "_")

	// Check iwd
	if _, err := os.Stat(fmt.Sprintf("/var/lib/iwd/%s.psk", safeName)); err == nil {
		return true
	}
	if _, err := os.Stat(fmt.Sprintf("/var/lib/iwd/%s.open", safeName)); err == nil {
		return true
	}

	// Check wpa_supplicant
	data, err := os.ReadFile("/etc/wpa_supplicant/wpa_supplicant.conf")
	if err == nil && strings.Contains(string(data), fmt.Sprintf(`ssid="%s"`, ssid)) {
		return true
	}

	return false
}

func dbmToPercent(dbm int) int {
	if dbm >= -30 {
		return 100
	}
	if dbm <= -90 {
		return 0
	}
	return (dbm + 90) * 100 / 60
}
