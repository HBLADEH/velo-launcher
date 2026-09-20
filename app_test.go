package main

import (
	"context"
	"io"
	"log/slog"
	"os"
	"path/filepath"
	"testing"
	"time"

	core "velo-launcher/internal/app"
	"velo-launcher/internal/config"
	"velo-launcher/internal/indexer"
	"velo-launcher/internal/model"
	"velo-launcher/internal/platform"
	"velo-launcher/internal/storage"
)

type testWindow struct {
	visible, active bool
	hides           int
}

func (w *testWindow) Visible() bool   { return w.visible }
func (w *testWindow) Active() bool    { return w.active }
func (w *testWindow) Show()           { w.visible = true }
func (w *testWindow) Hide()           { w.visible = false; w.hides++ }
func (w *testWindow) Resize(int, int) {}

func TestDefaultWindowCloseAndFocusBehavior(t *testing.T) {
	w := &testWindow{visible: true, active: true}
	a := &App{window: w, key: &platform.Hotkey{}}
	a.Blur()
	if !w.visible {
		t.Fatal("owned popup/input focus hid active launcher")
	}
	w.active = false
	a.Blur()
	if w.visible || w.hides != 1 {
		t.Fatal("losing foreground did not hide")
	}
	a.Blur()
	if w.hides != 1 {
		t.Fatal("duplicate blur performed a second transition")
	}
	w.visible = true
	if !a.beforeClose(context.Background()) || w.visible {
		t.Fatal("Alt+F4 did not keep launcher resident and hidden")
	}
	a.exitRequested = true
	if a.beforeClose(context.Background()) {
		t.Fatal("explicit Quit was vetoed")
	}
}
func TestWindowRemainsReachableWithoutHotkey(t *testing.T) {
	w := &testWindow{visible: true}
	a := &App{window: w}
	a.Blur()
	if !w.visible {
		t.Fatal("unavailable hotkey stranded hidden window")
	}
	if a.beforeClose(context.Background()) {
		t.Fatal("closing without a registered key must exit")
	}
}

func TestFailedLaunchDoesNotRecordHistory(t *testing.T) {
	dir := t.TempDir()
	if err := storage.Write(filepath.Join(dir, "index.json"), indexer.Cache{Version: 1, Apps: []model.AppItem{{ID: "missing", Name: "Missing", Path: filepath.Join(dir, "missing.exe")}}}); err != nil {
		t.Fatal(err)
	}
	logger := slog.New(slog.NewTextHandler(io.Discard, nil))
	service, err := core.New(dir, logger)
	if err != nil {
		t.Fatal(err)
	}
	app := NewApp(logger, service, false, false, time.Now())
	if err := app.Launch("missing", "m"); err == nil {
		t.Fatal("missing executable accepted")
	}
	if _, err := os.Stat(filepath.Join(dir, "history.json")); !os.IsNotExist(err) {
		t.Fatal("failed launch recorded")
	}
}
func TestSettingsPersistenceFailureRestoresHotkey(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "config.json")
	c := config.Defaults()
	c.Hotkey = "Ctrl+Alt+Shift+F20"
	if err := storage.Write(path, c); err != nil {
		t.Fatal(err)
	}
	logger := slog.New(slog.NewTextHandler(io.Discard, nil))
	service, err := core.New(dir, logger)
	if err != nil {
		t.Fatal(err)
	}
	app := NewApp(logger, service, false, false, time.Now())
	app.key, err = platform.RegisterHotkey(c.Hotkey, func() {})
	if err != nil {
		t.Fatal(err)
	}
	defer app.key.Close()
	if err := os.Remove(path); err != nil {
		t.Fatal(err)
	}
	if err := os.Mkdir(path, 0700); err != nil {
		t.Fatal(err)
	}
	c.Hotkey = "Ctrl+Alt+Shift+F19"
	if err := app.SaveSettings(c); err == nil {
		t.Fatal("write failure not reported")
	}
	probe, err := platform.RegisterHotkey(c.Hotkey, func() {})
	if err != nil {
		t.Fatal("new hotkey not released", err)
	}
	probe.Close()
	probe, err = platform.RegisterHotkey("Ctrl+Alt+Shift+F20", func() {})
	if err == nil {
		probe.Close()
		t.Fatal("old hotkey not restored")
	}
	if app.GetSettings().Hotkey != "Ctrl+Alt+Shift+F20" {
		t.Fatal("failed config applied")
	}
}
