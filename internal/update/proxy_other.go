//go:build !windows

package update

func systemProxySettings() proxySettings { return proxySettings{} }
