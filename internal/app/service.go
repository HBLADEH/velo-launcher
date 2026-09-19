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
	s.index = search.New(items)
	s.items = make(map[string]model.AppItem, len(items))
	for _, item := range items {
		s.items[item.ID] = item
	}
	s.state.Count = len(items)
}
func (s *Service) Start(parent context.Context, onChange func()) {
	ctx, cancel := context.WithCancel(parent)
	s.cancel = cancel
	s.onChange = onChange
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
	if err := c.Validate(); err != nil {
		return err
	}
	if err := storage.Write(filepath.Join(s.dir, "config.json"), c); err != nil {
		return err
	}
	s.mu.Lock()
	old := s.config
	s.config = c
	indexChanged := old.ScanProgramFiles != c.ScanProgramFiles || !slices.Equal(old.CustomDirectories, c.CustomDirectories)
	if indexChanged {
		s.revision++
	}
	s.mu.Unlock()
	if indexChanged || old.RefreshMinutes != c.RefreshMinutes {
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
func (s *Service) refresh(ctx context.Context) {
	start := time.Now()
	s.mu.Lock()
	c, old, rev := s.config, s.cache, s.revision
	s.state.Scanning = true
	s.mu.Unlock()
	s.onChange()
	next, warnings, err := scan(ctx, c, old, s.dir)
	if ctx.Err() != nil {
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
	if err == nil {
		if e := storage.Write(filepath.Join(s.dir, "index.json"), next); e != nil {
			warnings = append(warnings, "索引缓存写入失败: "+e.Error())
		}
	}
	s.mu.Lock()
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
	s.logger.Info("index refreshed", "apps", count, "duration_ms", time.Since(start).Milliseconds(), "error", err)
	s.onChange()
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
