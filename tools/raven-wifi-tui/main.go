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
	// Main container box
	mainBoxStyle = lipgloss.NewStyle().
			Border(lipgloss.RoundedBorder()).
			BorderForeground(lipgloss.Color("#00BCD4")).
			Padding(1, 2).
			Width(70)

	titleStyle = lipgloss.NewStyle().
			Bold(true).
			Foreground(lipgloss.Color("#00BCD4")).
			Background(lipgloss.Color("#1a1a2e")).
			Padding(0, 2)

	subtitleStyle = lipgloss.NewStyle().
			Foreground(lipgloss.Color("#666"))

	selectedStyle = lipgloss.NewStyle().
			Bold(true).
			Foreground(lipgloss.Color("#1a1a2e")).
			Background(lipgloss.Color("#00BCD4")).
			Padding(0, 1)

	normalStyle = lipgloss.NewStyle().
			Foreground(lipgloss.Color("#e0e0e0")).
			Padding(0, 1)

	dimStyle = lipgloss.NewStyle().
			Foreground(lipgloss.Color("#555")).
			Padding(0, 1)

	connectedStyle = lipgloss.NewStyle().
			Foreground(lipgloss.Color("#4CAF50")).
			Bold(true).
			Padding(0, 1)

	wirelessStyle = lipgloss.NewStyle().
			Foreground(lipgloss.Color("#00BCD4")).
			Padding(0, 1)

	ethernetStyle = lipgloss.NewStyle().
			Foreground(lipgloss.Color("#9C27B0")).
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

	warnStyle = lipgloss.NewStyle().
			Foreground(lipgloss.Color("#FFC107"))

	successStyle = lipgloss.NewStyle().
			Foreground(lipgloss.Color("#4CAF50")).
			Bold(true)

	helpStyle = lipgloss.NewStyle().
			Foreground(lipgloss.Color("#555")).
			Italic(true)

	infoBoxStyle = lipgloss.NewStyle().
			Border(lipgloss.RoundedBorder()).
			BorderForeground(lipgloss.Color("#444")).
			Padding(0, 1)

	// Section header style
	sectionStyle = lipgloss.NewStyle().
			Bold(true).
			Foreground(lipgloss.Color("#00BCD4")).
			BorderBottom(true).
			BorderStyle(lipgloss.NormalBorder()).
			BorderForeground(lipgloss.Color("#333"))

	// Status badge styles
	badgeOK = lipgloss.NewStyle().
		Foreground(lipgloss.Color("#1a1a2e")).
		Background(lipgloss.Color("#4CAF50")).
		Padding(0, 1)

	badgeWarn = lipgloss.NewStyle().
			Foreground(lipgloss.Color("#1a1a2e")).
			Background(lipgloss.Color("#FFC107")).
			Padding(0, 1)

	badgeOff = lipgloss.NewStyle().
			Foreground(lipgloss.Color("#888")).
			Background(lipgloss.Color("#333")).
			Padding(0, 1)

	// Divider
	dividerStyle = lipgloss.NewStyle().
			Foreground(lipgloss.Color("#333"))
)

// NetInterface represents a network interface
type NetInterface struct {
	Name       string
	Type       string // "wireless", "ethernet", "loopback", "virtual", "unknown"
	Driver     string
	MAC        string
	State      string // "up", "down", "unknown"
	IP         string
	HasCarrier bool
	IsWireless bool
	DevicePath string // PCI/USB path
}

// Network represents a WiFi network
type Network struct {
	SSID      string
	Signal    int
	Security  string
	Connected bool
}

// SystemStatus holds system service status
type SystemStatus struct {
	DBusRunning bool
	IWDRunning  bool
	WPARunning  bool
}

// App state
type state int

const (
	stateInterfaces state = iota
	stateInterfaceInfo
	stateScanning
	stateNetworkList
	statePassword
	stateConnecting
	stateSuccess
	stateError
)

type model struct {
	state         state
	interfaces    []NetInterface
	networks      []Network
	ifaceCursor   int
	netCursor     int
	selectedIface *NetInterface
	password      string
	message       string
	currentSSID   string
	sysStatus     SystemStatus
	lastError     string
}

// Messages
type interfacesLoadedMsg struct {
	interfaces []NetInterface
	sysStatus  SystemStatus
}

type scanDoneMsg struct {
	networks []Network
	err      error
	sysStatus SystemStatus
}

type connectDoneMsg struct {
	success bool
	err     error
	sysStatus SystemStatus
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
	return model{
		state: stateInterfaces,
	}
}

func (m model) Init() tea.Cmd {
	return loadInterfaces
}

func loadInterfaces() tea.Msg {
	interfaces := getAllInterfaces()
	sysStatus := getSystemStatus()
	return interfacesLoadedMsg{interfaces: interfaces, sysStatus: sysStatus}
}

