// Package logging provides structured local application logs.
package logging

import (
	"log/slog"
	"os"
	"path/filepath"
)

// Open keeps the current session and one previous session, without a
// background goroutine. Call closeLog after application shutdown.
func Open(dataDir string) (*slog.Logger, func(), error) {
	dir := filepath.Join(dataDir, "logs")
	if err := os.MkdirAll(dir, 0700); err != nil {
		return nil, nil, err
	}
	path := filepath.Join(dir, "velo.log")
	if _, err := os.Stat(path); err == nil {
		previous := filepath.Join(dir, "velo.previous.log")
		if err := os.Remove(previous); err != nil && !os.IsNotExist(err) {
			return nil, nil, err
		}
		if err := os.Rename(path, previous); err != nil {
			return nil, nil, err
		}
	} else if !os.IsNotExist(err) {
		return nil, nil, err
	}
	f, err := os.OpenFile(path, os.O_CREATE|os.O_WRONLY|os.O_TRUNC, 0600)
	if err != nil {
		return nil, nil, err
	}
	// GUI builds may inherit a closed stderr pipe. Log directly to the file so
	// a detached shell cannot suppress subsequent diagnostic entries.
	logger := slog.New(slog.NewJSONHandler(f, nil))
	return logger, func() { _ = f.Close() }, nil
}
