package platform

import (
	"golang.org/x/sys/windows"
	"golang.org/x/sys/windows/registry"
	"os"
	"velo-launcher/internal/model"
	"velo-launcher/internal/systemtools"
)

// SystemTools probes capabilities without launching tools or changing settings.
func SystemTools() []model.AppItem {
	root, err := windows.GetWindowsDirectory()
	if err != nil {
		return nil
	}
	_, _, build := windows.RtlGetNtVersionNumbers()
	env := systemtools.Environment{WindowsDir: root, Build: build, Exists: func(path string) bool {
		info, err := os.Stat(path)
		return err == nil && !info.IsDir()
	}}
	if key, err := registry.OpenKey(registry.CLASSES_ROOT, `ms-settings`, registry.QUERY_VALUE); err == nil {
		_, _, err := key.GetStringValue("URL Protocol")
		env.SettingsRegistered = err == nil
		key.Close()
	}
	for _, hive := range []registry.Key{registry.LOCAL_MACHINE, registry.CURRENT_USER} {
		key, err := registry.OpenKey(hive, `Software\Microsoft\Windows\CurrentVersion\Policies\Explorer`, registry.QUERY_VALUE)
		if err != nil {
			continue
		}
		if policy, _, err := key.GetStringValue("SettingsPageVisibility"); err == nil {
			env.PagePolicies = append(env.PagePolicies, policy)
		}
		key.Close()
	}
	return systemtools.Discover(env)
}
