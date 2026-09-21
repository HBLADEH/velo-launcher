package main

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"log/slog"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	core "velo-launcher/internal/app"
	"velo-launcher/internal/config"
	"velo-launcher/internal/indexer"
	"velo-launcher/internal/model"
	"velo-launcher/internal/platform"
	"velo-launcher/internal/storage"
	"velo-launcher/internal/update"
	"velo-launcher/internal/version"
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

func TestImportModeKeepsWindowAvailableForDrag(t *testing.T) {
	w := &testWindow{visible: true, active: false}
	a := &App{window: w, key: &platform.Hotkey{}}
	a.SetImportMode(true)
	a.Blur()
	if !w.visible {
		t.Fatal("import mode hid before drag")
	}
	a.Hide()
	if a.importMode {
		t.Fatal("hidden window retained import mode")
	}
	w.visible = true
	a.Blur()
	if w.visible {
		t.Fatal("normal blur behavior not restored")
	}
}

func TestStatusReportsBuildVersion(t *testing.T) {
	dir := t.TempDir()
	logger := slog.New(slog.NewTextHandler(io.Discard, nil))
	service, err := core.New(dir, logger)
	if err != nil {
		t.Fatal(err)
	}
	app := NewApp(logger, service, false, false, time.Now())
	if got := app.GetStatus().Version; got != version.Number {
		t.Fatalf("状态版本 = %q，想要 %q", got, version.Number)
	}
	if _, err := update.ParseVersion(version.Number); err != nil {
		t.Fatalf("构建版本无法解析，自动更新会失效: %v", err)
	}
}

// 发布流程容易漏改版本号：直接读元数据文件，确保三处版本一致。
func TestVersionMetadataMatchesBuild(t *testing.T) {
	build, err := update.ParseVersion(version.Number)
	if err != nil {
		t.Fatal(err)
	}
	raw, err := os.ReadFile("wails.json")
	if err != nil {
		t.Fatal(err)
	}
	var project struct {
		Info struct {
			ProductVersion string `json:"productVersion"`
		} `json:"info"`
	}
	if err := json.Unmarshal(raw, &project); err != nil {
		t.Fatal(err)
	}
	product, err := update.ParseVersion(project.Info.ProductVersion)
	if err != nil || product.Major != build.Major || product.Minor != build.Minor || product.Patch != build.Patch {
		t.Fatalf("wails.json 的 productVersion %q 与构建版本 %q 不一致", project.Info.ProductVersion, version.Number)
	}
	raw, err = os.ReadFile(filepath.Join("frontend", "package.json"))
	if err != nil {
		t.Fatal(err)
	}
	var frontend struct {
		Version string `json:"version"`
	}
	if err := json.Unmarshal(raw, &frontend); err != nil {
		t.Fatal(err)
	}
	if frontend.Version != version.Number {
		t.Fatalf("frontend/package.json 的版本 %q 与构建版本 %q 不一致", frontend.Version, version.Number)
	}
}

// 未签名程序必须依赖发布页的校验值，缺失时宁可拒绝安装。
func TestInstallUpdateRefusesReleaseWithoutChecksums(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		fmt.Fprintf(w, `[{"tag_name":"v9.9.9","assets":[{"name":"velo-launcher.exe","browser_download_url":"%s/velo-launcher.exe","size":3}]}]`, "http://"+r.Host)
	}))
	defer server.Close()
	dir := t.TempDir()
	logger := slog.New(slog.NewTextHandler(io.Discard, nil))
	service, err := core.New(dir, logger)
	if err != nil {
		t.Fatal(err)
	}
	app := NewApp(logger, service, false, false, time.Now())
	app.updates = &update.Client{Repository: "acme/velo", BaseURL: server.URL, HTTP: server.Client()}
	err = app.InstallUpdate()
	if err == nil || !strings.Contains(err.Error(), "SHA-256") {
		t.Fatalf("缺少校验文件时应拒绝安装: %v", err)
	}
	if entries, err := os.ReadDir(filepath.Join(dir, "updates")); err == nil && len(entries) > 0 {
		t.Fatalf("被拒绝的安装不应留下下载文件: %+v", entries)
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