func (m model) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.KeyMsg:
		return m.handleKey(msg)

	case interfacesLoadedMsg:
		m.interfaces = msg.interfaces
		m.sysStatus = msg.sysStatus
		m.state = stateInterfaces
		return m, nil

	case scanDoneMsg:
		m.sysStatus = msg.sysStatus
		if msg.err != nil {
			m.lastError = msg.err.Error()
			m.networks = nil
		} else {
			m.networks = msg.networks
			m.lastError = ""
		}
		m.state = stateNetworkList
		return m, nil

	case connectDoneMsg:
		m.sysStatus = msg.sysStatus
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
	case stateInterfaces:
		switch msg.String() {
		case "q", "ctrl+c":
			return m, tea.Quit
		case "up", "k":
			if m.ifaceCursor > 0 {
				m.ifaceCursor--
			}
		case "down", "j":
			if m.ifaceCursor < len(m.interfaces)-1 {
				m.ifaceCursor++
			}
		case "enter":
			if len(m.interfaces) > 0 {
				iface := m.interfaces[m.ifaceCursor]
				m.selectedIface = &iface
				if iface.IsWireless {
					m.state = stateScanning
					return m, scanNetworks(iface.Name, m.sysStatus)
				} else {
					m.state = stateInterfaceInfo
				}
			}
		case "i":
			// Show info for selected interface
			if len(m.interfaces) > 0 {
				iface := m.interfaces[m.ifaceCursor]
				m.selectedIface = &iface
				m.state = stateInterfaceInfo
			}
		case "r":
			// Refresh interfaces
			return m, loadInterfaces
		case "u":
			// Bring interface up
			if len(m.interfaces) > 0 {
				iface := m.interfaces[m.ifaceCursor]
				exec.Command("ip", "link", "set", iface.Name, "up").Run()
				return m, loadInterfaces
			}
		case "d":
			// Bring interface down
			if len(m.interfaces) > 0 {
				iface := m.interfaces[m.ifaceCursor]
				exec.Command("ip", "link", "set", iface.Name, "down").Run()
				return m, loadInterfaces
			}
		}

	case stateInterfaceInfo:
		switch msg.String() {
		case "q", "ctrl+c":
			return m, tea.Quit
		case "esc", "b":
			m.state = stateInterfaces
			m.selectedIface = nil
		case "s":
			// Scan if wireless
			if m.selectedIface != nil && m.selectedIface.IsWireless {
				m.state = stateScanning
				return m, scanNetworks(m.selectedIface.Name, m.sysStatus)
			}
		case "u":
			if m.selectedIface != nil {
				exec.Command("ip", "link", "set", m.selectedIface.Name, "up").Run()
				return m, loadInterfaces
			}
		case "d":
			if m.selectedIface != nil {
				exec.Command("ip", "link", "set", m.selectedIface.Name, "down").Run()
				return m, loadInterfaces
			}
		}

	case stateNetworkList:
		switch msg.String() {
		case "q", "ctrl+c":
			return m, tea.Quit
		case "esc", "b":
			m.state = stateInterfaces
			m.selectedIface = nil
			m.networks = nil
		case "up", "k":
			if m.netCursor > 0 {
				m.netCursor--
			}
		case "down", "j":
			if m.netCursor < len(m.networks)-1 {
				m.netCursor++
			}
		case "enter":
			if len(m.networks) > 0 {
				net := m.networks[m.netCursor]
				if net.Connected {
					return m, nil // Already connected
				}
				m.currentSSID = net.SSID
				if net.Security != "" && net.Security != "Open" {
					if isKnownNetwork(net.SSID) {
						m.state = stateConnecting
						m.message = "Connecting..."
						return m, connectToNetwork(m.selectedIface.Name, net.SSID, "", m.sysStatus)
					}
					m.state = statePassword
					m.password = ""
				} else {
					m.state = stateConnecting
					m.message = "Connecting..."
					return m, connectToNetwork(m.selectedIface.Name, net.SSID, "", m.sysStatus)
				}
			}
		case "r":
			m.state = stateScanning
			m.netCursor = 0
			return m, scanNetworks(m.selectedIface.Name, m.sysStatus)
		case "D":
			// Disconnect
			if m.selectedIface != nil {
				disconnect(m.selectedIface.Name)
				m.state = stateScanning
				return m, scanNetworks(m.selectedIface.Name, m.sysStatus)
			}
		case "i":
			m.state = stateInterfaceInfo
		}

	case statePassword:
		switch msg.String() {
		case "esc":
			m.state = stateNetworkList
			m.password = ""
		case "enter":
			if m.password != "" {
				m.state = stateConnecting
				m.message = "Connecting..."
				return m, connectToNetwork(m.selectedIface.Name, m.currentSSID, m.password, m.sysStatus)
			}
		case "backspace":
			if len(m.password) > 0 {
				m.password = m.password[:len(m.password)-1]
			}
		case "ctrl+c":
			return m, tea.Quit
		default:
			if len(msg.String()) == 1 {
				m.password += msg.String()
			}
		}

	case stateSuccess:
		switch msg.String() {
		case "enter", "q", "esc":
			return m, tea.Quit
		case "b":
			m.state = stateNetworkList
			return m, scanNetworks(m.selectedIface.Name, m.sysStatus)
		}

	case stateError:
		switch msg.String() {
		case "enter", "r":
			if m.selectedIface != nil && m.selectedIface.IsWireless {
				m.state = stateScanning
				return m, scanNetworks(m.selectedIface.Name, m.sysStatus)
			}
			m.state = stateInterfaces
		case "b":
			m.state = stateInterfaces
		case "q", "esc", "ctrl+c":
			return m, tea.Quit
		}

	case stateScanning:
		switch msg.String() {
		case "ctrl+c", "q":
			return m, tea.Quit
		}
	}

	return m, nil
}

