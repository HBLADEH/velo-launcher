// Offline oracle: existing Go version rules and curated system capabilities.
package main

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"velo-launcher/internal/model"
	"velo-launcher/internal/systemtools"
	"velo-launcher/internal/update"
)

func main() {
	type environment struct {
		Build             uint32
		Settings          bool
		Policies, Missing []string
	}
	var input struct {
		Versions     [][2]string
		Environments []environment
	}
	data, err := os.ReadFile("tests/fixtures/compat/rs11.json")
	must(err)
	must(json.Unmarshal(data, &input))
	var output struct {
		Versions []any             `json:"versions"`
		Tools    [][]model.AppItem `json:"tools"`
	}
	for _, pair := range input.Versions {
		a, ae := update.ParseVersion(pair[0])
		b, be := update.ParseVersion(pair[1])
		if ae != nil || be != nil {
			output.Versions = append(output.Versions, nil)
		} else {
			output.Versions = append(output.Versions, a.Compare(b))
		}
	}
	for _, e := range input.Environments {
		exists := func(path string) bool {
			relative, _ := filepath.Rel(`D:\Windows`, path)
			for _, missing := range e.Missing {
				if missing == "*" || strings.EqualFold(filepath.ToSlash(relative), missing) {
					return false
				}
			}
			return true
		}
		output.Tools = append(output.Tools, systemtools.Discover(systemtools.Environment{WindowsDir: `D:\Windows`, Build: e.Build, SettingsRegistered: e.Settings, PagePolicies: e.Policies, Exists: exists}))
	}
	data, err = json.MarshalIndent(output, "", "  ")
	must(err)
	path := "tests/fixtures/compat/rs11.expected.json"
	if len(os.Args) > 1 && os.Args[1] == "generate" {
		must(os.WriteFile(path, append(data, '\n'), 0600))
		return
	}
	previous, err := os.ReadFile(path)
	must(err)
	var expected any
	var actual any
	must(json.Unmarshal(previous, &expected))
	must(json.Unmarshal(data, &actual))
	a, _ := json.Marshal(expected)
	b, _ := json.Marshal(actual)
	if string(a) != string(b) {
		panic("RS-11 Go oracle changed; regenerate and review the fixture")
	}
}
func must(err error) {
	if err != nil {
		panic(err)
	}
}
