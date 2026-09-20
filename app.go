package main

import (
	"context"
	"fmt"
	"log/slog"
	"sync"
	"time"

	wruntime "github.com/wailsapp/wails/v2/pkg/runtime"
	core "velo-launcher/internal/app"
	"velo-launcher/internal/config"
	"velo-launcher/internal/hotkey"
	"velo-launcher/internal/platform"
	"velo-launcher/internal/search"
)

type App struct {
	mu            sync.Mutex
	ctx           context.Context
	logger        *slog.Logger
	service       *core.Service
	window        launcherWindow
	key           *platform.Hotkey
	keyError      string
	tray          *platform.Tray
	background    bool
	diagnostics   bool
	started       time.Time
	clientReady   bool
	exitRequested bool
	closing       bool
}
type launcherWindow interface {
	Visible() bool
	Active() bool
	Show()
	Hide()
	Resize(int, int)
}
type Status struct {
	core.State
	HotkeyError string `json:"hotkey_error"`
	Visible     bool   `json:"visible"`
}

func NewApp(logger *slog.Logger, service *core.Service, background, diagnostics bool, started time.Time) *App {
	return &App{logger: logger, service: service, background: background, diagnostics: diagnostics, started: started}
}
func (a *App) startup(ctx context.Context) {
	a.ctx = ctx
	a.service.Start(ctx, func() { wruntime.EventsEmit(ctx, "index:changed") })
	a.logger.Info("application started", "version", "0.8.0-beta.2")
}
func (a *App) ready(ctx context.Context) {
	a.mu.Lock()
	defer a.mu.Unlock()
	window, err := platform.FindWindow()
	if err != nil {
		a.keyError = err.Error()
		a.logger.Error("window setup failed", "error", err)
		return
	}
	a.window = window
	if a.tray, err = platform.NewTray(platform.TrayActions{Open: a.Show, Settings: a.OpenSettings, Quit: a.Quit}); err != nil {
		a.tray = nil
		a.logger.Warn("tray unavailable", "error", err)
	}
	a.key, err = platform.RegisterHotkey(a.service.Settings().Hotkey, a.Toggle)
	if err != nil {
		a.keyError = err.Error()
		a.logger.Warn("hotkey unavailable", "error", err)
	}
	if a.background && a.key != nil {
		a.window.Hide()
		wruntime.EventsEmit(ctx, "launcher:hidden")
	} else {
		a.window.Show()
		wruntime.EventsEmit(ctx, "launcher:shown")
	}
	a.logger.Info("window ready", "visible", a.window.Visible(), "hotkey_error", a.keyError, "tray", a.tray != nil)
}
func (a *App) shutdown(_ context.Context) {
	a.mu.Lock()
	a.closing = true
	key := a.key
	a.key = nil
	tray := a.tray
	a.tray = nil
	a.mu.Unlock()
	if tray != nil {
		if err := tray.Close(); err != nil {
			a.logger.Warn("tray shutdown", "error", err)
		}
	}
	if key != nil {
		if err := key.Close(); err != nil {
			a.logger.Warn("hotkey shutdown", "error", err)
		}
	}
	a.service.Close()
	a.logger.Info("application stopped")
}
func (a *App) Toggle() {
	a.mu.Lock()
	defer a.mu.Unlock()
	if a.window == nil || a.closing {
		return
	}
	if a.window.Visible() {
		a.hideLocked()
	} else {
		a.window.Show()
		wruntime.EventsEmit(a.ctx, "launcher:shown")
	}
}
func (a *App) Show() {
	a.mu.Lock()
	defer a.mu.Unlock()
	if a.window != nil && !a.closing {
		a.window.Show()
		wruntime.EventsEmit(a.ctx, "launcher:shown")
	}
}

