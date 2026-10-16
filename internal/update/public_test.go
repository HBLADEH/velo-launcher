package update

import (
	"context"
	"crypto/sha256"
	"fmt"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"sync/atomic"
	"testing"
	"time"
)

const publicFeedFixture = `<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <entry>
    <title>Velo v9.9.9</title>
    <updated>2026-10-17T01:00:00Z</updated>
    <link rel="alternate" href="/acme/velo/releases/tag/v0.9.9"/>
  </entry>
  <entry>
    <title>Velo preview</title>
    <updated>2026-10-16T01:00:00Z</updated>
    <link rel="alternate" href="/acme/velo/releases/tag/v0.10.0-beta.2"/>
    <content type="html">&lt;p&gt;修复 &amp;amp; 验证&lt;/p&gt;&lt;p&gt;保留&lt;strong&gt;配置&lt;/strong&gt;&lt;/p&gt;</content>
  </entry>
  <entry><link rel="alternate" href="/acme/velo/releases/tag/nightly"/></entry>
  <entry><link rel="alternate" href="/other/repo/releases/tag/v9.9.9"/></entry>
  <entry><link rel="alternate" href="https://untrusted.invalid/acme/velo/releases/tag/v9.9.9"/></entry>
</feed>`

func TestRateLimitFallbackChecksAndDownloadsVerifiedPrerelease(t *testing.T) {
	const payload = "verified portable app"
	sum := fmt.Sprintf("%x", sha256.Sum256([]byte(payload)))
	var apiCalls, feedCalls, assetCalls atomic.Int32
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/repos/acme/velo/releases" {
			apiCalls.Add(1)
			if r.Header.Get("Authorization") != "Bearer test-api-token" {
				t.Error("API authentication missing")
			}
			w.Header().Set("X-RateLimit-Remaining", "0")
			w.Header().Set("X-RateLimit-Reset", fmt.Sprint(time.Now().Add(time.Hour).Unix()))
			w.WriteHeader(http.StatusForbidden)
			fmt.Fprint(w, `{"message":"API rate limit exceeded for 134.195.101.188"}`)
			return
		}
		if r.Header.Get("Authorization") != "" {
			t.Error("API token leaked to a public endpoint")
		}
		switch r.URL.Path {
		case "/acme/velo/releases.atom":
			feedCalls.Add(1)
			fmt.Fprint(w, publicFeedFixture)
		case "/acme/velo/releases/expanded_assets/v0.10.0-beta.2":
			assetCalls.Add(1)
			fmt.Fprint(w, `<ul>
<li><a href="/acme/velo/releases/download/v0.10.0-beta.2/velo-launcher.exe"><span>Portable</span></a></li>
<li><a href="/acme/velo/releases/download/v0.10.0-beta.2/velo-launcher-amd64-installer.exe">Installer</a></li>
<li><a href="/acme/velo/releases/download/v0.10.0-beta.2/SHA256SUMS.txt">Checksums</a></li>
<li><a href="https://untrusted.invalid/acme/velo/releases/download/v0.10.0-beta.2/velo-launcher.exe">External</a></li>
<li><a href="//untrusted.invalid/acme/velo/releases/download/v0.10.0-beta.2/SHA256SUMS.txt">External checksum</a></li>
<li><a href="/other/repo/releases/download/v0.10.0-beta.2/velo-launcher.exe">Other repo</a></li>
<li><a href="/acme/velo/releases/download/v0.9.9/velo-launcher.exe">Old tag</a></li>
<li><a href="/acme/velo/archive/refs/tags/v0.10.0-beta.2.zip">Source code</a></li>
<li><a href="/acme/velo/releases/download/v0.10.0-beta.2/nested%2Fvelo-launcher.exe">Nested path</a></li>
</ul>`)
		case "/acme/velo/releases/download/v0.10.0-beta.2/SHA256SUMS.txt":
			fmt.Fprintf(w, "%s  velo-launcher.exe\n", sum)
		case "/acme/velo/releases/download/v0.10.0-beta.2/velo-launcher.exe":
			fmt.Fprint(w, payload)
		default:
			t.Errorf("unexpected request: %s", r.URL)
			http.NotFound(w, r)
		}
	}))
	defer server.Close()
	client := testClient(server)
	client.WebURL, client.Token = server.URL, "test-api-token"
	current, _ := ParseVersion("0.9.8")
	info, err := client.Check(context.Background(), current)
	if err != nil {
		t.Fatal(err)
	}
	if !info.Available || info.Version != "0.10.0-beta.2" || info.Notes != "修复 & 验证\n保留配置" || info.PublishedAt != "2026-10-16T01:00:00Z" {
		t.Fatalf("fallback info = %+v", info)
	}
	if info.Portable.Name != PortableName || info.Installer.Name != "velo-launcher-amd64-installer.exe" || info.Checksum.Name != ChecksumName {
		t.Fatalf("fallback assets = %+v", info)
	}
	for _, asset := range []Asset{info.Portable, info.Installer, info.Checksum} {
		if asset.URL != server.URL+"/acme/velo/releases/download/v0.10.0-beta.2/"+asset.Name {
			t.Fatalf("unexpected asset URL: %+v", asset)
		}
	}
	// InstallUpdate 会再次检查：限流窗口内仍能更新，且不重复请求 API。
	if _, err := client.Check(context.Background(), current); err != nil {
		t.Fatal(err)
	}
	latest, _ := ParseVersion(info.Version)
	upToDate, err := client.Check(context.Background(), latest)
	if err != nil || upToDate.Available || upToDate.Current != info.Version {
		t.Fatalf("same version = %+v, %v", upToDate, err)
	}
	if apiCalls.Load() != 1 || feedCalls.Load() != 3 || assetCalls.Load() != 2 {
		t.Fatalf("API/feed/assets requests = %d/%d/%d", apiCalls.Load(), feedCalls.Load(), assetCalls.Load())
	}
	// 下载地址使用不同主机，确保普通下载请求同样不携带 API Token。
	client.BaseURL = "https://api.invalid"
	sums, err := client.Checksums(context.Background(), info.Checksum.URL)
	if err != nil {
		t.Fatal(err)
	}
	dest := filepath.Join(t.TempDir(), PortableName)
	downloadedSum, err := client.Download(context.Background(), info.Portable.URL, dest, nil)
	if err != nil || downloadedSum != sums[PortableName] || downloadedSum != sum {
		t.Fatalf("download checksum = %q, error = %v", downloadedSum, err)
	}
	data, err := os.ReadFile(dest)
	if err != nil || string(data) != payload {
		t.Fatalf("download = %q, %v", data, err)
	}
}

