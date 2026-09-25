package app

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"slices"
	"strings"
	"time"

	"velo-launcher/internal/icon"
	"velo-launcher/internal/model"
	"velo-launcher/internal/platform"
	"velo-launcher/internal/search"
)

type Home struct {
	Pinned   []model.AppItem `json:"pinned"`
	Frequent []model.AppItem `json:"frequent"`
	Tools    []model.AppItem `json:"tools"`
}

type ImportResult struct {
	Added    int      `json:"added"`
	Warnings []string `json:"warnings"`
}

func (s *Service) Home() Home {
	s.mu.RLock()
	defer s.mu.RUnlock()
	home := Home{Pinned: []model.AppItem{}, Frequent: []model.AppItem{}, Tools: []model.AppItem{}}
	pinned := map[string]bool{}
	for _, item := range s.quick {
		if current, ok := s.items[item.ID]; ok {
			item = current
		}
		if item.Source == "System" {
			available := false
			for _, tool := range s.tools {
				if tool.ID == item.ID {
					item = tool
					available = true
					break
				}
			}
			if !available {
				continue
			}
		}
		item.Pinned = true
		pinned[item.ID] = true
		home.Pinned = append(home.Pinned, item)
	}
	weights := s.history.Scores("", time.Now())
	// Personal use takes precedence; cold-start suggestions only fill empty slots.
	// Search's history weight must not disable learning on the home screen.
	results := s.index.Query("", len(s.items), false, nil, 0)
	slices.SortStableFunc(results, func(a, b search.Result) int {
		if weights[a.ID] > weights[b.ID] {
			return -1
		}
		if weights[a.ID] < weights[b.ID] {
			return 1
		}
		return 0
	})
	for _, result := range results {
		if !pinned[result.ID] {
			home.Frequent = append(home.Frequent, result.AppItem)
			if len(home.Frequent) == 12 {
				break
			}
		}
	}
	for _, tool := range s.tools {
		if !pinned[tool.ID] {
			home.Tools = append(home.Tools, tool)
		}
	}
	slices.SortStableFunc(home.Tools, func(a, b model.AppItem) int {
		if weights[a.ID] > weights[b.ID] {
			return -1
		}
		if weights[a.ID] < weights[b.ID] {
			return 1
		}
		return 0
	})
	return home
}

// commitQuick is called with commitMu held. Publish only after persistence.
func (s *Service) commitQuick(next []model.AppItem) error {
	return s.commitLibrary(s.custom, next)
}

func (s *Service) SetPinned(id string, pinned bool) error {
	s.commitMu.Lock()
	defer s.commitMu.Unlock()
	s.mu.RLock()
	next := slices.Clone(s.quick)
	s.mu.RUnlock()
	if !pinned {
		next = slices.DeleteFunc(next, func(a model.AppItem) bool { return a.ID == id })
	} else if !slices.ContainsFunc(next, func(a model.AppItem) bool { return a.ID == id }) {
		if len(next) >= 200 {
			return fmt.Errorf("快速启动最多保存 200 项，请先移除不需要的项目")
		}
		item, err := s.Item(id)
		if err != nil {
			return err
		}
		item.Pinned = true
		next = append(next, item)
	}
	return s.commitQuick(next)
}

// AddApplications registers files but never executes or pins them. Imports share the
// refresh commit lock so icon cleanup cannot delete a newly imported icon.
func (s *Service) AddApplications(paths []string) (ImportResult, error) {
	result := ImportResult{Warnings: []string{}}
	if len(paths) == 0 {
		return result, nil
	}
	if len(paths) > 100 {
		return result, fmt.Errorf("每次最多添加 100 个应用")
	}
	s.commitMu.Lock()
	defer s.commitMu.Unlock()
	resolver, err := platform.NewResolver()
	if err != nil {
		return result, err
	}
	defer resolver.Close()
	s.mu.RLock()
	next := slices.Clone(s.custom)
	s.mu.RUnlock()
	for _, path := range paths {
		item, err := quickItem(path, resolver)
		if err != nil {
			result.Warnings = append(result.Warnings, err.Error())
			continue
		}
		if slices.ContainsFunc(next, func(a model.AppItem) bool { return a.ID == item.ID }) {
			continue
		}
		if len(next) >= 1000 {
			result.Warnings = append(result.Warnings, "自定义应用已达到 1000 项上限")
			break
		}
		icons := []model.AppItem{item}
		if err := icon.Populate(context.Background(), s.dir, icons, platform.ExtractIcon); err != nil {
			result.Warnings = append(result.Warnings, item.Name+"：图标读取失败，将使用默认图标")
		}
		next = append(next, icons[0])
		result.Added++
	}
	if result.Added == 0 {
		return result, nil
	}
	if err := s.commitLibrary(next, s.quick); err != nil {
		result.Added = 0
		return result, err
	}
	return result, nil
}

type shortcutResolver interface {
	Resolve(string) (model.AppItem, error)
}

func quickItem(path string, resolver shortcutResolver) (model.AppItem, error) {
	item := model.AppItem{}
	if !filepath.IsAbs(path) {
		return item, fmt.Errorf("需要完整文件路径：%s", path)
	}
	path = filepath.Clean(path)
	ext := strings.ToLower(filepath.Ext(path))
	if ext != ".exe" && ext != ".lnk" {
		return item, fmt.Errorf("仅支持 .exe 程序和 .lnk 快捷方式：%s", filepath.Base(path))
	}
	info, err := os.Stat(path)
	if err != nil {
		return item, fmt.Errorf("无法读取 %s：%w", filepath.Base(path), err)
	}
	if info.IsDir() {
		return item, fmt.Errorf("请拖入程序或快捷方式，而非目录：%s", path)
	}
	if ext == ".lnk" {
		item, err = resolver.Resolve(path)
		if err != nil {
			return item, fmt.Errorf("读取快捷方式 %s：%w", filepath.Base(path), err)
		}
	} else {
		item.ExecPath = path
		item.WorkingDirectory = filepath.Dir(path)
	}
	item.Path = path
	item.Name = strings.TrimSuffix(filepath.Base(path), filepath.Ext(path))
	item.Source = "Manual"
	identity := item.ExecPath
	if identity == "" {
		identity = path
	}
	item.ID = model.Identity(identity, item.Arguments)
	return item, nil
}

// systemTools is refreshed with the index to reflect installed Windows features.
func systemTools() []model.AppItem { return platform.SystemTools() }
