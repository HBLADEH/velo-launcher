package icon

import (
	"context"
	"fmt"
	"image"
	"image/png"
	"net/http"
	"os"
	"path/filepath"
	"regexp"
	"strings"
	"sync"
	"velo-launcher/internal/model"
)

type Extractor func(string) (image.Image, error)

type encoderPool struct{ sync.Pool }

func (p *encoderPool) Get() *png.EncoderBuffer {
	if value := p.Pool.Get(); value != nil {
		return value.(*png.EncoderBuffer)
	}
	return new(png.EncoderBuffer)
}
func (p *encoderPool) Put(buffer *png.EncoderBuffer) { p.Pool.Put(buffer) }

var buffers encoderPool

func Directory(dataDir string) string { return filepath.Join(dataDir, "cache", "icons") }
func signature(item model.AppItem) string {
	parts := []string{item.Path, item.ExecPath, strings.Trim(strings.Split(item.IconPath, ",")[0], "\"")}
	var stamp strings.Builder
	for _, path := range parts {
		stamp.WriteString(path)
		if info, err := os.Stat(path); err == nil {
			fmt.Fprintf(&stamp, "|%d|%d", info.Size(), info.ModTime().UnixNano())
		}
	}
	return model.Identity(stamp.String(), "")
}

// Populate is background-only. Search and render never extract icons.
func Populate(ctx context.Context, dataDir string, items []model.AppItem, extract Extractor) error {
	dir := Directory(dataDir)
	if err := os.MkdirAll(dir, 0700); err != nil {
		return err
	}
	used := make(map[string]bool, len(items))
	for n := range items {
		if err := ctx.Err(); err != nil {
			return err
		}
		name := signature(items[n]) + ".png"
		used[name] = true
		path := filepath.Join(dir, name)
		if _, err := os.Stat(path); err != nil {
			img, err := extract(items[n].Path)
			if err != nil {
				continue
			}
			f, err := os.CreateTemp(dir, ".icon-*.tmp")
			if err != nil {
				return err
			}
			encoder := png.Encoder{CompressionLevel: png.BestSpeed, BufferPool: &buffers}
			err = encoder.Encode(f, img)
			closeErr := f.Close()
			if err == nil {
				err = closeErr
			}
			if err == nil {
				err = os.Rename(f.Name(), path)
			}
			os.Remove(f.Name())
			if err != nil {
				return err
			}
		}
		items[n].IconURL = "/icons/" + name
	}
	// Only generated cache PNGs are eligible for cleanup.
	files, err := os.ReadDir(dir)
	if err != nil {
		return err
	}
	for _, f := range files {
		if cacheName.MatchString(f.Name()) && !used[f.Name()] {
			if err := os.Remove(filepath.Join(dir, f.Name())); err != nil {
				return err
			}
		}
	}
	return nil
}

var cacheName = regexp.MustCompile(`^[a-f0-9]{32}\.png$`)

func Handler(dataDir string) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		name := strings.TrimPrefix(r.URL.Path, "/icons/")
		if r.Method != "GET" && r.Method != "HEAD" {
			w.WriteHeader(http.StatusMethodNotAllowed)
			return
		}
		if !strings.HasPrefix(r.URL.Path, "/icons/") || !cacheName.MatchString(name) {
			http.NotFound(w, r)
			return
		}
		w.Header().Set("Cache-Control", "public, max-age=31536000, immutable")
		http.ServeFile(w, r, filepath.Join(Directory(dataDir), name))
	})
}