// OpenSettings 供托盘等外部入口直接进入设置面板，窗口已可见时不再重置查询。
func (a *App) OpenSettings() {
	a.mu.Lock()
	defer a.mu.Unlock()
	if a.window == nil || a.closing {
		return
	}
	if !a.window.Visible() {
		a.window.Show()
		wruntime.EventsEmit(a.ctx, "launcher:shown")
	}
	wruntime.EventsEmit(a.ctx, "settings:open")
}
func (a *App) Hide() {
	a.mu.Lock()
	defer a.mu.Unlock()
	a.hideLocked()
}
func (a *App) hideLocked() {
	if a.window == nil || a.closing || !a.window.Visible() {
		return
	}
	a.window.Hide()
	if a.ctx != nil {
		wruntime.EventsEmit(a.ctx, "launcher:hidden")
	}
}

// Route Alt+F4 through the same visibility transition as Escape/global toggle,
// so a hidden WebView does not continue rendering a focused input caret.
func (a *App) beforeClose(_ context.Context) bool {
	a.mu.Lock()
	defer a.mu.Unlock()
	if a.exitRequested || a.key == nil {
		return false
	}
	a.hideLocked()
	return true
}

// Keep the window reachable when the configured global key was rejected.
func (a *App) Blur() {
	a.mu.Lock()
	defer a.mu.Unlock()
	if a.window != nil && a.window.Visible() && a.key != nil && !a.diagnostics && !a.window.Active() {
		a.hideLocked()
	}
}

// FrontendReady measures from process entry through cached results rendered.
func (a *App) FrontendReady() {
	a.mu.Lock()
	defer a.mu.Unlock()
	if !a.clientReady {
		a.clientReady = true
		a.logger.Info("frontend interactive", "elapsed_ms", time.Since(a.started).Milliseconds())
	}
}
func (a *App) Quit() {
	a.mu.Lock()
	a.exitRequested = true
	a.mu.Unlock()
	wruntime.Quit(a.ctx)
}
func (a *App) Search(query string) []search.Result { return a.service.Search(query) }
func (a *App) GetSettings() config.Config          { return a.service.Settings() }
func (a *App) GetStatus() Status {
	a.mu.Lock()
	keyError := a.keyError
	visible := a.window != nil && a.window.Visible()
	a.mu.Unlock()
	return Status{a.service.State(), keyError, visible}
}
func (a *App) RefreshIndex() { a.service.Refresh() }
func (a *App) Resize(height int) {
	a.mu.Lock()
	defer a.mu.Unlock()
	if a.window != nil && !a.closing {
		a.window.Resize(640, min(720, max(160, height)))
	}
}
func (a *App) Launch(id, query string) error {
	item, err := a.service.Item(id)
	if err != nil {
		return err
	}
	a.Hide()
	if err := platform.Launch(item); err != nil {
		a.Show()
		return err
	}
	if err := a.service.Record(id, query); err != nil {
		return fmt.Errorf("应用已启动，但历史记录保存失败: %w", err)
	}
	return nil
}
func (a *App) SaveSettings(c config.Config) error {
	if err := c.Validate(); err != nil {
		return err
	}
	if _, err := hotkey.Parse(c.Hotkey); err != nil {
		return err
	}
	a.mu.Lock()
	defer a.mu.Unlock()
	old := a.service.Settings()
	hadKey := a.key != nil
	var err error
	if hadKey {
		err = a.key.Rebind(c.Hotkey)
	} else {
		a.key, err = platform.RegisterHotkey(c.Hotkey, a.Toggle)
	}
	if err != nil {
		return err
	}
	rollback := func() {
		if hadKey {
			if e := a.key.Rebind(old.Hotkey); e != nil {
				a.keyError = e.Error()
			}
		} else {
			_ = a.key.Close()
			a.key = nil
		}
	}
	if c.LaunchAtStartup != old.LaunchAtStartup {
		if err = platform.SetLaunchAtStartup(c.LaunchAtStartup); err != nil {
			rollback()
			return err
		}
	}
	if err = a.service.SaveSettings(c); err != nil {
		rollback()
		if c.LaunchAtStartup != old.LaunchAtStartup {
			if e := platform.SetLaunchAtStartup(old.LaunchAtStartup); e != nil {
				return fmt.Errorf("配置写入失败: %v；恢复登录启动失败: %w", err, e)
			}
		}
		return err
	}
	a.keyError = ""
	return nil
}