func (m model) View() string {
	var s strings.Builder
	var content strings.Builder

	// Header with logo
	header := lipgloss.NewStyle().
		Bold(true).
		Foreground(lipgloss.Color("#00BCD4")).
		Render("  RAVEN WIFI  ")

	s.WriteString("\n")
	s.WriteString(lipgloss.NewStyle().
		Border(lipgloss.DoubleBorder()).
		BorderForeground(lipgloss.Color("#00BCD4")).
		Padding(0, 1).
		Render(header))
	s.WriteString("\n\n")

	// System status bar (compact)
	s.WriteString(m.renderSystemStatus())
	s.WriteString("\n")
	s.WriteString(dividerStyle.Render(strings.Repeat("─", 60)))
	s.WriteString("\n\n")

	switch m.state {
	case stateInterfaces:
		content.WriteString(m.renderInterfaceList())

	case stateInterfaceInfo:
		content.WriteString(m.renderInterfaceInfo())

	case stateScanning:
		content.WriteString(lipgloss.NewStyle().Foreground(lipgloss.Color("#00BCD4")).Render("◐ "))
		content.WriteString(fmt.Sprintf("Scanning on %s...\n\n", m.selectedIface.Name))
		content.WriteString(dimStyle.Render("  Looking for available networks..."))

	case stateNetworkList:
		content.WriteString(m.renderNetworkList())

	case statePassword:
		content.WriteString(sectionStyle.Render("Connect to Network"))
		content.WriteString("\n\n")
		content.WriteString(fmt.Sprintf("  Network: %s\n\n", lipgloss.NewStyle().Bold(true).Foreground(lipgloss.Color("#00BCD4")).Render(m.currentSSID)))
		content.WriteString("  Password: ")
		content.WriteString(lipgloss.NewStyle().Foreground(lipgloss.Color("#00BCD4")).Render(strings.Repeat("●", len(m.password))))
		content.WriteString(lipgloss.NewStyle().Foreground(lipgloss.Color("#00BCD4")).Blink(true).Render("│"))
		content.WriteString("\n\n")
		content.WriteString(helpStyle.Render("  Enter: Connect  •  Esc: Cancel"))

	case stateConnecting:
		content.WriteString(lipgloss.NewStyle().Foreground(lipgloss.Color("#00BCD4")).Render("◐ "))
		content.WriteString(fmt.Sprintf("Connecting to %s...\n\n", lipgloss.NewStyle().Bold(true).Render(m.currentSSID)))
		content.WriteString(dimStyle.Render("  Establishing connection and obtaining IP..."))

	case stateSuccess:
		successBox := lipgloss.NewStyle().
			Border(lipgloss.RoundedBorder()).
			BorderForeground(lipgloss.Color("#4CAF50")).
			Padding(1, 2).
			Render(successStyle.Render("✓ ") + m.message + "\n\n" + dimStyle.Render("Network saved for future connections."))
		content.WriteString(successBox)
		content.WriteString("\n\n")
		content.WriteString(helpStyle.Render("Enter/q: Exit  •  b: Back"))

	case stateError:
		errorBox := lipgloss.NewStyle().
			Border(lipgloss.RoundedBorder()).
			BorderForeground(lipgloss.Color("#F44336")).
			Padding(1, 2).
			Render(errorStyle.Render("✗ Error\n\n") + m.message)
		content.WriteString(errorBox)
		content.WriteString("\n\n")
		content.WriteString(helpStyle.Render("r: Retry  •  b: Back  •  q: Quit"))
	}

	s.WriteString(content.String())
	s.WriteString("\n")

	return s.String()
}

