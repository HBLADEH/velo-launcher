package app

import (
	"context"
	"fmt"
	"log/slog"
	"os"
	"path/filepath"
	"slices"
	"sync"
	"time"

	"velo-launcher/internal/config"
	"velo-launcher/internal/history"
	"velo-launcher/internal/icon"
	"velo-launcher/internal/indexer"
	"velo-launcher/internal/model"
	"velo-launcher/internal/platform"
	"velo-launcher/internal/search"
	"velo-launcher/internal/storage"
)

type State struct {
	Count            int      `json:"count"`
	Scanning         bool     `json:"scanning"`
	LastRefresh      string   `json:"last_refresh"`
	ScanMilliseconds int64    `json:"scan_milliseconds"`
	Warnings         []string `json:"warnings"`
}
type Service struct {
	// Serialize settings writes and refresh commits without blocking readers on disk I/O.
	commitMu sync.Mutex
	mu       sync.RWMutex
	dir      string
	config   config.Config
	revision uint64
	index    *search.Index
	items    map[string]model.AppItem
	cache    indexer.Cache
	history  *history.Store
	state    State
	logger   *slog.Logger
	wake     chan struct{}
	done     chan struct{}
	cancel   context.CancelFunc
	onChange func()
}

func New(dir string, logger *slog.Logger) (*Service, error) {
	c, warning, err := config.Load(filepath.Join(dir, "config.json"))
	if err != nil {
		return nil, err
	}
	s := &Service{dir: dir, config: c, logger: logger, items: map[string]model.AppItem{}, wake: make(chan struct{}, 1), done: make(chan struct{}), state: State{Warnings: []string{}}}
	if warning != "" {
		s.state.Warnings = append(s.state.Warnings, warning)
	}
	if err := storage.Read(filepath.Join(dir, "index.json"), &s.cache); err != nil && !os.IsNotExist(err) {
		s.state.Warnings = append(s.state.Warnings, "索引缓存无效，将重新扫描")
		s.cache = indexer.Cache{}
	}
	if s.cache.Version != 1 {
		s.cache = indexer.Cache{}
	}
	s.replace(s.cache.Apps)
	historyPath := filepath.Join(dir, "history.json")
	s.history, err = history.Open(historyPath)
	if err != nil {
		backup := historyPath + ".invalid-" + time.Now().Format("20060102-150405.000000000")
		if err := os.Rename(historyPath, backup); err != nil {
			return nil, fmt.Errorf("保留损坏历史: %w", err)
		}
		s.history, err = history.Open(historyPath)
		if err != nil {
			return nil, err
		}
		s.state.Warnings = append(s.state.Warnings, "历史记录损坏，已保留原文件并重建")
	}
	return s, nil
}
func (s *Service) replace(items []model.AppItem) {
	// 调用方必须持有 s.mu，或在启动阶段独占地初始化。
	visible := indexer.Visible(items, s.config.FilterNoise)
	s.index = search.New(visible)
	s.items = make(map[string]model.AppItem, len(visible))
	for _, item := range visible {
		s.items[item.ID] = item
	}
	s.state.Count = len(visible)
}
func (s *Service) Start(parent context.Context, onChange func()) {
	ctx, cancel := context.WithCancel(parent)
	s.cancel = cancel
	s.mu.Lock()
	s.onChange = onChange
	s.mu.Unlock()
	go func() {
		defer close(s.done)
		for {
			s.refresh(ctx)
			c := s.Settings()
			timer := time.NewTimer(time.Duration(c.RefreshMinutes) * time.Minute)
			select {
			case <-ctx.Done():
				timer.Stop()
				return
			case <-s.wake:
				timer.Stop()
			case <-timer.C:
			}
		}
	}()
}
func (s *Service) Close() {
	if s.cancel != nil {
		s.cancel()
		<-s.done
	}
}
func (s *Service) Refresh() {
	select {
	case s.wake <- struct{}{}:
	default:
	}
}
func (s *Service) Settings() config.Config {
	s.mu.RLock()
	defer s.mu.RUnlock()
	c := s.config
	c.CustomDirectories = append([]string{}, c.CustomDirectories...)
	return c
}
func (s *Service) SaveSettings(c config.Config) error {
	c.CustomDirectories = slices.Clone(c.CustomDirectories)
	if err := c.Validate(); err != nil {
		return err
	}
	s.commitMu.Lock()
	defer s.commitMu.Unlock()
	if err := storage.Write(filepath.Join(s.dir, "config.json"), c); err != nil {
		return err
	}
	s.mu.Lock()
	old := s.config
	s.config = c
	filterChanged := old.FilterNoise != c.FilterNoise
	rescan := old.ScanProgramFiles != c.ScanProgramFiles || filterChanged || !slices.Equal(old.CustomDirectories, c.CustomDirectories)
	if rescan {
		s.revision++
	}
	s.mu.Unlock()
	if filterChanged {
		// 先用现有缓存重建索引，让开关立即生效；深层辅助程序的排除在后台重扫后生效。
		s.mu.Lock()
		s.replace(s.cache.Apps)
		s.mu.Unlock()
		s.notify()
	}
	if rescan || old.RefreshMinutes != c.RefreshMinutes {
		s.Refresh()
	}
	return nil
}
func (s *Service) State() State {
	s.mu.RLock()
	defer s.mu.RUnlock()
	state := s.state
	state.Warnings = append([]string{}, state.Warnings...)
	return state
}
func (s *Service) Search(query string) []search.Result {
	query = search.Normalize(query)
	s.mu.RLock()
	i, c := s.index, s.config
	s.mu.RUnlock()
	return i.Query(query, c.MaxResults, c.Search.Fuzzy, s.history.Scores(query, time.Now()), c.Search.HistoryWeight)
}
func (s *Service) Item(id string) (model.AppItem, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	item, ok := s.items[id]
	if !ok {
		return item, fmt.Errorf("索引已更新，请重新选择应用")
	}
	return item, nil
}
func (s *Service) Record(id, query string) error {
	return s.history.Record(id, search.Normalize(query), time.Now())
}

