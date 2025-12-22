package main

import (
	"encoding/json"
	"fmt"
	"net"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
	"syscall"
	"time"
)

const (
	socketName    = "raven-compositor.sock"
	socketTimeout = 2 * time.Second
)

// Command represents an IPC command to the compositor
type Command struct {
	Action string `json:"action"`
	PID    int    `json:"pid,omitempty"`
	WinID  string `json:"win_id,omitempty"`
}

// Response represents an IPC response from the compositor
type Response struct {
	Success bool   `json:"success"`
	Error   string `json:"error,omitempty"`
	Data    any    `json:"data,omitempty"`
}

// WindowInfo represents information about a window
type WindowInfo struct {
	ID        string `json:"id"`
	PID       int    `json:"pid"`
	Title     string `json:"title"`
	AppID     string `json:"app_id"`
	Focused   bool   `json:"focused"`
	Minimized bool   `json:"minimized"`
}

func main() {
	if len(os.Args) < 2 {
		printUsage()
		os.Exit(1)
	}

	cmd := os.Args[1]

	switch cmd {
	case "help", "-h", "--help":
		printUsage()

	case "focus":
		if len(os.Args) < 3 {
			fmt.Fprintln(os.Stderr, "Error: focus requires a PID argument")
			os.Exit(1)
		}
		pid, err := strconv.Atoi(os.Args[2])
		if err != nil {
			fmt.Fprintf(os.Stderr, "Error: invalid PID '%s'\n", os.Args[2])
			os.Exit(1)
		}
		if err := focusWindow(pid); err != nil {
			fmt.Fprintf(os.Stderr, "Error: %v\n", err)
			os.Exit(1)
		}

	case "minimize":
		if len(os.Args) < 3 {
			fmt.Fprintln(os.Stderr, "Error: minimize requires a PID argument")
			os.Exit(1)
		}
		pid, err := strconv.Atoi(os.Args[2])
		if err != nil {
			fmt.Fprintf(os.Stderr, "Error: invalid PID '%s'\n", os.Args[2])
			os.Exit(1)
		}
		if err := minimizeWindow(pid); err != nil {
			fmt.Fprintf(os.Stderr, "Error: %v\n", err)
			os.Exit(1)
		}

	case "restore":
		if len(os.Args) < 3 {
			fmt.Fprintln(os.Stderr, "Error: restore requires a PID argument")
			os.Exit(1)
		}
		pid, err := strconv.Atoi(os.Args[2])
		if err != nil {
			fmt.Fprintf(os.Stderr, "Error: invalid PID '%s'\n", os.Args[2])
			os.Exit(1)
		}
		if err := restoreWindow(pid); err != nil {
			fmt.Fprintf(os.Stderr, "Error: %v\n", err)
			os.Exit(1)
		}

	case "close":
		if len(os.Args) < 3 {
			fmt.Fprintln(os.Stderr, "Error: close requires a PID argument")
			os.Exit(1)
		}
		pid, err := strconv.Atoi(os.Args[2])
		if err != nil {
			fmt.Fprintf(os.Stderr, "Error: invalid PID '%s'\n", os.Args[2])
			os.Exit(1)
		}
		if err := closeWindow(pid); err != nil {
			fmt.Fprintf(os.Stderr, "Error: %v\n", err)
			os.Exit(1)
		}

	case "list", "list-windows":
		windows, err := listWindows()
		if err != nil {
			fmt.Fprintf(os.Stderr, "Error: %v\n", err)
			os.Exit(1)
		}
		for _, w := range windows {
			status := ""
			if w.Focused {
				status = "[focused]"
			} else if w.Minimized {
				status = "[minimized]"
			}
			fmt.Printf("%d\t%s\t%s %s\n", w.PID, w.AppID, w.Title, status)
		}

	case "active", "get-active":
		window, err := getActiveWindow()
		if err != nil {
			fmt.Fprintf(os.Stderr, "Error: %v\n", err)
			os.Exit(1)
		}
		if window != nil {
			fmt.Printf("%d\t%s\t%s\n", window.PID, window.AppID, window.Title)
		}

	case "version", "-v", "--version":
		fmt.Println("raven-ctl version 0.1.0")

	default:
		fmt.Fprintf(os.Stderr, "Unknown command: %s\n", cmd)
		printUsage()
		os.Exit(1)
	}
}

