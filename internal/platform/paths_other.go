//go:build !windows

package platform

import (
	"os"
	"path/filepath"
)

func DataDir() (string, error) {
	base, err := os.UserConfigDir()
	if err != nil {
		return "", err
	}
	return filepath.Join(base, "Velo"), nil
}
