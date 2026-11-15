package update

import (
	"context"
	"crypto/sha256"
	"encoding/json"
	"fmt"
	"net/http"
	"net/http/httptest"
	"path/filepath"
	"sort"
	"testing"
)

func TestRustMigrationReleaseRemainsInstallableByLegacyClients(t *testing.T) {
	const alias = "velo-launcher-amd64-installer.exe"
	const payload = "verified software installer"
	for _, feed := range []bool{false, true} {
		t.Run(fmt.Sprintf("feed=%t", feed), func(t *testing.T) {
			var server *httptest.Server
			server = httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
				names := []string{alias, "velo-rust-x64-installer.exe", "velo-rust-femtovg-x64-installer.exe", ChecksumName}
				switch r.URL.Path {
				case "/repos/acme/velo/releases":
					if feed {
						w.Header().Set("X-RateLimit-Remaining", "0")
						w.WriteHeader(http.StatusForbidden)
						fmt.Fprint(w, `{"message":"API rate limit exceeded"}`)
						return
					}
					var assets []Asset
					for _, name := range names {
						assets = append(assets, Asset{Name: name, URL: server.URL + "/acme/velo/releases/download/v0.10.0/" + name})
					}
					if err := json.NewEncoder(w).Encode([]Release{{TagName: "v0.10.0", Assets: assets}, {TagName: "0.1.0"}}); err != nil {
						t.Error(err)
					}
				case "/acme/velo/releases.atom":
					fmt.Fprint(w, `<feed xmlns="http://www.w3.org/2005/Atom"><entry><link rel="alternate" href="/acme/velo/releases/tag/v0.10.0"/></entry></feed>`)
				case "/acme/velo/releases/expanded_assets/v0.10.0":
					// GitHub's public list sorts names; API upload order is separate.
					sort.Strings(names)
					for _, name := range names {
						fmt.Fprintf(w, `<a href="/acme/velo/releases/download/v0.10.0/%s">%s</a>`, name, name)
					}
				case "/acme/velo/releases/download/v0.10.0/SHA256SUMS.txt":
					fmt.Fprintf(w, "%x  %s\n", sha256.Sum256([]byte(payload)), alias)
				case "/acme/velo/releases/download/v0.10.0/" + alias:
					fmt.Fprint(w, payload)
				default:
					http.NotFound(w, r)
				}
			}))
			defer server.Close()
			client := testClient(server)
			client.WebURL = server.URL
			for _, old := range []string{"0.9.0", "0.9.11"} {
				current, err := ParseVersion(old)
				if err != nil {
					t.Fatal(err)
				}
				info, err := client.Check(context.Background(), current)
				if err != nil {
					t.Fatal(err)
				}
				if !info.Available || info.Version != "0.10.0" || info.Installer.Name != alias || info.Portable.URL != "" {
					t.Fatalf("unsafe migration selection: %+v", info)
				}
				sums, err := client.Checksums(context.Background(), info.Checksum.URL)
				if err != nil {
					t.Fatal(err)
				}
				hash, err := client.Download(context.Background(), info.Installer.URL, filepath.Join(t.TempDir(), alias), nil)
				if err != nil || hash != sums[alias] {
					t.Fatalf("legacy verified download: hash=%s, error=%v", hash, err)
				}
			}
		})
	}
}