func (m model) renderSystemStatus() string {
	var parts []string

	if m.sysStatus.DBusRunning {
		parts = append(parts, badgeOK.Render("D-Bus"))
	} else {
		parts = append(parts, badgeOff.Render("D-Bus"))
	}

	if m.sysStatus.IWDRunning {
		parts = append(parts, badgeOK.Render("iwd"))
	} else if m.sysStatus.WPARunning {
		parts = append(parts, badgeOff.Render("iwd"))
	} else {
		parts = append(parts, badgeWarn.Render("iwd"))
	}

	if m.sysStatus.WPARunning {
		parts = append(parts, badgeOK.Render("wpa"))
	} else if m.sysStatus.IWDRunning {
		parts = append(parts, badgeOff.Render("wpa"))
	} else {
		parts = append(parts, badgeWarn.Render("wpa"))
	}

	return subtitleStyle.Render("Status: ") + strings.Join(parts, " ")
}

func (m model) renderInterfaceList() string {
	var s strings.Builder

	if len(m.interfaces) == 0 {
		errorBox := lipgloss.NewStyle().
			Border(lipgloss.RoundedBorder()).
			BorderForeground(lipgloss.Color("#F44336")).
			Padding(1, 2).
			Render(errorStyle.Render("No network interfaces found!\n\n") +
				"Possible causes:\n" +
				dimStyle.Render("  • Network drivers not loaded\n") +
				dimStyle.Render("  • /sys/class/net not mounted\n") +
				dimStyle.Render("  • Hardware not detected"))
		s.WriteString(errorBox)
		s.WriteString("\n\n")
		s.WriteString(helpStyle.Render("r: Refresh  •  q: Quit"))
		return s.String()
	}

	s.WriteString(sectionStyle.Render("Select Interface"))
	s.WriteString("\n\n")

	// Count by type
	wireless := 0
	ethernet := 0
	for _, iface := range m.interfaces {
		if iface.IsWireless {
			wireless++
		} else if iface.Type == "ethernet" {
			ethernet++
		}
	}
	s.WriteString(dimStyle.Render(fmt.Sprintf("  %d wireless • %d ethernet • %d total\n\n",
		wireless, ethernet, len(m.interfaces))))

	for i, iface := range m.interfaces {
		line := m.formatInterfaceLine(iface)

		if i == m.ifaceCursor {
			s.WriteString(selectedStyle.Render(" ▶ " + line))
		} else if iface.IsWireless {
			s.WriteString(wirelessStyle.Render("   " + line))
		} else if iface.Type == "ethernet" {
			s.WriteString(ethernetStyle.Render("   " + line))
		} else {
			s.WriteString(dimStyle.Render("   " + line))
		}
		s.WriteString("\n")
	}

	s.WriteString("\n")
	s.WriteString(helpStyle.Render("↑↓: Navigate  •  Enter: Select  •  i: Info  •  u/d: Up/Down  •  r: Refresh  •  q: Quit"))
	return s.String()
}

func (m model) formatInterfaceLine(iface NetInterface) string {
	// Type indicator (text-based for better terminal compatibility)
	typeStr := ""
	switch iface.Type {
	case "wireless":
		typeStr = "wifi"
	case "ethernet":
		typeStr = "eth "
	case "loopback":
		typeStr = "lo  "
	case "virtual":
		typeStr = "virt"
	default:
		typeStr = "    "
	}

	// State indicator with visual cue
	stateStr := ""
	if iface.State == "up" {
		stateStr = "▲ UP"
		if iface.IP != "" {
			stateStr = "▲ " + iface.IP
		}
	} else {
		stateStr = "▼ DOWN"
	}

	// Driver info
	driverStr := ""
	if iface.Driver != "" {
		driverStr = fmt.Sprintf("[%s]", iface.Driver)
	}

	return fmt.Sprintf("[%s] %-10s  %-16s  %s", typeStr, iface.Name, stateStr, driverStr)
}

