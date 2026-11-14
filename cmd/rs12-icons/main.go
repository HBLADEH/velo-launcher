// Windows-only offline RS-12 cache measurements. Never launches an application.
package main

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"image"
	"image/draw"
	"image/png"
	"os"
	"path/filepath"
	"time"

	"velo-launcher/internal/icon"
	"velo-launcher/internal/model"
	"velo-launcher/internal/platform"
)

type row struct {
	Round        int      `json:"round"`
	Phase        string   `json:"phase"`
	Items        int      `json:"items"`
	ElapsedNS    int64    `json:"elapsed_nanoseconds"`
	Extractions  int      `json:"extractions"`
	DiskPNGBytes int64    `json:"disk_png_bytes"`
	DecodedBytes int      `json:"decoded_rgba_bytes"`
	PixelHashes  []string `json:"pixel_hashes"`
	IconURLs     []string `json:"icon_urls"`
}

func pixels(img image.Image) (string, int) {
	rgba := image.NewNRGBA(img.Bounds())
	draw.Draw(rgba, rgba.Bounds(), img, img.Bounds().Min, draw.Src)
	sum := sha256.Sum256(rgba.Pix)
	return hex.EncodeToString(sum[:]), len(rgba.Pix)
}

func run() error {
	if len(os.Args) == 3 && os.Args[1] == "prepare-frame-icons" {
		return prepareFrameIcons(os.Args[2])
	}
	if len(os.Args) != 2 {
		return fmt.Errorf("usage: rs12-icons <new evidence directory>")
	}
	directory := os.Args[1]
	if _, err := os.Stat(directory); os.IsNotExist(err) {
		if err := os.Mkdir(directory, 0700); err != nil {
			return err
		}
	} else if err != nil {
		return err
	} else {
		entries, err := os.ReadDir(directory)
		if err != nil {
			return err
		}
		if len(entries) != 0 {
			return fmt.Errorf("icon evidence directory must be new or empty")
		}
	}
	resolver, err := platform.NewResolver()
	if err != nil {
		return err
	}
	defer resolver.Close()
	items := []model.AppItem{}
	for _, name := range []string{"notepad.exe", "calc.exe", "cmd.exe", "control.exe"} {
		path := filepath.Join(os.Getenv("WINDIR"), "System32", name)
		if _, err := os.Stat(path); err != nil {
			return err
		}
		items = append(items, model.AppItem{ID: model.Identity(path, ""), Name: name, Path: path, ExecPath: path, Source: "System"})
	}
	rows := []row{}
	for round := 1; round <= 3; round++ {
		data := filepath.Join(directory, fmt.Sprintf("round-%d", round))
		current := append([]model.AppItem{}, items...)
		calls := 0
		hashes := []string{}
		decodedBytes := 0
		extracted := []image.Image{}
		extract := func(item model.AppItem) (image.Image, error) {
			calls++
			img, err := platform.ExtractIcon(item)
			if err == nil {
				extracted = append(extracted, img)
			}
			return img, err
		}
		start := time.Now()
		if err := icon.Populate(context.Background(), data, current, extract); err != nil {
			return err
		}
		coldNS := time.Since(start).Nanoseconds()
		for _, img := range extracted {
			hash, size := pixels(img)
			hashes = append(hashes, hash)
			decodedBytes += size
		}
		if calls != len(items) || len(hashes) != len(items) {
			return fmt.Errorf("native icon extraction incomplete")
		}
		var diskBytes int64
		urls := []string{}
		for _, item := range current {
			info, err := os.Stat(filepath.Join(icon.Directory(data), filepath.Base(item.IconURL)))
			if err != nil {
				return err
			}
			diskBytes += info.Size()
			urls = append(urls, item.IconURL)
		}
		rows = append(rows, row{round, "empty-disk-cache-extract-encode", len(items), coldNS, calls, diskBytes, decodedBytes, hashes, urls})
		start = time.Now()
		if err := icon.Populate(context.Background(), data, current, func(model.AppItem) (image.Image, error) {
			return nil, fmt.Errorf("cache hit unexpectedly requested extraction")
		}); err != nil {
			return err
		}
		hitNS := time.Since(start).Nanoseconds()
		decoded := []string{}
		decodedImages := []image.Image{}
		start = time.Now()
		for _, item := range current {
			file, err := os.Open(filepath.Join(icon.Directory(data), filepath.Base(item.IconURL)))
			if err != nil {
				return err
			}
			img, err := png.Decode(file)
			closeErr := file.Close()
			if err != nil {
				return err
			}
			if closeErr != nil {
				return closeErr
			}
			decodedImages = append(decodedImages, img)
		}
		decodeNS := time.Since(start).Nanoseconds()
		for n, img := range decodedImages {
			hash, _ := pixels(img)
			if hash != hashes[n] {
				return fmt.Errorf("PNG cache changed decoded pixels")
			}
			decoded = append(decoded, hash)
		}
		rows = append(rows, row{round, "existing-disk-cache-populate", len(items), hitNS, 0, diskBytes, 0, nil, urls})
		rows = append(rows, row{round, "existing-disk-cache-decode-check", len(items), decodeNS, 0, diskBytes, decodedBytes, decoded, urls})
	}
	file, err := os.OpenFile(filepath.Join(directory, "go-icons.json"), os.O_CREATE|os.O_EXCL|os.O_WRONLY, 0600)
	if err != nil {
		return err
	}
	defer file.Close()
	return json.NewEncoder(file).Encode(rows)
}

func main() {
	if err := run(); err != nil {
		fmt.Fprintln(os.Stderr, "RS-12 Go icon cache measurement failed:", err)
		os.Exit(1)
	}
}
