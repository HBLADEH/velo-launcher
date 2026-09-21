package model

import (
	"crypto/sha256"
	"encoding/hex"
	"strings"
)

type AppItem struct {
	Pinned           bool     `json:"pinned"`
	ID               string   `json:"id"`
	Name             string   `json:"name"`
	Path             string   `json:"path"`
	ExecPath         string   `json:"exec_path"`
	Arguments        string   `json:"arguments"`
	WorkingDirectory string   `json:"working_directory"`
	IconPath         string   `json:"icon_path"`
	IconURL          string   `json:"icon_url"`
	Description      string   `json:"description"`
	Source           string   `json:"source"`
	Keywords         []string `json:"keywords"`
}

// Identity retains arguments so different profiles/commands remain separate.
func Identity(path, arguments string) string {
	sum := sha256.Sum256([]byte(strings.ToLower(path) + "\x00" + arguments))
	return hex.EncodeToString(sum[:16])
}
