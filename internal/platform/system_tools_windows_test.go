package platform

import (
	"os"
	"strings"
	"testing"
	"velo-launcher/internal/systemtools"
)

func TestSystemToolsLocalCapabilities(t *testing.T) {
	items := SystemTools()
	if len(items) == 0 {
		t.Fatal("no Windows tools detected")
	}
	for _, item := range items {
		if strings.HasPrefix(item.Path, "ms-settings:") {
			if !systemtools.IsSettingsItem(item) {
				t.Fatalf("invalid settings item: %+v", item)
			}
			continue
		}
		if _, err := os.Stat(item.Path); err != nil {
			t.Errorf("unavailable target %s: %v", item.Path, err)
		}
		if strings.HasSuffix(strings.ToLower(item.Path), "mmc.exe") {
			if _, err := os.Stat(strings.Trim(item.Arguments, `"`)); err != nil {
				t.Errorf("unavailable console: %v", err)
			}
		}
	}
	t.Logf("discovered %d available system entries without launching them", len(items))
}
