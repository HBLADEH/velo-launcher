package platform

import (
	"fmt"
	"path/filepath"
	"testing"
	"time"

	"golang.org/x/sys/windows/registry"
)

func TestStartupRegistryRoundTrip(t *testing.T) {
	// Exercise the real registry API without changing the user's Run entries.
	path := fmt.Sprintf(`Software\Velo-Test-%d`, time.Now().UnixNano())
	key, _, err := registry.CreateKey(registry.CURRENT_USER, path, registry.SET_VALUE|registry.QUERY_VALUE)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		key.Close()
		if err := registry.DeleteKey(registry.CURRENT_USER, path); err != nil {
			t.Error(err)
		}
	})
	exe := filepath.Join(t.TempDir(), "directory with spaces", "Velo.exe")
	if err := setStartupValue(key, true, exe); err != nil {
		t.Fatal(err)
	}
	value, _, err := key.GetStringValue("Velo")
	if err != nil || value != `"`+exe+`" --background` {
		t.Fatalf("unexpected Run command: %q %v", value, err)
	}
	if err := setStartupValue(key, false, ""); err != nil {
		t.Fatal(err)
	}
	if _, _, err := key.GetStringValue("Velo"); err != registry.ErrNotExist {
		t.Fatal("startup entry remains", err)
	}
	if err := setStartupValue(key, false, ""); err != nil {
		t.Fatal("disable is not idempotent", err)
	}
}
