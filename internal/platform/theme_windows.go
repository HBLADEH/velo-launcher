package platform

import "golang.org/x/sys/windows/registry"

// DarkAppearance resolves the saved preference before WebView2 paints its first frame.
func DarkAppearance(theme string) bool {
	if theme != "system" {
		return theme == "dark"
	}
	key, err := registry.OpenKey(registry.CURRENT_USER, `Software\Microsoft\Windows\CurrentVersion\Themes\Personalize`, registry.QUERY_VALUE)
	if err != nil {
		return false
	}
	defer key.Close()
	light, _, err := key.GetIntegerValue("AppsUseLightTheme")
	return err == nil && light == 0
}
