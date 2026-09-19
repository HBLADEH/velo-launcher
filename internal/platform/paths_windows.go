package platform

import (
	"fmt"
	"os"
	"path/filepath"
)

// DataDir returns the non-roaming application data directory.
func DataDir() (string, error) {
	base := os.Getenv("LOCALAPPDATA")
	if base == "" {
		return "", fmt.Errorf("LOCALAPPDATA is not set")
	}
	return filepath.Join(base, "Velo"), nil
}
