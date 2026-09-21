package update

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

const releasesFixture = `[
  {
    "tag_name": "v0.9.0",
    "name": "v0.9.0",
    "body": "自动更新",
    "draft": false,
    "prerelease": true,
    "published_at": "2026-09-22T02:00:00Z",
    "assets": [
      {"name": "velo-launcher.exe", "browser_download_url": "%s/files/velo-launcher.exe", "size": 12},
      {"name": "velo-launcher-amd64-installer.exe", "browser_download_url": "%s/files/installer.exe", "size": 9},
      {"name": "SHA256SUMS.txt", "browser_download_url": "%s/files/SHA256SUMS.txt", "size": 80}
    ]
  },
  {
    "tag_name": "v0.8.0-beta.2",
    "draft": false,
    "prerelease": true,
    "assets": [{"name": "velo-launcher.exe", "browser_download_url": "%s/files/old.exe", "size": 3}]
  },
  {
    "tag_name": "v9.9.9",
    "draft": true,
    "assets": [{"name": "velo-launcher.exe", "browser_download_url": "%s/files/draft.exe", "size": 1}]
  },
  {
    "tag_name": "nightly",
    "draft": false,
    "assets": []
  }
]`

func newReleaseServer(t *testing.T) *httptest.Server {
	t.Helper()
	var server *httptest.Server
	server = httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.Path {
		case "/repos/acme/velo/releases":
			fmt.Fprintf(w, releasesFixture, server.URL, server.URL, server.URL, server.URL, server.URL)
		case "/files/velo-launcher.exe":
			fmt.Fprint(w, "portable-app")
		case "/files/installer.exe":
			fmt.Fprint(w, "installer")
		case "/files/SHA256SUMS.txt":
			portable := sha256.Sum256([]byte("portable-app"))
			installer := sha256.Sum256([]byte("installer"))
			fmt.Fprintf(w, "# 发布校验值\n%s  velo-launcher.exe\n%s *velo-launcher-amd64-installer.exe\n",
				hex.EncodeToString(portable[:]), hex.EncodeToString(installer[:]))
		case "/limited":
			w.WriteHeader(http.StatusForbidden)
			fmt.Fprint(w, `{"message":"API rate limit exceeded for 1.2.3.4"}`)
		default:
			http.NotFound(w, r)
		}
	}))
	t.Cleanup(server.Close)
	return server
}

func testClient(server *httptest.Server) *Client {
	return &Client{Repository: "acme/velo", BaseURL: server.URL, HTTP: server.Client()}
}

func TestCheckFindsPrereleaseUpdateAndAssets(t *testing.T) {
	server := newReleaseServer(t)
	client := testClient(server)
	current, err := ParseVersion("0.8.0")
	if err != nil {
		t.Fatal(err)
	}
	info, err := client.Check(context.Background(), current)
	if err != nil {
		t.Fatal(err)
	}
	if !info.Available || info.Version != "0.9.0" || info.Notes != "自动更新" {
		t.Fatalf("检查结果 = %+v", info)
	}
	if info.Portable.Name != PortableName || !strings.HasSuffix(info.Installer.Name, InstallerSuffix) {
		t.Fatalf("资产选择错误: %+v", info)
	}
	if info.Checksum.Name != ChecksumName {
		t.Fatalf("缺少校验文件: %+v", info.Checksum)
	}
}

func TestCheckIgnoresSameOrNewerCurrentAndDrafts(t *testing.T) {
	server := newReleaseServer(t)
	client := testClient(server)
	// 0.9.0 与更新的版本都不算有更新；draft 与无法解析的 tag 必须被忽略。
	for _, tag := range []string{"0.9.0", "0.10.0", "1.0.0"} {
		current, err := ParseVersion(tag)
		if err != nil {
			t.Fatal(err)
		}
		info, err := client.Check(context.Background(), current)
		if err != nil {
			t.Fatal(err)
		}
		if info.Available {
			t.Fatalf("%s 不应看到更新：%+v", tag, info)
		}
		if info.Current != tag {
			t.Fatalf("当前版本未返回: %+v", info)
		}
	}
}

func TestCheckReportsServerErrors(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusForbidden)
		fmt.Fprint(w, `{"message":"API rate limit exceeded for 1.2.3.4"}`)
	}))
	defer server.Close()
	client := testClient(server)
	current, _ := ParseVersion("0.8.0")
	_, err := client.Check(context.Background(), current)
	if err == nil || !strings.Contains(err.Error(), "rate limit") {
		t.Fatalf("错误信息应包含服务器说明: %v", err)
	}
}

func TestCheckRejectsReleaseWithoutAssets(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		fmt.Fprint(w, `[{"tag_name":"v1.0.0","assets":[]}]`)
	}))
	defer server.Close()
	client := testClient(server)
	current, _ := ParseVersion("0.9.0")
	if _, err := client.Check(context.Background(), current); err == nil {
		t.Fatal("缺少资产的发布应返回错误而不是静默忽略")
	}
}

func TestDownloadVerifiesChecksumsAndCleansUp(t *testing.T) {
	server := newReleaseServer(t)
	client := testClient(server)
	dir := t.TempDir()
	dest := filepath.Join(dir, "nested", "velo-launcher.exe")
	reported := int64(0)
	sum, err := client.Download(context.Background(), server.URL+"/files/velo-launcher.exe", dest, func(done, _ int64) { reported = done })
	if err != nil {
		t.Fatal(err)
	}
	data, err := os.ReadFile(dest)
	if err != nil || string(data) != "portable-app" {
		t.Fatalf("下载内容错误: %q %v", data, err)
	}
	if reported != int64(len("portable-app")) {
		t.Fatalf("进度回调 = %d", reported)
	}
	sums, err := client.Checksums(context.Background(), server.URL+"/files/SHA256SUMS.txt")
	if err != nil {
		t.Fatal(err)
	}
	if sums[PortableName] != sum {
		t.Fatalf("校验值不匹配: %s != %s", sums[PortableName], sum)
	}
	if sums["velo-launcher-amd64-installer.exe"] == "" {
		t.Fatal("带 * 前缀的校验行未解析")
	}
	entries, err := os.ReadDir(filepath.Dir(dest))
	if err != nil {
		t.Fatal(err)
	}
	if len(entries) != 1 || entries[0].Name() != "velo-launcher.exe" {
		t.Fatalf("下载目录残留临时文件: %+v", entries)
	}
}

func TestDownloadFailsOnServerError(t *testing.T) {
	server := newReleaseServer(t)
	client := testClient(server)
	dest := filepath.Join(t.TempDir(), "velo-launcher.exe")
	if _, err := client.Download(context.Background(), server.URL+"/missing.exe", dest, nil); err == nil {
		t.Fatal("404 应返回错误")
	}
	if _, err := os.Stat(dest); !os.IsNotExist(err) {
		t.Fatal("失败时不应留下目标文件")
	}
}

func TestParseChecksumsIgnoresNoise(t *testing.T) {
	sums := ParseChecksums("garbage\n\n# comment\nABCDEF  ./tools/app.exe\n0123 *plain.exe\n")
	if len(sums) != 2 || sums["app.exe"] != "abcdef" || sums["plain.exe"] != "0123" {
		t.Fatalf("解析结果 = %+v", sums)
	}
}