func TestRateLimitSignalsAndCooldownRecovery(t *testing.T) {
	for _, test := range []struct {
		name    string
		status  int
		headers http.Header
		message string
	}{
		{"403 message", http.StatusForbidden, nil, "API rate limit exceeded"},
		{"403 header", http.StatusForbidden, http.Header{"X-Ratelimit-Remaining": {"0"}, "X-Ratelimit-Reset": {fmt.Sprint(time.Now().Add(time.Hour).Unix())}}, "Forbidden"},
		{"secondary limit", http.StatusForbidden, http.Header{"Retry-After": {"120"}}, "You have exceeded a secondary rate limit"},
		{"429", http.StatusTooManyRequests, http.Header{"Retry-After": {time.Now().Add(time.Minute).UTC().Format(http.TimeFormat)}}, "Too many requests"},
	} {
		t.Run(test.name, func(t *testing.T) {
			var apiCalls, feedCalls atomic.Int32
			server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
				switch r.URL.Path {
				case "/repos/acme/velo/releases":
					if apiCalls.Add(1) > 1 {
						fmt.Fprint(w, `[]`)
						return
					}
					for name, values := range test.headers {
						for _, value := range values {
							w.Header().Add(name, value)
						}
					}
					w.WriteHeader(test.status)
					fmt.Fprintf(w, `{"message":%q}`, test.message)
				case "/acme/velo/releases.atom":
					feedCalls.Add(1)
					fmt.Fprint(w, `<feed xmlns="http://www.w3.org/2005/Atom"/>`)
				default:
					t.Errorf("unexpected request: %s", r.URL)
				}
			}))
			defer server.Close()
			client := testClient(server)
			client.WebURL = server.URL
			current, _ := ParseVersion("0.9.8")
			for range 2 {
				if _, err := client.Check(context.Background(), current); err != nil {
					t.Fatal(err)
				}
			}
			if apiCalls.Load() != 1 || feedCalls.Load() != 2 {
				t.Fatalf("requests during cooldown = %d/%d", apiCalls.Load(), feedCalls.Load())
			}
			client.mu.Lock()
			client.apiRetryAt = time.Now().Add(-time.Second)
			client.mu.Unlock()
			if _, err := client.Check(context.Background(), current); err != nil {
				t.Fatal(err)
			}
			if apiCalls.Load() != 2 || feedCalls.Load() != 2 {
				t.Fatalf("API did not recover after cooldown = %d/%d", apiCalls.Load(), feedCalls.Load())
			}
		})
	}
}