func printUsage() {
	fmt.Println(`raven-ctl - Raven Desktop window control utility

Usage:
  raven-ctl <command> [arguments]

Commands:
  focus <pid>       Focus the window belonging to the process
  minimize <pid>    Minimize the window belonging to the process
  restore <pid>     Restore a minimized window
  close <pid>       Close the window (graceful termination)
  list              List all windows
  active            Get the currently active window
  version           Print version information
  help              Print this help message

Examples:
  raven-ctl focus 12345
  raven-ctl minimize 12345
  raven-ctl list`)
}

// capitalizeFirst capitalizes the first letter of a string
func capitalizeFirst(s string) string {
	if s == "" {
		return s
	}
	return strings.ToUpper(s[:1]) + s[1:]
}

// getSocketPath returns the path to the compositor socket
func getSocketPath() string {
	runtimeDir := os.Getenv("XDG_RUNTIME_DIR")
	if runtimeDir == "" {
		runtimeDir = fmt.Sprintf("/run/user/%d", os.Getuid())
	}
	return filepath.Join(runtimeDir, socketName)
}

// sendCommand sends a command to the compositor via IPC
func sendCommand(cmd Command) (*Response, error) {
	socketPath := getSocketPath()

	conn, err := net.DialTimeout("unix", socketPath, socketTimeout)
	if err != nil {
		return nil, fmt.Errorf("compositor not available: %w", err)
	}
	defer conn.Close()

	conn.SetDeadline(time.Now().Add(socketTimeout))

	encoder := json.NewEncoder(conn)
	if err := encoder.Encode(cmd); err != nil {
		return nil, fmt.Errorf("failed to send command: %w", err)
	}

	var resp Response
	decoder := json.NewDecoder(conn)
	if err := decoder.Decode(&resp); err != nil {
		return nil, fmt.Errorf("failed to read response: %w", err)
	}

	return &resp, nil
}

// tryCompositorIPC attempts to send a command via the compositor socket
func tryCompositorIPC(action string, pid int) error {
	cmd := Command{
		Action: action,
		PID:    pid,
	}

	resp, err := sendCommand(cmd)
	if err != nil {
		return err
	}

	if !resp.Success {
		return fmt.Errorf("%s", resp.Error)
	}

	return nil
}

// focusWindow focuses a window by PID
func focusWindow(pid int) error {
	// Try compositor IPC first
	if err := tryCompositorIPC("focus", pid); err == nil {
		return nil
	}

	// Fallback: try wlr-foreign-toplevel via hyprctl (if Hyprland)
	if err := tryHyprctl("focuswindow", pid); err == nil {
		return nil
	}

	// Fallback: try swaymsg (if Sway)
	if err := trySwaymsg("focus", pid); err == nil {
		return nil
	}

	// Fallback: try generic wlroots approach
	if err := tryWlrForeignToplevel("activate", pid); err == nil {
		return nil
	}

	// Last resort: send SIGCONT to wake up the process (limited effect)
	return syscall.Kill(pid, syscall.SIGCONT)
}

// minimizeWindow minimizes a window by PID
func minimizeWindow(pid int) error {
	// Try compositor IPC first
	if err := tryCompositorIPC("minimize", pid); err == nil {
		return nil
	}

	// Fallback: try hyprctl
	if err := tryHyprctl("movetoworkspacesilent", pid); err == nil {
		return nil
	}

	// Fallback: try swaymsg
	if err := trySwaymsg("move scratchpad", pid); err == nil {
		return nil
	}

	// Fallback: try generic wlroots approach
	if err := tryWlrForeignToplevel("minimize", pid); err == nil {
		return nil
	}

	// Fallback: send SIGSTOP to pause the process
	return syscall.Kill(pid, syscall.SIGSTOP)
}

// restoreWindow restores a minimized window by PID
func restoreWindow(pid int) error {
	// Try compositor IPC first
	if err := tryCompositorIPC("restore", pid); err == nil {
		return nil
	}

	// Fallback: try hyprctl
	if err := tryHyprctl("focuswindow", pid); err == nil {
		return nil
	}

	// Fallback: try swaymsg
	if err := trySwaymsg("focus", pid); err == nil {
		return nil
	}

	// Fallback: try generic wlroots approach
	if err := tryWlrForeignToplevel("unminimize", pid); err == nil {
		return nil
	}

	// Fallback: send SIGCONT to resume the process
	return syscall.Kill(pid, syscall.SIGCONT)
}

