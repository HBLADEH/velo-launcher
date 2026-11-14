// RS-10's oracle calls the unchanged production discovery/filter/action rules.
package main

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"reflect"

	"velo-launcher/internal/config"
	"velo-launcher/internal/icon"
	"velo-launcher/internal/indexer"
	"velo-launcher/internal/model"
	"velo-launcher/internal/platform"
)

type nameCase struct {
	Name  string `json:"name"`
	Noise bool   `json:"noise"`
}
type itemCase struct {
	Name         string                 `json:"name"`
	Items        []model.AppItem        `json:"items"`
	Deduplicated []model.AppItem        `json:"deduplicated"`
	Filtered     []model.AppItem        `json:"filtered"`
	All          []model.AppItem        `json:"all"`
	Actions      []platform.ItemActions `json:"actions"`
}
type fixture struct {
	SchemaVersion int        `json:"schema_version"`
	Names         []nameCase `json:"names"`
	Cases         []itemCase `json:"cases"`
}

func main() {
	if err := run(); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}
func clone(items []model.AppItem) []model.AppItem {
	data, _ := json.Marshal(items)
	var copy []model.AppItem
	_ = json.Unmarshal(data, &copy)
	return copy
}
func run() error {
	if len(os.Args) == 3 && os.Args[1] == "native" {
		return native(os.Args[2])
	}
	if len(os.Args) != 2 || (os.Args[1] != "check" && os.Args[1] != "refresh") {
		return fmt.Errorf("usage: rs10-compat check|refresh")
	}
	path := "tests/fixtures/compat/rs10.json"
	data, err := os.ReadFile(path)
	if err != nil {
		return err
	}
	var f fixture
	if err := json.Unmarshal(data, &f); err != nil {
		return err
	}
	if f.SchemaVersion != 1 {
		return fmt.Errorf("unsupported fixture")
	}
	refresh := os.Args[1] == "refresh"
	for n := range f.Names {
		c := &f.Names[n]
		got := indexer.IsNoise(c.Name)
		if refresh {
			c.Noise = got
		} else if got != c.Noise {
			return fmt.Errorf("noise mismatch: %d", n)
		}
	}
	for n := range f.Cases {
		c := &f.Cases[n]
		dedup, filtered, all := indexer.Deduplicate(clone(c.Items)), indexer.Visible(clone(c.Items), true), indexer.Visible(clone(c.Items), false)
		actions := []platform.ItemActions{}
		for _, item := range c.Items {
			actions = append(actions, platform.ActionsFor(item))
		}
		if refresh {
			c.Deduplicated, c.Filtered, c.All, c.Actions = dedup, filtered, all, actions
		} else if !reflect.DeepEqual(dedup, c.Deduplicated) || !reflect.DeepEqual(filtered, c.Filtered) || !reflect.DeepEqual(all, c.All) || !reflect.DeepEqual(actions, c.Actions) {
			return fmt.Errorf("discovery mismatch: %s", c.Name)
		}
	}
	if refresh {
		data, err := json.MarshalIndent(f, "", "  ")
		if err != nil {
			return err
		}
		return os.WriteFile(path, append(data, '\n'), 0600)
	}
	fmt.Printf("RS-10 Go oracle passed: %d names, %d discovery/action groups\n", len(f.Names), len(f.Cases))
	return nil
}

// Local-only evidence uses a disposable synthetic directory. No real names or
// paths from this output are copied into the public baseline report.
func native(directory string) error {
	r, err := platform.NewResolver()
	if err != nil {
		return err
	}
	defer r.Close()
	cache, _, err := indexer.Scan(context.Background(), []indexer.Root{{Path: directory, Source: "Custom"}}, indexer.Cache{}, r)
	if err != nil {
		return err
	}
	apps, err := platform.WindowsApps(context.Background())
	if err != nil {
		return err
	}
	if err := icon.Populate(context.Background(), directory, cache.Apps, platform.ExtractIcon); err != nil {
		return err
	}
	c := config.Defaults()
	c.ScanProgramFiles = false
	result := struct {
		Cache indexer.Cache   `json:"cache"`
		Apps  []model.AppItem `json:"packaged"`
		Roots []indexer.Root  `json:"roots"`
	}{cache, apps, platform.Roots(c)}
	data, err := json.Marshal(result)
	if err != nil {
		return err
	}
	return os.WriteFile(filepath.Join(directory, "go-native.json"), data, 0600)
}
