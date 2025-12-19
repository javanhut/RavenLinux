package shell

import (
	"io"
	"os"
	"os/exec"
	"os/user"
	"strings"
	"sync"
	"syscall"

	"github.com/creack/pty"
)

// getDistroName reads the distribution name from /etc/os-release
func getDistroName() string {
	data, err := os.ReadFile("/etc/os-release")
	if err != nil {
		return "linux"
	}
	for _, line := range strings.Split(string(data), "\n") {
		if strings.HasPrefix(line, "ID=") {
			id := strings.TrimPrefix(line, "ID=")
			id = strings.Trim(id, "\"")
			return id
		}
	}
	return "linux"
}

// PtySession manages a pseudo-terminal connection to a shell
type PtySession struct {
	cmd      *exec.Cmd
	pty      *os.File
	mu       sync.Mutex
	exited   bool
	exitedMu sync.Mutex
}

// NewPtySession creates a new PTY session with a login shell
func NewPtySession(cols, rows uint16) (*PtySession, error) {
	shell := findShell()

	// Get user info from system, not environment
	currentUser, err := user.Current()
	if err != nil {
		return nil, err
	}

	cmd := exec.Command(shell, "-l")

	// Create new session - critical for independence from parent terminal
	cmd.SysProcAttr = &syscall.SysProcAttr{
		Setsid: true,
	}

	// Get distro name for prompt
	distro := getDistroName()

	// Custom PS1: path on line above, then [user@distro] >
	// \[\e[0;36m\] = cyan color, \[\e[0m\] = reset, \w = working directory
	ps1 := `\[\e[0;36m\]\w\[\e[0m\]\n[\u@` + distro + `] > `

	// Clean environment - don't inherit from parent terminal
	cmd.Env = []string{
		"PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
		"TERM=xterm-256color",
		"COLORTERM=truecolor",
		"RAVEN_TERMINAL=1",
		"HOME=" + currentUser.HomeDir,
		"USER=" + currentUser.Username,
		"SHELL=" + shell,
		"LANG=en_US.UTF-8",
		"LC_ALL=en_US.UTF-8",
		"PS1=" + ps1,
	}

	// Start in home directory
	cmd.Dir = currentUser.HomeDir

	ptmx, err := pty.StartWithSize(cmd, &pty.Winsize{
		Cols: cols,
		Rows: rows,
	})
	if err != nil {
		return nil, err
	}

	session := &PtySession{
		cmd:    cmd,
		pty:    ptmx,
		exited: false,
	}

	// Monitor for process exit
	go func() {
		cmd.Wait()
		session.exitedMu.Lock()
		session.exited = true
		session.exitedMu.Unlock()
	}()

	return session, nil
}

// findShell finds the default shell from system user database
func findShell() string {
	// Get shell from /etc/passwd, not environment variable
	currentUser, err := user.Current()
	if err == nil {
		shell := getUserShell(currentUser.Username)
		if shell != "" {
			if _, err := os.Stat(shell); err == nil {
				return shell
			}
		}
	}

	// Fallback to common shells
	shells := []string{"/bin/bash", "/usr/bin/bash", "/bin/zsh", "/usr/bin/zsh", "/bin/sh"}
	for _, shell := range shells {
		if _, err := os.Stat(shell); err == nil {
			return shell
		}
	}
	return "/bin/sh"
}

// getUserShell reads the user's shell from /etc/passwd
func getUserShell(username string) string {
	data, err := os.ReadFile("/etc/passwd")
	if err != nil {
		return ""
	}
	for _, line := range strings.Split(string(data), "\n") {
		fields := strings.Split(line, ":")
		if len(fields) >= 7 && fields[0] == username {
			return fields[6]
		}
	}
	return ""
}

// Read reads from the PTY
func (p *PtySession) Read(buf []byte) (int, error) {
	return p.pty.Read(buf)
}

// Write writes to the PTY
func (p *PtySession) Write(data []byte) (int, error) {
	p.mu.Lock()
	defer p.mu.Unlock()
	return p.pty.Write(data)
}

// Resize resizes the PTY
func (p *PtySession) Resize(cols, rows uint16) error {
	p.mu.Lock()
	defer p.mu.Unlock()
	return pty.Setsize(p.pty, &pty.Winsize{
		Cols: cols,
		Rows: rows,
	})
}

// HasExited returns true if the shell process has exited
func (p *PtySession) HasExited() bool {
	p.exitedMu.Lock()
	defer p.exitedMu.Unlock()
	return p.exited
}

// Close closes the PTY session
func (p *PtySession) Close() error {
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.cmd.Process != nil {
		p.cmd.Process.Kill()
	}
	return p.pty.Close()
}

// Reader returns an io.Reader for the PTY
func (p *PtySession) Reader() io.Reader {
	return p.pty
}

// Writer returns an io.Writer for the PTY
func (p *PtySession) Writer() io.Writer {
	return p.pty
}
