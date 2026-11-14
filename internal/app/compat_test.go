package app

import (
	"os"
	"path/filepath"
	"reflect"
	"testing"

	"velo-launcher/internal/compat"
	"velo-launcher/internal/storage"
)

func TestSharedRS08LegacyLibraryContract(t *testing.T) {
	f, err := compat.Load(filepath.Join("..", "..", "tests", "fixtures", "compat", "rs08.json"))
	if err != nil {
		t.Fatal(err)
	}
	directory := t.TempDir()
	legacy := filepath.Join(directory, "quick-launch.json")
	if err := storage.Write(legacy, f.Legacy); err != nil {
		t.Fatal(err)
	}
	original, err := os.ReadFile(legacy)
	if err != nil {
		t.Fatal(err)
	}
	s := &Service{dir: directory}
	if err := s.loadLibrary(); err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(s.custom, f.ExpectedLegacy.Applications) || !reflect.DeepEqual(s.quick, f.ExpectedLegacy.Pinned) {
		t.Fatal("legacy migration differs from shared fixture")
	}
	// An empty current library remains authoritative over the legacy backup.
	if err := storage.Write(filepath.Join(directory, "application-library.json"), applicationLibrary{Version: 1}); err != nil {
		t.Fatal(err)
	}
	if err := s.loadLibrary(); err != nil {
		t.Fatal(err)
	}
	if len(s.custom) != 0 || len(s.quick) != 0 {
		t.Fatal("legacy records resurrected")
	}
	if got, err := os.ReadFile(legacy); err != nil || string(got) != string(original) {
		t.Fatal("legacy backup changed")
	}
}