func (m model) renderInterfaceInfo() string {
	var s strings.Builder
	iface := m.selectedIface

	s.WriteString(sectionStyle.Render("Interface Details"))
	s.WriteString("\n\n")

	// Format info in a clean table
	labelStyle := lipgloss.NewStyle().Foreground(lipgloss.Color("#888")).Width(14)
	valueStyle := lipgloss.NewStyle().Foreground(lipgloss.Color("#00BCD4"))

	info := [][]string{
		{"Name", iface.Name},
		{"Type", iface.Type},
		{"MAC", iface.MAC},
		{"State", iface.State},
		{"Driver", iface.Driver},
		{"IP Address", iface.IP},
		{"Carrier", fmt.Sprintf("%v", iface.HasCarrier)},
	}

	for _, row := range info {
		label := row[0]
		value := row[1]
		if value == "" {
			value = "—"
		}
		s.WriteString("  ")
		s.WriteString(labelStyle.Render(label + ":"))
		s.WriteString("  ")
		s.WriteString(valueStyle.Render(value))
		s.WriteString("\n")
	}

	s.WriteString("\n")

	// Show additional info for wireless
	if iface.IsWireless {
		tipBox := lipgloss.NewStyle().
			Border(lipgloss.RoundedBorder()).
			BorderForeground(lipgloss.Color("#00BCD4")).
			Padding(0, 1).
			Render("Press 's' to scan for WiFi networks")
		s.WriteString(tipBox)
		s.WriteString("\n")
	}

	// Show warnings
	if iface.IsWireless && !m.sysStatus.DBusRunning {
		s.WriteString("\n")
		warnBox := lipgloss.NewStyle().
			Border(lipgloss.RoundedBorder()).
			BorderForeground(lipgloss.Color("#FFC107")).
			Padding(0, 1).
			Render(warnStyle.Render("⚠ D-Bus not running") + "\n" + dimStyle.Render("iwd/iwctl requires D-Bus"))
		s.WriteString(warnBox)
	}

	if iface.IsWireless && !m.sysStatus.IWDRunning && !m.sysStatus.WPARunning {
		s.WriteString("\n")
		warnBox := lipgloss.NewStyle().
			Border(lipgloss.RoundedBorder()).
			BorderForeground(lipgloss.Color("#FFC107")).
			Padding(0, 1).
			Render(warnStyle.Render("⚠ No WiFi daemon") + "\n" + dimStyle.Render("Start iwd or wpa_supplicant"))
		s.WriteString(warnBox)
	}

	help := "\nb: Back"
	if iface.IsWireless {
		help += "  •  s: Scan"
	}
	help += "  •  u: Up  •  d: Down  •  q: Quit"
	s.WriteString(helpStyle.Render(help))

	return s.String()
}

func (m model) renderNetworkList() string {
	var s strings.Builder

	// Header with interface info
	ifaceInfo := fmt.Sprintf("Interface: %s", m.selectedIface.Name)
	if m.selectedIface.Driver != "" {
		ifaceInfo += fmt.Sprintf(" [%s]", m.selectedIface.Driver)
	}
	s.WriteString(dimStyle.Render(ifaceInfo))
	s.WriteString("\n\n")

	// Show any error from scanning
	if m.lastError != "" {
		errMsg := m.lastError
		if len(errMsg) > 50 {
			errMsg = errMsg[:50] + "..."
		}
		s.WriteString(warnStyle.Render("⚠ "))
		s.WriteString(dimStyle.Render(errMsg))
		s.WriteString("\n\n")
	}

	if len(m.networks) == 0 {
		emptyBox := lipgloss.NewStyle().
			Border(lipgloss.RoundedBorder()).
			BorderForeground(lipgloss.Color("#444")).
			Padding(1, 2).
			Render("No networks found.\n\n" +
				dimStyle.Render("  • No WiFi networks in range\n") +
				dimStyle.Render("  • Interface not fully up\n") +
				dimStyle.Render("  • Driver issues"))
		s.WriteString(emptyBox)
		s.WriteString("\n\n")
		s.WriteString(helpStyle.Render("r: Rescan  •  i: Info  •  b: Back  •  q: Quit"))
		return s.String()
	}

	// Show connected status
	for _, net := range m.networks {
		if net.Connected {
			connBox := lipgloss.NewStyle().
				Border(lipgloss.RoundedBorder()).
				BorderForeground(lipgloss.Color("#4CAF50")).
				Padding(0, 1).
				Render(successStyle.Render("✓ Connected: ") + net.SSID)
			s.WriteString(connBox)
			s.WriteString("\n\n")
			break
		}
	}

	s.WriteString(sectionStyle.Render("Available Networks"))
	s.WriteString("\n\n")

	for i, net := range m.networks {
		signal := getSignalBars(net.Signal)
		lock := "○"
		if net.Security != "" && net.Security != "Open" {
			lock = "●"
		}

		line := fmt.Sprintf("%s  %s  %s", signal, lock, net.SSID)
		if net.Connected {
			line += "  ✓"
		}

		if i == m.netCursor {
			s.WriteString(selectedStyle.Render(" ▶ " + line))
		} else if net.Connected {
			s.WriteString(connectedStyle.Render("   " + line))
		} else {
			s.WriteString(normalStyle.Render("   " + line))
		}
		s.WriteString("\n")
	}

	s.WriteString("\n")
	s.WriteString(helpStyle.Render("↑↓: Navigate  •  Enter: Connect  •  r: Rescan  •  D: Disconnect  •  b: Back  •  q: Quit"))
	return s.String()
}

