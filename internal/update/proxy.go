package update

import (
	"net/http"
	"net/url"
	"path"
	"strings"

	"golang.org/x/net/http/httpproxy"
)

type proxySettings struct {
	server string
	bypass string
}

// 每次请求读取配置，用户开启或关闭系统代理后无需重启 Velo。
// 环境变量按协议优先；NO_PROXY 对系统代理同样生效。
func updateProxy(request *http.Request) (*url.URL, error) {
	return resolveProxy(request, httpproxy.FromEnvironment(), systemProxySettings)
}

func resolveProxy(request *http.Request, env *httpproxy.Config, system func() proxySettings) (*url.URL, error) {
	config := *env
	configured := config.HTTPProxy
	if request.URL.Scheme == "https" {
		configured = config.HTTPSProxy
	}
	if configured == "" {
		settings := system()
		if bypassProxy(request.URL, settings.bypass) {
			return nil, nil
		}
		config.HTTPProxy = proxyServer(settings.server, "http")
		config.HTTPSProxy = proxyServer(settings.server, "https")
	}
	return config.ProxyFunc()(request.URL)
}

// Windows 的统一代理（host:port）和按协议配置（http=...;https=...）。
// https= 表示目标请求协议，代理本身默认仍通过 HTTP CONNECT 通信。
func proxyServer(server, scheme string) string {
	for _, entry := range strings.Split(server, ";") {
		entry = strings.TrimSpace(entry)
		if entry == "" {
			continue
		}
		if protocol, address, ok := strings.Cut(entry, "="); ok {
			if !strings.EqualFold(strings.TrimSpace(protocol), scheme) {
				continue
			}
			entry = strings.TrimSpace(address)
		}
		if entry == "" {
			return ""
		}
		if !strings.Contains(entry, "://") {
			entry = "http://" + entry
		}
		return entry
	}
	return ""
}

func bypassProxy(target *url.URL, bypass string) bool {
	host := strings.ToLower(target.Hostname())
	for _, pattern := range strings.Split(bypass, ";") {
		pattern = strings.ToLower(strings.TrimSpace(pattern))
		if pattern == "" {
			continue
		}
		if pattern == "<local>" {
			if !strings.ContainsAny(host, ".:") {
				return true
			}
			continue
		}
		if matched, _ := path.Match(pattern, host); matched {
			return true
		}
		if matched, _ := path.Match(pattern, strings.ToLower(target.Host)); matched {
			return true
		}
	}
	return false
}
