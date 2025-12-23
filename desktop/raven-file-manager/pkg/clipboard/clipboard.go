package clipboard

import (
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"sync"

	"raven-file-manager/pkg/fileview"
)

// Operation represents the type of clipboard operation
type Operation int

const (
	OpNone Operation = iota
	OpCopy
	OpCut
)

// Manager handles file clipboard operations
type Manager struct {
	files     []string
	operation Operation
	mu        sync.RWMutex
}

// NewManager creates a new clipboard manager
func NewManager() *Manager {
	return &Manager{
		files:     make([]string, 0),
		operation: OpNone,
	}
}

// Copy stores files for copying
func (c *Manager) Copy(files []string) {
	c.mu.Lock()
	defer c.mu.Unlock()

	c.files = make([]string, len(files))
	copy(c.files, files)
	c.operation = OpCopy
}

// Cut stores files for cutting
func (c *Manager) Cut(files []string) {
	c.mu.Lock()
	defer c.mu.Unlock()

	c.files = make([]string, len(files))
	copy(c.files, files)
	c.operation = OpCut
}

// GetFiles returns the current clipboard files
func (c *Manager) GetFiles() []string {
	c.mu.RLock()
	defer c.mu.RUnlock()

	result := make([]string, len(c.files))
	copy(result, c.files)
	return result
}

// GetOperation returns the current clipboard operation
func (c *Manager) GetOperation() Operation {
	c.mu.RLock()
	defer c.mu.RUnlock()
	return c.operation
}

// Clear clears the clipboard
func (c *Manager) Clear() {
	c.mu.Lock()
	defer c.mu.Unlock()

	c.files = make([]string, 0)
	c.operation = OpNone
}

// HasFiles returns true if there are files in the clipboard
func (c *Manager) HasFiles() bool {
	c.mu.RLock()
	defer c.mu.RUnlock()
	return len(c.files) > 0
}

// Paste performs the paste operation to the target directory
func (c *Manager) Paste(targetDir string) error {
	c.mu.Lock()
	defer c.mu.Unlock()

	if len(c.files) == 0 {
		return nil
	}

	var lastErr error
	for _, src := range c.files {
		dst := filepath.Join(targetDir, filepath.Base(src))
		dst = ResolveConflict(dst)

		if c.operation == OpCut {
			err := MoveFile(src, dst)
			if err != nil {
				lastErr = err
			}
		} else {
			err := CopyFile(src, dst)
			if err != nil {
				lastErr = err
			}
		}
	}

	if c.operation == OpCut {
		c.files = make([]string, 0)
		c.operation = OpNone
	}

	return lastErr
}

// ResolveConflict generates a unique filename if target exists
func ResolveConflict(path string) string {
	if !fileview.FileExists(path) {
		return path
	}

	dir := filepath.Dir(path)
	base := filepath.Base(path)
	ext := filepath.Ext(base)
	name := base[:len(base)-len(ext)]

	for i := 1; i < 1000; i++ {
		newName := fmt.Sprintf("%s (%d)%s", name, i, ext)
		newPath := filepath.Join(dir, newName)
		if !fileview.FileExists(newPath) {
			return newPath
		}
	}

	return path
}

// CopyFile copies a file or directory
func CopyFile(src, dst string) error {
	srcInfo, err := os.Stat(src)
	if err != nil {
		return err
	}

	if srcInfo.IsDir() {
		return copyDir(src, dst)
	}

	return copyRegularFile(src, dst, srcInfo.Mode())
}

func copyRegularFile(src, dst string, mode os.FileMode) error {
	srcFile, err := os.Open(src)
	if err != nil {
		return err
	}
	defer srcFile.Close()

	dstFile, err := os.OpenFile(dst, os.O_CREATE|os.O_WRONLY|os.O_TRUNC, mode)
	if err != nil {
		return err
	}
	defer dstFile.Close()

	_, err = io.Copy(dstFile, srcFile)
	return err
}

func copyDir(src, dst string) error {
	srcInfo, err := os.Stat(src)
	if err != nil {
		return err
	}

	err = os.MkdirAll(dst, srcInfo.Mode())
	if err != nil {
		return err
	}

	entries, err := os.ReadDir(src)
	if err != nil {
		return err
	}

	for _, entry := range entries {
		srcPath := filepath.Join(src, entry.Name())
		dstPath := filepath.Join(dst, entry.Name())

		if entry.IsDir() {
			err = copyDir(srcPath, dstPath)
		} else {
			info, err := entry.Info()
			if err != nil {
				continue
			}
			err = copyRegularFile(srcPath, dstPath, info.Mode())
		}

		if err != nil {
			return err
		}
	}

	return nil
}

// MoveFile moves a file or directory
func MoveFile(src, dst string) error {
	err := os.Rename(src, dst)
	if err == nil {
		return nil
	}

	err = CopyFile(src, dst)
	if err != nil {
		return err
	}

	return os.RemoveAll(src)
}

// TrashFiles moves files to trash
func TrashFiles(files []fileview.FileEntry) error {
	var lastErr error
	for _, f := range files {
		err := exec.Command("gio", "trash", f.Path).Run()
		if err != nil {
			trashDir := filepath.Join(os.Getenv("HOME"), ".local/share/Trash/files")
			os.MkdirAll(trashDir, 0755)
			trashPath := filepath.Join(trashDir, f.Name)
			trashPath = ResolveConflict(trashPath)
			err = MoveFile(f.Path, trashPath)
		}
		if err != nil {
			lastErr = err
		}
	}
	return lastErr
}

// DeleteFiles permanently deletes files
func DeleteFiles(files []fileview.FileEntry) error {
	var lastErr error
	for _, f := range files {
		err := os.RemoveAll(f.Path)
		if err != nil {
			lastErr = err
		}
	}
	return lastErr
}