func getSignalBars(signal int) string {
	if signal >= 75 {
		return signalGood.Render("████")
	} else if signal >= 50 {
		return signalMed.Render("███") + "░"
	} else if signal >= 25 {
		return signalBad.Render("██") + "░░"
	}
	return signalBad.Render("█") + "░░░"
}

// ============================================================================
// Network Interface Detection
// ============================================================================

func getAllInterfaces() []NetInterface {
	var interfaces []NetInterface

	entries, err := os.ReadDir("/sys/class/net")
	if err != nil {
		return interfaces
	}

	for _, entry := range entries {
		name := entry.Name()
		iface := NetInterface{
			Name: name,
		}

		basePath := "/sys/class/net/" + name

		// Check if wireless
		if _, err := os.Stat(basePath + "/wireless"); err == nil {
			iface.IsWireless = true
			iface.Type = "wireless"
		} else if name == "lo" {
			iface.Type = "loopback"
		} else if strings.HasPrefix(name, "eth") || strings.HasPrefix(name, "enp") || strings.HasPrefix(name, "eno") {
			iface.Type = "ethernet"
		} else if strings.HasPrefix(name, "veth") || strings.HasPrefix(name, "docker") || strings.HasPrefix(name, "br-") {
			iface.Type = "virtual"
		} else {
			iface.Type = "unknown"
			// Try to determine if ethernet by checking for /device
			if _, err := os.Stat(basePath + "/device"); err == nil {
				iface.Type = "ethernet"
			}
		}

		// Get MAC address
		if mac, err := os.ReadFile(basePath + "/address"); err == nil {
			iface.MAC = strings.TrimSpace(string(mac))
		}

		// Get state (operstate)
		if state, err := os.ReadFile(basePath + "/operstate"); err == nil {
			iface.State = strings.TrimSpace(string(state))
		}

		// Get carrier
		if carrier, err := os.ReadFile(basePath + "/carrier"); err == nil {
			iface.HasCarrier = strings.TrimSpace(string(carrier)) == "1"
		}

		// Get driver
		driverLink := basePath + "/device/driver"
		if target, err := os.Readlink(driverLink); err == nil {
			iface.Driver = filepath.Base(target)
		}

		// Get device path (PCI/USB)
		deviceLink := basePath + "/device"
		if target, err := os.Readlink(deviceLink); err == nil {
			// Extract meaningful part of path
			parts := strings.Split(target, "/")
			for i, p := range parts {
				if strings.Contains(p, ":") && (strings.HasPrefix(p, "0000:") || strings.Contains(p, "usb")) {
					iface.DevicePath = strings.Join(parts[i:], "/")
					if len(iface.DevicePath) > 30 {
						iface.DevicePath = "..." + iface.DevicePath[len(iface.DevicePath)-27:]
					}
					break
				}
			}
		}

		// Get IP address
		iface.IP = getInterfaceIP(name)

		interfaces = append(interfaces, iface)
	}

	// Sort: wireless first, then ethernet, then others
	sort.Slice(interfaces, func(i, j int) bool {
		typeOrder := map[string]int{"wireless": 0, "ethernet": 1, "loopback": 3, "virtual": 4, "unknown": 2}
		return typeOrder[interfaces[i].Type] < typeOrder[interfaces[j].Type]
	})

	return interfaces
}

func getInterfaceIP(name string) string {
	cmd := exec.Command("ip", "-4", "addr", "show", name)
	output, err := cmd.Output()
	if err != nil {
		return ""
	}

	re := regexp.MustCompile(`inet (\d+\.\d+\.\d+\.\d+)`)
	matches := re.FindStringSubmatch(string(output))
	if len(matches) >= 2 {
		return matches[1]
	}
	return ""
}

// ============================================================================
// System Status Checks
// ============================================================================

func isIWDRunning() bool {
	cmd := exec.Command("pgrep", "-x", "iwd")
	if err := cmd.Run(); err == nil {
		return true
	}
	if _, err := os.Stat("/run/iwd"); err == nil {
		return true
	}
	return false
}

func isDBusRunning() bool {
	cmd := exec.Command("pgrep", "-x", "dbus-daemon")
	if err := cmd.Run(); err == nil {
		return true
	}
	if _, err := os.Stat("/run/dbus/system_bus_socket"); err == nil {
		return true
	}
	return false
}

func isWPARunning() bool {
	cmd := exec.Command("pgrep", "-x", "wpa_supplicant")
	return cmd.Run() == nil
}

// ============================================================================
// WiFi Scanning
// ============================================================================