func TestRateLimitFallbackFailuresExplainRecovery(t *testing.T) {
	for _, test := range []struct {
		name, feed, assets string
		status             int
		want               string
	}{
		{"unavailable feed", "", "", http.StatusServiceUnavailable, "503"},
		{"malformed feed", "<feed", "", http.StatusOK, "无法解析"},
		{"HTML instead of feed", "<html></html>", "", http.StatusOK, "无法解析"},
		{"foreign version", `<feed xmlns="http://www.w3.org/2005/Atom"><entry><link rel="alternate" href="https://untrusted.invalid/acme/velo/releases/tag/v9.9.9"/></entry></feed>`, "", http.StatusOK, "没有可识别"},
		{"unavailable assets", publicFeedFixture, "", http.StatusNotFound, "404"},
	} {
		t.Run(test.name, func(t *testing.T) {
			server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
				switch r.URL.Path {
				case "/repos/acme/velo/releases":
					w.WriteHeader(http.StatusTooManyRequests)
				case "/acme/velo/releases.atom":
					if test.feed == "" {
						w.WriteHeader(test.status)
					}
					fmt.Fprint(w, test.feed)
				default:
					w.WriteHeader(test.status)
					fmt.Fprint(w, test.assets)
				}
			}))
			defer server.Close()
			client := testClient(server)
			client.WebURL = server.URL
			current, _ := ParseVersion("0.9.8")
			_, err := client.Check(context.Background(), current)
			for _, want := range []string{"限流", "切换代理节点", "手动下载", server.URL + "/acme/velo/releases", test.want} {
				if err == nil || !strings.Contains(err.Error(), want) {
					t.Fatalf("error = %v, want %q", err, want)
				}
			}
		})
	}
}

func TestPublicFallbackDoesNotInventMissingAssets(t *testing.T) {
	for _, assets := range []string{
		`<a href="/acme/velo/archive/refs/tags/v0.10.0-beta.2.zip">Source</a>`,
		`<a href="/acme/velo/releases/download/v0.10.0-beta.2/velo-launcher.exe">Portable</a>`,
	} {
		t.Run(assets, func(t *testing.T) {
			server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
				switch r.URL.Path {
				case "/repos/acme/velo/releases":
					w.WriteHeader(http.StatusTooManyRequests)
				case "/acme/velo/releases.atom":
					fmt.Fprint(w, publicFeedFixture)
				default:
					fmt.Fprint(w, assets)
				}
			}))
			defer server.Close()
			client := testClient(server)
			client.WebURL = server.URL
			current, _ := ParseVersion("0.9.8")
			info, err := client.Check(context.Background(), current)
			if strings.Contains(assets, "archive/") {
				if err == nil || !strings.Contains(err.Error(), "没有可下载") {
					t.Fatalf("source archive accepted as update: %+v, %v", info, err)
				}
			} else if err != nil || !info.Available || info.Checksum.URL != "" {
				t.Fatalf("missing checksum must remain unavailable for install: %+v, %v", info, err)
			}
		})
	}
}
