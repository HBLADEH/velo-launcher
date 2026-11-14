// Package compat runs the offline Go oracle for the Rust data migration. All
// inputs are synthetic; it never selects or discovers a user's data directory.
package compat

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"reflect"

	"velo-launcher/internal/config"
	"velo-launcher/internal/history"
	"velo-launcher/internal/indexer"
	"velo-launcher/internal/model"
	"velo-launcher/internal/storage"
)

type IdentityCase struct {
	Name      string `json:"name"`
	Path      string `json:"path"`
	Arguments string `json:"arguments"`
	Expected  string `json:"expected"`
}

type ConfigCase struct {
	Name     string          `json:"name"`
	Input    string          `json:"input"`
	Valid    bool            `json:"valid"`
	Expected json.RawMessage `json:"expected,omitempty"`
}

// Library mirrors the private application's JSON wire shape. The shared
// fixture is also tested against Service.loadLibrary in internal/app.
type Library struct {
	Version      int             `json:"version"`
	Applications []model.AppItem `json:"applications"`
	Pinned       []model.AppItem `json:"pinned"`
}

type Bundle struct {
	Config  config.Config            `json:"config"`
	History map[string]history.Entry `json:"history"`
	Index   indexer.Cache            `json:"index"`
	Library Library                  `json:"library"`
}

type Fixture struct {
	SchemaVersion  int             `json:"schema_version"`
	FixedTime      string          `json:"fixed_time"`
	Identities     []IdentityCase  `json:"identities"`
	Configs        []ConfigCase    `json:"configs"`
	Bundle         json.RawMessage `json:"bundle"`
	ExpectedBundle json.RawMessage `json:"expected_bundle"`
	EditedBundle   json.RawMessage `json:"edited_bundle"`
	Legacy         []model.AppItem `json:"legacy"`
	ExpectedLegacy Library         `json:"expected_legacy"`
}

func Load(path string) (Fixture, error) {
	var f Fixture
	err := storage.Read(path, &f)
	if err == nil && f.SchemaVersion != 1 {
		err = fmt.Errorf("unsupported fixture version")
	}
	return f, err
}

func decodeBundle(data []byte) (Bundle, error) {
	b := Bundle{Config: config.Defaults()}
	err := json.Unmarshal(data, &b)
	return b, err
}

func edited(b Bundle, fixedTime string) (Bundle, error) {
	b.Config.Theme, b.Config.MaxResults = "light", 19
	entry := b.History["custom-preserved-id"]
	entry.Count++
	if err := entry.Last.UnmarshalText([]byte(fixedTime)); err != nil {
		return b, err
	}
	entry.Queries["测试"]++
	b.History["custom-preserved-id"] = entry
	b.Library.Applications = []model.AppItem{}
	kept := []model.AppItem{}
	for _, item := range b.Library.Pinned {
		if item.ID != "custom-preserved-id" {
			kept = append(kept, item)
		}
	}
	b.Library.Pinned = kept
	return b, nil
}

// Refresh records expectations from the existing Go types and validation rules.
// Run deliberately when adding reviewed cases, never during normal verification.
func Refresh(path string) error {
	f, err := Load(path)
	if err != nil {
		return err
	}
	for n, c := range f.Identities {
		f.Identities[n].Expected = model.Identity(c.Path, c.Arguments)
	}
	for n, c := range f.Configs {
		value := config.Defaults()
		err := json.Unmarshal([]byte(c.Input), &value)
		if err == nil {
			err = value.Validate()
		}
		f.Configs[n].Valid = err == nil
		f.Configs[n].Expected = nil
		if err == nil {
			f.Configs[n].Expected, err = json.Marshal(value)
			if err != nil {
				return err
			}
		}
	}
	b, err := decodeBundle(f.Bundle)
	if err != nil {
		return err
	}
	f.ExpectedBundle, err = json.Marshal(b)
	if err != nil {
		return err
	}
	b, err = edited(b, f.FixedTime)
	if err != nil {
		return err
	}
	f.EditedBundle, err = json.Marshal(b)
	if err != nil {
		return err
	}
	f.ExpectedLegacy = Library{Version: 1, Pinned: append([]model.AppItem(nil), f.Legacy...)}
	for n, item := range f.Legacy {
		if item.Source == "Quick Launch" {
			item.Source, item.Pinned = "Manual", false
			f.ExpectedLegacy.Applications = append(f.ExpectedLegacy.Applications, item)
			f.ExpectedLegacy.Pinned[n].Source = "Manual"
		}
	}
	return storage.Write(path, f)
}

func Check(f Fixture) error {
	for _, c := range f.Identities {
		if model.Identity(c.Path, c.Arguments) != c.Expected {
			return fmt.Errorf("identity case %s", c.Name)
		}
	}
	for _, c := range f.Configs {
		value := config.Defaults()
		err := json.Unmarshal([]byte(c.Input), &value)
		if err == nil {
			err = value.Validate()
		}
		if (err == nil) != c.Valid {
			return fmt.Errorf("config case %s validity", c.Name)
		}
		if c.Valid {
			var expected config.Config
			if err := json.Unmarshal(c.Expected, &expected); err != nil {
				return err
			}
			if !reflect.DeepEqual(value, expected) {
				return fmt.Errorf("config case %s value", c.Name)
			}
		}
	}
	return nil
}

func writeBundle(directory string, b Bundle) error {
	for name, value := range map[string]any{
		"config.json": b.Config, "history.json": b.History,
		"index.json": b.Index, "application-library.json": b.Library,
	} {
		if err := storage.Write(filepath.Join(directory, name), value); err != nil {
			return err
		}
	}
	return nil
}

func Emit(f Fixture, directory string) error {
	// A new explicit destination is required; the tool cannot overwrite a store.
	if err := os.Mkdir(directory, 0700); err != nil {
		return err
	}
	b, err := decodeBundle(f.Bundle)
	if err != nil {
		return err
	}
	return writeBundle(directory, b)
}

func ReadBundle(directory string) (Bundle, error) {
	b := Bundle{}
	var warning string
	var err error
	b.Config, warning, err = config.Load(filepath.Join(directory, "config.json"))
	if err != nil {
		return b, err
	}
	if warning != "" {
		return b, fmt.Errorf("fixture config needed recovery")
	}
	for name, value := range map[string]any{
		"history.json": &b.History,
		"index.json":   &b.Index, "application-library.json": &b.Library,
	} {
		if err := storage.Read(filepath.Join(directory, name), value); err != nil {
			return b, err
		}
	}
	return b, b.Config.Validate()
}

func Verify(f Fixture, directory string, changed bool) error {
	b, err := ReadBundle(directory)
	if err != nil {
		return err
	}
	expectedJSON := f.ExpectedBundle
	if changed {
		expectedJSON = f.EditedBundle
	}
	expected, err := decodeBundle(expectedJSON)
	if err != nil {
		return err
	}
	if !reflect.DeepEqual(b, expected) {
		return fmt.Errorf("data bundle differs from Go expectations")
	}
	return nil
}

func Rewrite(f Fixture, directory string) error {
	if err := Verify(f, directory, false); err != nil {
		return err
	}
	b, err := ReadBundle(directory)
	if err != nil {
		return err
	}
	b, err = edited(b, f.FixedTime)
	if err != nil {
		return err
	}
	return writeBundle(directory, b)
}