func scanNetworks(iface string, status SystemStatus) tea.Cmd {
	return func() tea.Msg {
		// Ensure WiFi daemon is running (also brings up interface)
		ensureWiFiDaemons(iface)
		status = getSystemStatus()

		// Give it a moment
		time.Sleep(500 * time.Millisecond)

		var networks []Network
		var lastErr error

		// Try iwd if available and running
		if status.IWDRunning {
			if _, err := exec.LookPath("iwctl"); err == nil {
				networks, lastErr = scanWithIWD(iface)
			}
		}

		// Fallback to wpa_supplicant
		if len(networks) == 0 && status.WPARunning {
			if _, err := exec.LookPath("wpa_cli"); err == nil {
				var err error
				networks, err = scanWithWPA(iface)
				if err != nil {
					lastErr = err
				}
			}
		}

		// Fallback to raw iw scan
		if len(networks) == 0 {
			var err error
			networks, err = scanWithIW(iface)
			if err != nil {
				lastErr = err
			}
		}

		// Mark connected network
		currentSSID := getCurrentSSID(iface)
		for i := range networks {
			if networks[i].SSID == currentSSID {
				networks[i].Connected = true
			}
		}

		// Sort and dedupe
		sort.Slice(networks, func(i, j int) bool {
			if networks[i].Connected != networks[j].Connected {
				return networks[i].Connected
			}
			return networks[i].Signal > networks[j].Signal
		})

		seen := make(map[string]bool)
		unique := []Network{}
		for _, n := range networks {
			if n.SSID != "" && !seen[n.SSID] {
				seen[n.SSID] = true
				unique = append(unique, n)
			}
		}

		return scanDoneMsg{networks: unique, err: lastErr, sysStatus: status}
	}
}