// notify 在索引或可见条目变化后通知前端刷新。
func (s *Service) notify() {
	s.mu.RLock()
	change := s.onChange
	s.mu.RUnlock()
	if change != nil {
		change()
	}
}
func (s *Service) refresh(ctx context.Context) {
	start := time.Now()
	s.mu.Lock()
	c, old, rev := s.config, s.cache, s.revision
	s.state.Scanning = true
	s.mu.Unlock()
	s.notify()
	next, warnings, err := scan(ctx, c, old, s.dir)
	s.commitRefresh(ctx, rev, next, warnings, err, start)
}

func (s *Service) commitRefresh(ctx context.Context, rev uint64, next indexer.Cache, warnings []string, err error, start time.Time) {
	s.commitMu.Lock()
	defer s.commitMu.Unlock()
	if ctx.Err() != nil {
		s.mu.Lock()
		s.state.Scanning = false
		s.mu.Unlock()
		return
	}
	s.mu.RLock()
	obsolete := rev != s.revision
	s.mu.RUnlock()
	if obsolete {
		s.mu.Lock()
		s.state.Scanning = false
		s.mu.Unlock()
		s.Refresh()
		return
	}
	persisted := false
	if err == nil {
		if e := storage.Write(filepath.Join(s.dir, "index.json"), next); e != nil {
			warnings = append(warnings, "索引缓存写入失败: "+e.Error())
		} else {
			persisted = true
		}
	}
	s.mu.Lock()
	previous := s.cache.Apps
	s.state.Scanning = false
	s.state.ScanMilliseconds = time.Since(start).Milliseconds()
	if err != nil {
		s.state.Warnings = append(warnings, err.Error())
	} else {
		s.cache = next
		s.replace(next.Apps)
		s.state.LastRefresh = time.Now().Format(time.RFC3339)
		s.state.Warnings = warnings
	}
	count := s.state.Count
	s.mu.Unlock()
	// Never delete icons from the persisted index if its replacement failed.
	// Retain one previous generation for results still displayed by the frontend.
	if persisted {
		if e := icon.Prune(s.dir, append(slices.Clone(next.Apps), previous...)); e != nil {
			s.logger.Warn("icon cleanup failed", "error", e)
		}
	}
	s.logger.Info("index refreshed", "apps", count, "duration_ms", time.Since(start).Milliseconds(), "error", err)
	s.notify()
}
func scan(ctx context.Context, c config.Config, old indexer.Cache, dataDir string) (indexer.Cache, []string, error) {
	resolver, err := platform.NewResolver()
	if err != nil {
		return old, nil, err
	}
	defer resolver.Close()
	next, warnings, err := indexer.Scan(ctx, platform.Roots(c), old, resolver)
	if err != nil {
		return old, warnings, err
	}
	apps, err := platform.WindowsApps(ctx)
	if err != nil {
		warnings = append(warnings, err.Error())
		for _, item := range old.Apps {
			if item.Source == "Windows Apps" {
				apps = append(apps, item)
			}
		}
	}
	next.Apps = indexer.Deduplicate(append(next.Apps, apps...))
	if err := icon.Populate(ctx, dataDir, next.Apps, platform.ExtractIcon); err != nil {
		warnings = append(warnings, "图标缓存: "+err.Error())
	}
	return next, warnings, nil
}