// closeWindow closes a window by PID
func closeWindow(pid int) error {
	// Try compositor IPC first
	if err := tryCompositorIPC("close", pid); err == nil {
		return nil
	}

	// Fallback: try hyprctl
	if err := tryHyprctl("closewindow", pid); err == nil {
		return nil
	}

	// Fallback: try swaymsg
	if err := trySwaymsg("kill", pid); err == nil {
		return nil
	}

	// Fallback: try generic wlroots approach
	if err := tryWlrForeignToplevel("close", pid); err == nil {
		return nil
	}

	// Fallback: send SIGTERM for graceful termination
	if err := syscall.Kill(pid, syscall.SIGTERM); err != nil {
		return fmt.Errorf("failed to terminate process: %w", err)
	}
	return nil
}

// listWindows returns a list of all windows
func listWindows() ([]WindowInfo, error) {
	// Try compositor IPC first
	cmd := Command{Action: "list"}
	resp, err := sendCommand(cmd)
	if err == nil && resp.Success {
		if data, ok := resp.Data.([]any); ok {
			var windows []WindowInfo
			for _, item := range data {
				if m, ok := item.(map[string]any); ok {
					w := WindowInfo{
						PID:   int(m["pid"].(float64)),
						Title: m["title"].(string),
						AppID: m["app_id"].(string),
					}
					if focused, ok := m["focused"].(bool); ok {
						w.Focused = focused
					}
					if minimized, ok := m["minimized"].(bool); ok {
						w.Minimized = minimized
					}
					windows = append(windows, w)
				}
			}
			return windows, nil
		}
	}

	// Fallback: try hyprctl
	if windows, err := listWindowsHyprctl(); err == nil {
		return windows, nil
	}

	// Fallback: try swaymsg
	if windows, err := listWindowsSwaymsg(); err == nil {
		return windows, nil
	}

	// Fallback: list raven processes
	return listRavenProcesses()
}

// getActiveWindow returns the currently focused window
func getActiveWindow() (*WindowInfo, error) {
	// Try compositor IPC first
	cmd := Command{Action: "get-active"}
	resp, err := sendCommand(cmd)
	if err == nil && resp.Success {
		if m, ok := resp.Data.(map[string]any); ok {
			return &WindowInfo{
				PID:     int(m["pid"].(float64)),
				Title:   m["title"].(string),
				AppID:   m["app_id"].(string),
				Focused: true,
			}, nil
		}
	}

	// Fallback: try hyprctl
	if window, err := getActiveWindowHyprctl(); err == nil {
		return window, nil
	}

	// Fallback: try swaymsg
	if window, err := getActiveWindowSwaymsg(); err == nil {
		return window, nil
	}

	return nil, fmt.Errorf("no compositor available to query active window")
}

// Hyprland support

func tryHyprctl(action string, pid int) error {
	hyprctlPath, err := exec.LookPath("hyprctl")
	if err != nil {
		return fmt.Errorf("hyprctl not found")
	}

	var cmd *exec.Cmd
	switch action {
	case "focuswindow":
		cmd = exec.Command(hyprctlPath, "dispatch", "focuswindow", fmt.Sprintf("pid:%d", pid))
	case "closewindow":
		cmd = exec.Command(hyprctlPath, "dispatch", "closewindow", fmt.Sprintf("pid:%d", pid))
	case "movetoworkspacesilent":
		cmd = exec.Command(hyprctlPath, "dispatch", "movetoworkspacesilent", fmt.Sprintf("special,pid:%d", pid))
	default:
		return fmt.Errorf("unsupported action: %s", action)
	}

	return cmd.Run()
}

func listWindowsHyprctl() ([]WindowInfo, error) {
	hyprctlPath, err := exec.LookPath("hyprctl")
	if err != nil {
		return nil, fmt.Errorf("hyprctl not found")
	}

	output, err := exec.Command(hyprctlPath, "clients", "-j").Output()
	if err != nil {
		return nil, err
	}

	var clients []struct {
		PID       int    `json:"pid"`
		Title     string `json:"title"`
		Class     string `json:"class"`
		Focusable bool   `json:"focusable"`
	}
	if err := json.Unmarshal(output, &clients); err != nil {
		return nil, err
	}

	var windows []WindowInfo
	for _, c := range clients {
		windows = append(windows, WindowInfo{
			PID:   c.PID,
			Title: c.Title,
			AppID: c.Class,
		})
	}
	return windows, nil
}

