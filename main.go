//go:build !bindings

package main

import (
	"embed"
	"fmt"
	"os"
	"path/filepath"
	"slices"
	"time"

	"github.com/wailsapp/wails/v2"
	"github.com/wailsapp/wails/v2/pkg/options"
	"github.com/wailsapp/wails/v2/pkg/options/assetserver"
	"github.com/wailsapp/wails/v2/pkg/options/windows"
	core "velo-launcher/internal/app"
	"velo-launcher/internal/icon"
	"velo-launcher/internal/logging"
	"velo-launcher/internal/platform"
)

//go:embed all:frontend/dist
var assets embed.FS

func main() {
	if err := run(); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}
func run() error {
	started := time.Now()
	acquired, release, err := platform.AcquireInstance()
	if err != nil {
		return err
	}
	if !acquired {
		return nil
	}
	defer release()
	dir, err := platform.DataDir()
	if err != nil {
		return err
	}
	logger, closeLog, err := logging.Open(dir)
	if err != nil {
		return err
	}
	defer closeLog()
	service, err := core.New(dir, logger)
	if err != nil {
		return err
	}
	app := NewApp(logger, service, slices.Contains(os.Args, "--background"), slices.Contains(os.Args, "--diagnostics"), started)
	err = wails.Run(&options.App{
		Title: "Velo", Width: 640, Height: 540, DisableResize: true,
		Frameless: true, AlwaysOnTop: true, HideWindowOnClose: true,
		BackgroundColour: &options.RGBA{R: 247, G: 248, B: 250, A: 255},
		AssetServer:      &assetserver.Options{Assets: assets, Handler: icon.Handler(dir)},
		OnStartup:        app.startup, OnDomReady: app.ready, OnShutdown: app.shutdown,
		Windows:            &windows.Options{WindowClassName: platform.WindowClass, WebviewUserDataPath: filepath.Join(dir, "webview"), WebviewGpuIsDisabled: true},
		SingleInstanceLock: &options.SingleInstanceLock{UniqueId: "6ab2e7d8-15d5-456b-a8eb-f924ac650291", OnSecondInstanceLaunch: func(_ options.SecondInstanceData) { app.Show() }},
		Bind:               []interface{}{app},
	})
	if err != nil {
		logger.Error("desktop runtime failed", "error", err)
	}
	return err
}
