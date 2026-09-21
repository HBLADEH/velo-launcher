package app

import (
	"fmt"
	"os"
	"path/filepath"
	"slices"

	"velo-launcher/internal/model"
	"velo-launcher/internal/storage"
)

// Store custom search entries and pins in one atomic document so removing an
// application cannot leave a pin that resurrects it on restart.
type applicationLibrary struct {
	Version      int             `json:"version"`
	Applications []model.AppItem `json:"applications"`
	Pinned       []model.AppItem `json:"pinned"`
}

func (s *Service) loadLibrary() error {
	path := filepath.Join(s.dir, "application-library.json")
	var library applicationLibrary
	err := storage.Read(path, &library)
	if err == nil {
		if library.Version != 1 {
			return fmt.Errorf("不支持的应用库版本 %d，请升级 Velo", library.Version)
		}
		s.custom, s.quick = library.Applications, library.Pinned
		return nil
	}
	if !os.IsNotExist(err) {
		return fmt.Errorf("读取应用库失败（原文件已保留）: %w", err)
	}
	// The old file mixed imported applications with pins. Migrate once; leave
	// the original intact as a backup. An existing new file is authoritative.
	var legacy []model.AppItem
	if err := storage.Read(filepath.Join(s.dir, "quick-launch.json"), &legacy); err != nil {
		if os.IsNotExist(err) {
			return nil
		}
		return fmt.Errorf("读取旧快速启动列表失败（原文件已保留）: %w", err)
	}
	s.quick = slices.Clone(legacy)
	for n, item := range legacy {
		if item.Source == "Quick Launch" {
			item.Source, item.Pinned = "Manual", false
			s.custom = append(s.custom, item)
			s.quick[n].Source = "Manual"
		}
	}
	return storage.Write(path, applicationLibrary{1, s.custom, s.quick})
}

// commitLibrary requires commitMu. Readers keep the old snapshot if saving fails.
func (s *Service) commitLibrary(applications, pinned []model.AppItem) error {
	if err := storage.Write(filepath.Join(s.dir, "application-library.json"), applicationLibrary{1, applications, pinned}); err != nil {
		return err
	}
	s.mu.Lock()
	s.custom, s.quick = applications, pinned
	s.replace(s.cache.Apps)
	s.mu.Unlock()
	s.notify()
	return nil
}

func (s *Service) CustomApplications() []model.AppItem {
	s.mu.RLock()
	defer s.mu.RUnlock()
	items := append([]model.AppItem{}, s.custom...)
	for n := range items {
		items[n].Keywords = slices.Clone(items[n].Keywords)
		items[n].Pinned = slices.ContainsFunc(s.quick, func(a model.AppItem) bool { return a.ID == items[n].ID })
	}
	return items
}

func (s *Service) DeleteApplication(id string) error {
	s.commitMu.Lock()
	defer s.commitMu.Unlock()
	s.mu.RLock()
	applications, pinned := slices.Clone(s.custom), slices.Clone(s.quick)
	s.mu.RUnlock()
	if !slices.ContainsFunc(applications, func(a model.AppItem) bool { return a.ID == id }) {
		return fmt.Errorf("自定义应用不存在，请刷新列表")
	}
	applications = slices.DeleteFunc(applications, func(a model.AppItem) bool { return a.ID == id })
	pinned = slices.DeleteFunc(pinned, func(a model.AppItem) bool { return a.ID == id })
	return s.commitLibrary(applications, pinned)
}
