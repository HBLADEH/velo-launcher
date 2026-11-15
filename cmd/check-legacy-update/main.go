// Command check-legacy-update verifies the public release with the unchanged Go
// updater. It downloads to an explicit fixture directory and never installs.
package main

import (
	"context"
	"encoding/json"
	"flag"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"time"

	"velo-launcher/internal/update"
)

type feedTransport struct{ http.RoundTripper }

func (transport feedTransport) RoundTrip(request *http.Request) (*http.Response, error) {
	if request.URL.Host == "api.github.com" {
		return &http.Response{StatusCode: http.StatusTooManyRequests, Header: make(http.Header), Body: io.NopCloser(strings.NewReader("rate limit")), Request: request}, nil
	}
	return transport.RoundTripper.RoundTrip(request)
}

func run() error {
	currentText := flag.String("current", "0.9.11", "legacy version to check")
	expect := flag.String("expect", "0.10.0", "required update version")
	directory := flag.String("download-dir", "", "required isolated download directory")
	feed := flag.Bool("feed", false, "force the public-feed fallback")
	flag.Parse()
	if *directory == "" {
		return fmt.Errorf("provide an isolated -download-dir; this command never installs")
	}
	current, err := update.ParseVersion(*currentText)
	if err != nil {
		return err
	}
	client := update.NewClient()
	if *feed {
		client.HTTP.Transport = feedTransport{client.HTTP.Transport}
	}
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Minute)
	defer cancel()
	info, err := client.Check(ctx, current)
	if err != nil {
		return err
	}
	const alias = "velo-launcher-amd64-installer.exe"
	if !info.Available || info.Version != *expect || info.Installer.Name != alias || info.Portable.URL != "" || info.Checksum.URL == "" {
		return fmt.Errorf("release does not provide the expected legacy Software update")
	}
	sums, err := client.Checksums(ctx, info.Checksum.URL)
	if err != nil {
		return err
	}
	hash, err := client.Download(ctx, info.Installer.URL, filepath.Join(*directory, alias), nil)
	if err != nil {
		return err
	}
	if hash != sums[alias] || hash != sums["velo-rust-x64-installer.exe"] {
		return fmt.Errorf("legacy installer alias and Software checksum differ")
	}
	return json.NewEncoder(os.Stdout).Encode(struct {
		Current   string `json:"current"`
		Version   string `json:"version"`
		Installer string `json:"installer"`
		SHA256    string `json:"sha256"`
		Feed      bool   `json:"feed"`
	}{*currentText, info.Version, info.Installer.Name, hash, *feed})
}

func main() {
	if err := run(); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}