func scanWithIWD(iface string) ([]Network, error) {
	// Trigger scan
	if out, err := exec.Command("iwctl", "station", iface, "scan").CombinedOutput(); err != nil {
		return nil, fmt.Errorf("iwctl scan: %v: %s", err, strings.TrimSpace(string(out)))
	}
	time.Sleep(3 * time.Second)

	// Get networks
	cmd := exec.Command("iwctl", "station", iface, "get-networks")
	output, err := cmd.CombinedOutput()
	if err != nil {
		return nil, fmt.Errorf("iwctl get-networks: %v: %s", err, strings.TrimSpace(string(output)))
	}

	var networks []Network
	lines := strings.Split(string(output), "\n")

	for _, line := range lines {
		clean := regexp.MustCompile(`\x1b\[[0-9;]*m`).ReplaceAllString(line, "")
		clean = strings.TrimSpace(clean)

		if clean == "" || strings.HasPrefix(clean, "Available") || strings.HasPrefix(clean, "---") {
			continue
		}

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

		for j := startIdx + 1; j < len(fields); j++ {
			f := fields[j]
			if strings.Contains(strings.ToLower(f), "psk") || strings.Contains(strings.ToLower(f), "wpa") {
				security = "WPA2"
			}
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

func scanWithWPA(iface string) ([]Network, error) {
	exec.Command("wpa_cli", "-i", iface, "scan").Run()
	time.Sleep(3 * time.Second)

	cmd := exec.Command("wpa_cli", "-i", iface, "scan_results")
	output, err := cmd.Output()
	if err != nil {
		return nil, fmt.Errorf("wpa_cli: %v", err)
	}

	var networks []Network
	lines := strings.Split(string(output), "\n")

	for i, line := range lines {
		if i == 0 || len(strings.TrimSpace(line)) == 0 {
			continue
		}

		fields := strings.Fields(line)
		if len(fields) < 5 {
			continue
		}

		signalDBM, _ := strconv.Atoi(fields[2])
		flags := fields[3]
		ssid := strings.Join(fields[4:], " ")

		signal := dbmToPercent(signalDBM)
		security := "Open"
		if strings.Contains(flags, "WPA") {
			security = "WPA2"
		} else if strings.Contains(flags, "WEP") {
			security = "WEP"
		}

		networks = append(networks, Network{
			SSID:     ssid,
			Signal:   signal,
			Security: security,
		})
	}

	return networks, nil
}

func scanWithIW(iface string) ([]Network, error) {
	cmd := exec.Command("iw", "dev", iface, "scan")
	output, err := cmd.CombinedOutput()
	if err != nil {
		return nil, fmt.Errorf("iw scan: %v: %s", err, strings.TrimSpace(string(output)))
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

// ============================================================================
// Connection
// ============================================================================

func connectToNetwork(iface, ssid, password string, status SystemStatus) tea.Cmd {
	return func() tea.Msg {
		// Ensure WiFi daemon is running (also brings up interface)
		ensureWiFiDaemons(iface)
		status = getSystemStatus()

		var err error

		// Use best available method
		if status.IWDRunning {
			err = connectWithIWD(iface, ssid, password)
		} else {
			err = connectWithWPA(iface, ssid, password)
		}

		if err != nil {
			return connectDoneMsg{success: false, err: err, sysStatus: status}
		}

		// Wait for association + get IP
		time.Sleep(3 * time.Second)
		requestDHCP(iface)
		time.Sleep(2 * time.Second)

		// Verify connection
		if getCurrentSSID(iface) == ssid {
			return connectDoneMsg{success: true, sysStatus: status}
		}

		return connectDoneMsg{success: false, err: fmt.Errorf("connection verification failed"), sysStatus: status}
	}
}

func connectWithIWD(iface, ssid, password string) error {
	if password != "" {
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

	exec.Command("killall", "wpa_supplicant").Run()
	time.Sleep(500 * time.Millisecond)

	cmd := exec.Command("wpa_supplicant", "-B", "-i", iface, "-c", configPath)
	if err := cmd.Run(); err != nil {
		return err
	}

	return nil
}

func requestDHCP(iface string) {
	exec.Command("killall", "dhcpcd").Run()
	exec.Command("killall", "dhclient").Run()
	exec.Command("killall", "udhcpc").Run()

	if _, err := exec.LookPath("dhcpcd"); err == nil {
		exec.Command("dhcpcd", "-n", iface).Run()
		return
	}
	if _, err := exec.LookPath("dhclient"); err == nil {
		exec.Command("dhclient", iface).Run()
		return
	}
	if _, err := exec.LookPath("udhcpc"); err == nil {
		exec.Command("udhcpc", "-i", iface, "-n", "-q").Run()
		return
	}
	if _, err := exec.LookPath("raven-dhcp"); err == nil {
		exec.Command("raven-dhcp", "-i", iface).Run()
	}
}

func getSystemStatus() SystemStatus {
	return SystemStatus{
		DBusRunning: isDBusRunning(),
		IWDRunning:  isIWDRunning(),
		WPARunning:  isWPARunning(),
	}
}

func ensureWiFiDaemons(iface string) {
	// Bring up interface first
	exec.Command("rfkill", "unblock", "wifi").Run()
	exec.Command("ip", "link", "set", iface, "up").Run()
	time.Sleep(200 * time.Millisecond)

	// Try iwd first (preferred - simpler, modern)
	if _, err := exec.LookPath("iwd"); err == nil && !isIWDRunning() {
		// Ensure D-Bus is running (required by iwd)
		if !isDBusRunning() {
			if _, err := exec.LookPath("dbus-daemon"); err == nil {
				_ = os.MkdirAll("/run/dbus", 0755)
				if _, err := exec.LookPath("dbus-uuidgen"); err == nil {
					exec.Command("dbus-uuidgen", "--ensure=/etc/machine-id").Run()
				}
				_ = exec.Command("dbus-daemon", "--system", "--fork", "--nopidfile").Start()
				time.Sleep(150 * time.Millisecond)
			}
		}

		_ = os.MkdirAll("/var/lib/iwd", 0755)
		if _, err := os.Stat("/usr/libexec/iwd"); err == nil {
			_ = exec.Command("/usr/libexec/iwd").Start()
		} else {
			_ = exec.Command("iwd").Start()
		}
		time.Sleep(300 * time.Millisecond)

		if isIWDRunning() {
			return // iwd started successfully
		}
	}

	// Fallback to wpa_supplicant
	if _, err := exec.LookPath("wpa_supplicant"); err == nil && !isWPARunning() {
		_ = os.MkdirAll("/etc/wpa_supplicant", 0755)
		_ = os.MkdirAll("/run/wpa_supplicant", 0755)

		configPath := "/etc/wpa_supplicant/wpa_supplicant.conf"
		if _, err := os.Stat(configPath); os.IsNotExist(err) {
			baseConfig := "ctrl_interface=/run/wpa_supplicant\nupdate_config=1\n"
			os.WriteFile(configPath, []byte(baseConfig), 0600)
		}

		exec.Command("wpa_supplicant", "-B", "-i", iface, "-c", configPath).Run()
		time.Sleep(500 * time.Millisecond)
	}
}

func disconnect(iface string) {
	if _, err := exec.LookPath("iwctl"); err == nil {
		if exec.Command("iwctl", "station", iface, "disconnect").Run() == nil {
			return
		}
	}
	exec.Command("wpa_cli", "-i", iface, "disconnect").Run()
}

func isKnownNetwork(ssid string) bool {
	safeName := strings.ReplaceAll(ssid, " ", "_")

	if _, err := os.Stat(fmt.Sprintf("/var/lib/iwd/%s.psk", safeName)); err == nil {
		return true
	}
	if _, err := os.Stat(fmt.Sprintf("/var/lib/iwd/%s.open", safeName)); err == nil {
		return true
	}

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