func getActiveWindowHyprctl() (*WindowInfo, error) {
	hyprctlPath, err := exec.LookPath("hyprctl")
	if err != nil {
		return nil, fmt.Errorf("hyprctl not found")
	}

	output, err := exec.Command(hyprctlPath, "activewindow", "-j").Output()
	if err != nil {
		return nil, err
	}

	var window struct {
		PID   int    `json:"pid"`
		Title string `json:"title"`
		Class string `json:"class"`
	}
	if err := json.Unmarshal(output, &window); err != nil {
		return nil, err
	}

	return &WindowInfo{
		PID:     window.PID,
		Title:   window.Title,
		AppID:   window.Class,
		Focused: true,
	}, nil
}

// Sway support

func trySwaymsg(action string, pid int) error {
	swaymsgPath, err := exec.LookPath("swaymsg")
	if err != nil {
		return fmt.Errorf("swaymsg not found")
	}

	selector := fmt.Sprintf("[pid=%d]", pid)
	cmd := exec.Command(swaymsgPath, selector, action)
	return cmd.Run()
}

func listWindowsSwaymsg() ([]WindowInfo, error) {
	swaymsgPath, err := exec.LookPath("swaymsg")
	if err != nil {
		return nil, fmt.Errorf("swaymsg not found")
	}

	output, err := exec.Command(swaymsgPath, "-t", "get_tree").Output()
	if err != nil {
		return nil, err
	}

	var tree map[string]any
	if err := json.Unmarshal(output, &tree); err != nil {
		return nil, err
	}

	var windows []WindowInfo
	extractWindows(tree, &windows)
	return windows, nil
}

func extractWindows(node map[string]any, windows *[]WindowInfo) {
	if pid, ok := node["pid"].(float64); ok && pid > 0 {
		title := ""
		if t, ok := node["name"].(string); ok {
			title = t
		}
		appID := ""
		if a, ok := node["app_id"].(string); ok {
			appID = a
		}
		focused := false
		if f, ok := node["focused"].(bool); ok {
			focused = f
		}
		*windows = append(*windows, WindowInfo{
			PID:     int(pid),
			Title:   title,
			AppID:   appID,
			Focused: focused,
		})
	}

	if nodes, ok := node["nodes"].([]any); ok {
		for _, n := range nodes {
			if m, ok := n.(map[string]any); ok {
				extractWindows(m, windows)
			}
		}
	}
	if floating, ok := node["floating_nodes"].([]any); ok {
		for _, n := range floating {
			if m, ok := n.(map[string]any); ok {
				extractWindows(m, windows)
			}
		}
	}
}

func getActiveWindowSwaymsg() (*WindowInfo, error) {
	windows, err := listWindowsSwaymsg()
	if err != nil {
		return nil, err
	}

	for _, w := range windows {
		if w.Focused {
			return &w, nil
		}
	}
	return nil, fmt.Errorf("no focused window found")
}

// Generic wlroots support via wlr-foreign-toplevel-management
func tryWlrForeignToplevel(_ string, _ int) error {
	// This would require a dedicated wlr-foreign-toplevel client
	// For now, this is a placeholder for future implementation
	return fmt.Errorf("wlr-foreign-toplevel not implemented")
}

// listRavenProcesses lists Raven processes as a fallback
func listRavenProcesses() ([]WindowInfo, error) {
	output, err := exec.Command("ps", "-eo", "pid,comm", "--no-headers").Output()
	if err != nil {
		return nil, err
	}

	var windows []WindowInfo
	lines := strings.Split(string(output), "\n")
	for _, line := range lines {
		line = strings.TrimSpace(line)
		if line == "" {
			continue
		}

		fields := strings.Fields(line)
		if len(fields) < 2 {
			continue
		}

		pid, err := strconv.Atoi(fields[0])
		if err != nil {
			continue
		}

		procName := fields[1]

		// Only list graphical Raven applications
		if strings.HasPrefix(procName, "raven-") {
			// Skip non-graphical components
			if procName == "raven-shell" || procName == "raven-compositor" || procName == "raven-ctl" {
				continue
			}

			displayName := strings.TrimPrefix(procName, "raven-")
			displayName = capitalizeFirst(displayName)

			windows = append(windows, WindowInfo{
				PID:   pid,
				Title: displayName,
				AppID: procName,
			})
		}
	}

	return windows, nil
}
