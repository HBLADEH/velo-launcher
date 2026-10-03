package update

import (
	"context"
	"crypto/sha256"
	"errors"
	"fmt"
	"net/http"
	"net/http/httptest"
	"net/url"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"testing"

	"golang.org/x/net/http/httpproxy"
)

func TestResolveProxy(t *testing.T) {
	settings := proxySettings{server: "127.0.0.1:7897", bypass: "localhost;127.*;192.168.*;*.internal;<local>"}
	for _, test := range []struct {
		name, target string
		env          httpproxy.Config
		settings     proxySettings
		want         string
	}{
		{"system HTTPS", "https://github.com/releases", httpproxy.Config{}, settings, "http://127.0.0.1:7897"},
		{"system HTTP", "http://api.github.com", httpproxy.Config{}, settings, "http://127.0.0.1:7897"},
		{"redirect host", "https://release-assets.githubusercontent.com/file", httpproxy.Config{}, settings, "http://127.0.0.1:7897"},
		{"environment wins", "https://github.com", httpproxy.Config{HTTPSProxy: "http://env-proxy:8080"}, proxySettings{server: "system:80", bypass: "*"}, "http://env-proxy:8080"},
		{"environment per scheme", "https://github.com", httpproxy.Config{HTTPProxy: "http://env-proxy:8080"}, settings, "http://127.0.0.1:7897"},
		{"no proxy environment", "https://github.com", httpproxy.Config{NoProxy: "github.com"}, settings, ""},
		{"wildcard bypass", "https://files.internal", httpproxy.Config{}, settings, ""},
		{"IP wildcard bypass", "http://192.168.10.1", httpproxy.Config{}, settings, ""},
		{"local bypass", "http://intranet", httpproxy.Config{}, settings, ""},
		{"loopback bypass", "http://127.0.0.1:8000", httpproxy.Config{}, proxySettings{server: "proxy:80"}, ""},
		{"proxy disabled", "https://github.com", httpproxy.Config{}, proxySettings{}, ""},
		{"protocol HTTPS", "https://github.com", httpproxy.Config{}, proxySettings{server: "http=http-proxy:80;https=https-proxy:81"}, "http://https-proxy:81"},
		{"protocol HTTP", "http://github.com", httpproxy.Config{}, proxySettings{server: "http=http-proxy:80;https=https-proxy:81"}, "http://http-proxy:80"},
		{"missing protocol", "https://github.com", httpproxy.Config{}, proxySettings{server: "http=http-proxy:80"}, ""},
		{"explicit scheme", "https://github.com", httpproxy.Config{}, proxySettings{server: "https=socks5://127.0.0.1:1080"}, "socks5://127.0.0.1:1080"},
	} {
		t.Run(test.name, func(t *testing.T) {
			request, err := http.NewRequest(http.MethodGet, test.target, nil)
			if err != nil {
				t.Fatal(err)
			}
			proxy, err := resolveProxy(request, &test.env, func() proxySettings { return test.settings })
			if err != nil {
				t.Fatal(err)
			}
			got := ""
			if proxy != nil {
				got = proxy.String()
			}
			if got != test.want {
				t.Fatalf("proxy = %q, want %q", got, test.want)
			}
		})
	}
}

func TestProxyReadsChangedSettings(t *testing.T) {
	request, _ := http.NewRequest(http.MethodGet, "https://github.com", nil)
	settings := proxySettings{server: "127.0.0.1:7897"}
	read := func() proxySettings { return settings }
	proxy, err := resolveProxy(request, &httpproxy.Config{}, read)
	if err != nil || proxy == nil {
		t.Fatalf("enabled proxy = %v, %v", proxy, err)
	}
	settings = proxySettings{}
	proxy, err = resolveProxy(request, &httpproxy.Config{}, read)
	if err != nil || proxy != nil {
		t.Fatalf("disabled proxy = %v, %v", proxy, err)
	}
}

func TestUpdateRequestsAndRedirectsUseProxy(t *testing.T) {
	payload := "verified update"
	expected := fmt.Sprintf("%x", sha256.Sum256([]byte(payload)))
	var hosts []string
	var hostsMu sync.Mutex
	proxy := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if !r.URL.IsAbs() {
			t.Error("proxy request must have an absolute URL")
		}
		hostsMu.Lock()
		hosts = append(hosts, r.URL.Host)
		hostsMu.Unlock()
		switch r.URL.Path {
		case "/repos/acme/velo/releases":
			fmt.Fprint(w, `[{"tag_name":"v1.0.0","assets":[{"name":"velo-launcher.exe","browser_download_url":"http://downloads.invalid/app.exe"},{"name":"SHA256SUMS.txt","browser_download_url":"http://downloads.invalid/sums"}]}]`)
		case "/sums":
			http.Redirect(w, r, "http://assets.invalid/checksums", http.StatusFound)
		case "/checksums":
			fmt.Fprintf(w, "%s  velo-launcher.exe\n", expected)
		case "/app.exe":
			http.Redirect(w, r, "http://assets.invalid/binary", http.StatusFound)
		case "/binary":
			fmt.Fprint(w, payload)
		default:
			http.NotFound(w, r)
		}
	}))
	defer proxy.Close()
	client := NewClient()
	client.Repository, client.BaseURL = "acme/velo", "http://api.invalid"
	client.HTTP.Transport.(*http.Transport).Proxy = func(request *http.Request) (*url.URL, error) {
		return resolveProxy(request, &httpproxy.Config{}, func() proxySettings { return proxySettings{server: proxy.URL} })
	}
	defer client.HTTP.CloseIdleConnections()
	current, _ := ParseVersion("0.9.0")
	info, err := client.Check(context.Background(), current)
	if err != nil {
		t.Fatal(err)
	}
	sums, err := client.Checksums(context.Background(), info.Checksum.URL)
	if err != nil {
		t.Fatal(err)
	}
	dest := filepath.Join(t.TempDir(), PortableName)
	sum, err := client.Download(context.Background(), info.Portable.URL, dest, nil)
	if err != nil || sum != sums[PortableName] || sum != expected {
		t.Fatalf("download checksum = %q, error = %v", sum, err)
	}
	data, err := os.ReadFile(dest)
	if err != nil || string(data) != payload {
		t.Fatalf("download = %q, %v", data, err)
	}
	hostsMu.Lock()
	defer hostsMu.Unlock()
	if got := strings.Join(hosts, ","); got != "api.invalid,downloads.invalid,assets.invalid,downloads.invalid,assets.invalid" {
		t.Fatalf("proxy requests = %s", got)
	}
}

type failingTransport struct{ err error }

func (f failingTransport) RoundTrip(*http.Request) (*http.Response, error) { return nil, f.err }

func TestConnectionErrorsExplainRecoveryAndPreserveCause(t *testing.T) {
	cause := errors.New("connection timed out")
	client := &Client{Repository: "acme/velo", HTTP: &http.Client{Transport: failingTransport{cause}}}
	current, _ := ParseVersion("0.9.0")
	for _, operation := range []func() error{
		func() error { _, err := client.Check(context.Background(), current); return err },
		func() error { _, err := client.Checksums(context.Background(), "https://github.com/sums"); return err },
		func() error {
			_, err := client.Download(context.Background(), "https://github.com/app", filepath.Join(t.TempDir(), PortableName), nil)
			return err
		},
	} {
		err := operation()
		if !errors.Is(err, cause) || !strings.Contains(err.Error(), "代理设置") || !strings.Contains(err.Error(), "手动下载") {
			t.Fatalf("connection error = %v", err)
		}
	}
}
