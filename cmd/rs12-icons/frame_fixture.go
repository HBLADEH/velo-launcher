package main

import (
	"context"
	"encoding/json"
	"fmt"
	"image"
	"image/png"
	"os"
	"path/filepath"
	"strings"

	"velo-launcher/internal/icon"
	"velo-launcher/internal/model"
	"velo-launcher/internal/storage"
)

// Prepare only the first visible home/search rows with the real cache signature.
// The synthetic applications are never executed and no Shell extraction runs.
func prepareFrameIcons(directory string) error {
	if !filepath.IsAbs(directory) {
		return fmt.Errorf("frame seed must be absolute")
	}
	marker := filepath.Join(directory, "frame-icon-preparation.json")
	if _, err := os.Stat(marker); !os.IsNotExist(err) {
		return fmt.Errorf("preserve earlier frame icon preparation")
	}
	var library struct {
		Version      int             `json:"version"`
		Applications []model.AppItem `json:"applications"`
		Pinned       []model.AppItem `json:"pinned"`
	}
	if err := storage.Read(filepath.Join(directory, "application-library.json"), &library); err != nil {
		return err
	}
	if library.Version != 1 || (len(library.Applications) != 1000 && len(library.Applications) != 10000) {
		return fmt.Errorf("expected the controlled 1000/10000 library")
	}
	for _, item := range library.Applications {
		if !strings.HasPrefix(item.Name, "RS12 测试 App ") || item.Source != "Manual" || item.Arguments != "" {
			return fmt.Errorf("refuse non-synthetic application data")
		}
		if _, err := os.Stat(item.ExecPath); !os.IsNotExist(err) {
			return fmt.Errorf("synthetic execution target must not exist")
		}
	}
	file, err := os.Open(library.Applications[0].IconPath)
	if err != nil {
		return err
	}
	pixels, decodeErr := png.Decode(file)
	closeErr := file.Close()
	if decodeErr != nil {
		return decodeErr
	}
	if closeErr != nil {
		return closeErr
	}
	visible := library.Applications[:16]
	if err := icon.Populate(context.Background(), directory, visible, func(model.AppItem) (image.Image, error) { return pixels, nil }); err != nil {
		return err
	}
	byID := map[string]model.AppItem{}
	for _, item := range visible {
		byID[item.ID] = item
		cached, err := os.Open(filepath.Join(icon.Directory(directory), filepath.Base(item.IconURL)))
		if err != nil {
			return err
		}
		_, decodedErr := png.Decode(cached)
		closedErr := cached.Close()
		if decodedErr != nil {
			return decodedErr
		}
		if closedErr != nil {
			return closedErr
		}
	}
	for index, item := range library.Pinned {
		if prepared, ok := byID[item.ID]; ok {
			library.Pinned[index] = prepared
		}
	}
	if err := storage.Write(filepath.Join(directory, "application-library.json"), library); err != nil {
		return err
	}
	// This test cache is read before normal background discovery, just like a
	// real saved cache. Keep it aligned with the synthetic library and pins.
	if err := storage.Write(filepath.Join(directory, "index.json"), struct {
		Version int                    `json:"version"`
		Files   map[string]interface{} `json:"files"`
		Apps    []model.AppItem        `json:"apps"`
	}{1, map[string]interface{}{}, library.Applications}); err != nil {
		return err
	}
	output, err := os.OpenFile(marker, os.O_CREATE|os.O_EXCL|os.O_WRONLY, 0600)
	if err != nil {
		return err
	}
	defer output.Close()
	return json.NewEncoder(output).Encode(map[string]interface{}{
		"prepared_visible_items": len(visible), "application_count": len(library.Applications),
		"method": "Actual Go icon.Populate/signature with a fixed decoded checker; no Shell extraction; first 16 rows only",
		"icon_urls": func() []string {
			urls := []string{}
			for _, item := range visible {
				urls = append(urls, item.IconURL)
			}
			return urls
		}(),
	})
}
