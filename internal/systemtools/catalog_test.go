package systemtools

import (
	"path/filepath"
	"strings"
	"testing"

	"velo-launcher/internal/model"
)

func fixture() Environment {
	return Environment{WindowsDir: filepath.Join("C:", "Windows Test"), Build: 22631, SettingsRegistered: true, Exists: func(string) bool { return true }}
}

func byID(items []model.AppItem) map[string]model.AppItem {
	result := map[string]model.AppItem{}
	for _, item := range items {
		result[item.ID] = item
	}
	return result
}

func TestCapabilitiesFilterMissingToolsAndConsoleDependencies(t *testing.T) {
	env := fixture()
	env.SettingsRegistered = false
	env.Exists = func(path string) bool {
		return strings.EqualFold(filepath.Base(path), "mmc.exe") || strings.EqualFold(filepath.Base(path), "compmgmt.msc")
	}
	items := Discover(env)
	if len(items) != 1 || items[0].ID != "system:computer" {
		t.Fatalf("missing components were advertised: %+v", items)
	}
	want := `"` + filepath.Join(env.WindowsDir, "System32", "compmgmt.msc") + `"`
	if items[0].Arguments != want {
		t.Fatalf("console path not quoted: %q", items[0].Arguments)
	}
	env.Exists = func(string) bool { return false }
	if len(Discover(env)) != 0 {
		t.Fatal("missing launch host was advertised")
	}
}

func TestSettingsVersionProtocolAndPolicy(t *testing.T) {
	env := fixture()
	for _, tt := range []struct {
		build uint32
		id    string
		want  bool
	}{
		{9600, "system:display", false}, {10240, "system:display", true},
		{15063, "system:startup", false}, {17134, "system:startup", true},
		{10240, "system:sound", false}, {17134, "system:sound", true},
		{19045, "system:shared", true}, {22631, "system:shared", false},
	} {
		env.Build = tt.build
		_, got := byID(Discover(env))[tt.id]
		if got != tt.want {
			t.Errorf("build %d, %s: present=%v", tt.build, tt.id, got)
		}
	}
	env.Build = 22631
	env.PagePolicies = []string{"showonly:display;windowsupdate;activation", "hide:activation"}
	items := byID(Discover(env))
	for _, id := range []string{"system:recovery", "system:activation"} {
		if _, ok := items[id]; ok {
			t.Errorf("policy-hidden entry %s", id)
		}
	}
	if _, ok := items["system:updates"]; !ok {
		t.Fatal("allowed page hidden")
	}
	env.SettingsRegistered = false
	for _, item := range Discover(env) {
		if strings.HasPrefix(item.Path, "ms-settings:") {
			t.Fatal("unregistered settings protocol advertised")
		}
	}
}

func TestCatalogueIdentityAndSettingsLaunchValidation(t *testing.T) {
	items := Discover(fixture())
	seen := map[string]bool{}
	for _, item := range items {
		if seen[item.ID] {
			t.Fatalf("duplicate ID %s", item.ID)
		}
		seen[item.ID] = true
		if item.Name == "" || item.Description == "" || len(item.Keywords) == 0 {
			t.Fatalf("incomplete metadata: %+v", item)
		}
		if strings.HasPrefix(item.Path, "ms-settings:") && !IsSettingsItem(item) {
			t.Fatalf("valid URI rejected: %+v", item)
		}
	}
	valid := byID(items)["system:updates"]
	for _, mutate := range []func(*model.AppItem){
		func(a *model.AppItem) { a.Path += "?query=arbitrary"; a.ExecPath = a.Path },
		func(a *model.AppItem) { a.Arguments = "arbitrary" },
		func(a *model.AppItem) { a.ID = "system:unknown" },
		func(a *model.AppItem) { a.Source = "Manual" },
		func(a *model.AppItem) { a.ExecPath = "cmd.exe" },
	} {
		item := valid
		mutate(&item)
		if IsSettingsItem(item) {
			t.Fatalf("unexpected settings command accepted: %+v", item)
		}
	}
}
