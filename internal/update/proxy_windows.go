//go:build windows

package update

import "golang.org/x/sys/windows/registry"

// 读取当前用户在 Windows 设置中启用的手动代理，不修改系统配置。
// PAC/自动发现不在此处执行，使用这些方式时可通过 HTTPS_PROXY 指定代理。
func systemProxySettings() proxySettings {
	key, err := registry.OpenKey(registry.CURRENT_USER, `Software\Microsoft\Windows\CurrentVersion\Internet Settings`, registry.QUERY_VALUE)
	if err != nil {
		return proxySettings{}
	}
	defer key.Close()
	enabled, _, err := key.GetIntegerValue("ProxyEnable")
	if err != nil || enabled == 0 {
		return proxySettings{}
	}
	server, _, err := key.GetStringValue("ProxyServer")
	if err != nil {
		return proxySettings{}
	}
	bypass, _, _ := key.GetStringValue("ProxyOverride")
	return proxySettings{server: server, bypass: bypass}
}
