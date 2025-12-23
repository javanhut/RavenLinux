package navigation

import (
	"sync"
)

// History tracks browsing history for back/forward navigation
type History struct {
	paths    []string
	position int
	mu       sync.RWMutex
}

// NewHistory creates a new navigation history
func NewHistory() *History {
	return &History{
		paths:    make([]string, 0),
		position: -1,
	}
}

// Push adds a new path to history
func (h *History) Push(path string) {
	h.mu.Lock()
	defer h.mu.Unlock()

	// If we're not at the end, truncate forward history
	if h.position < len(h.paths)-1 {
		h.paths = h.paths[:h.position+1]
	}

	// Don't add duplicate consecutive entries
	if len(h.paths) > 0 && h.paths[len(h.paths)-1] == path {
		return
	}

	h.paths = append(h.paths, path)
	h.position = len(h.paths) - 1

	// Limit history size
	const maxHistory = 100
	if len(h.paths) > maxHistory {
		h.paths = h.paths[len(h.paths)-maxHistory:]
		h.position = len(h.paths) - 1
	}
}

// Back returns the previous path in history, or empty string if none
func (h *History) Back() string {
	h.mu.Lock()
	defer h.mu.Unlock()

	if h.position <= 0 {
		return ""
	}

	h.position--
	return h.paths[h.position]
}

// Forward returns the next path in history, or empty string if none
func (h *History) Forward() string {
	h.mu.Lock()
	defer h.mu.Unlock()

	if h.position >= len(h.paths)-1 {
		return ""
	}

	h.position++
	return h.paths[h.position]
}

// CanGoBack returns true if there's history to go back to
func (h *History) CanGoBack() bool {
	h.mu.RLock()
	defer h.mu.RUnlock()
	return h.position > 0
}

// CanGoForward returns true if there's history to go forward to
func (h *History) CanGoForward() bool {
	h.mu.RLock()
	defer h.mu.RUnlock()
	return h.position < len(h.paths)-1
}

// Current returns the current path
func (h *History) Current() string {
	h.mu.RLock()
	defer h.mu.RUnlock()

	if h.position < 0 || h.position >= len(h.paths) {
		return ""
	}
	return h.paths[h.position]
}

// Clear clears all history
func (h *History) Clear() {
	h.mu.Lock()
	defer h.mu.Unlock()

	h.paths = make([]string, 0)
	h.position = -1
}

// GetHistory returns a copy of the history paths
func (h *History) GetHistory() []string {
	h.mu.RLock()
	defer h.mu.RUnlock()

	result := make([]string, len(h.paths))
	copy(result, h.paths)
	return result
}

// GetPosition returns the current position in history
func (h *History) GetPosition() int {
	h.mu.RLock()
	defer h.mu.RUnlock()
	return h.position
}
