package indexer

import (
	"context"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"velo-launcher/internal/model"
)

type Root struct {
	Path   string
	Source string
}
type File struct {
	Modified int64         `json:"modified"`
	Size     int64         `json:"size"`
	Item     model.AppItem `json:"item"`
}
type Cache struct {
	Version int             `json:"version"`
	Files   map[string]File `json:"files"`
	Apps    []model.AppItem `json:"apps"`
}
type Resolver interface {
	Resolve(path string) (model.AppItem, error)
}

// Scan only resolves changed files. Missing files disappear from the new
// snapshot. Inaccessible subtrees retain their previous entries for retry.
func Scan(ctx context.Context, roots []Root, old Cache, resolver Resolver) (Cache, []string, error) {
	next := Cache{Version: 1, Files: map[string]File{}, Apps: []model.AppItem{}}
	warnings := []string{}
	seenRoots := map[string]bool{}
	for _, root := range roots {
		root.Path = filepath.Clean(root.Path)
		key := strings.ToLower(root.Path)
		if seenRoots[key] {
			continue
		}
		seenRoots[key] = true
		err := filepath.WalkDir(root.Path, func(path string, d fs.DirEntry, walkErr error) error {
			if err := ctx.Err(); err != nil {
				return err
			}
			if walkErr != nil {
				if os.IsNotExist(walkErr) {
					if path == root.Path && len(warnings) < 25 {
						warnings = append(warnings, "索引目录不存在: "+path)
					}
					return nil
				}
				if len(warnings) < 25 {
					warnings = append(warnings, fmt.Sprintf("%s: %v", path, walkErr))
				}
				prefix := strings.ToLower(filepath.Clean(path))
				for p, f := range old.Files {
					if p == prefix || strings.HasPrefix(p, prefix+string(filepath.Separator)) {
						next.Files[p] = f
					}
				}
				return nil
			}
			if d.Type()&os.ModeSymlink != 0 {
				return nil
			}
			if d.IsDir() {
				// Packaged apps are enumerated through AppsFolder separately.
				if strings.HasPrefix(root.Source, "Program Files") && strings.EqualFold(path, filepath.Join(root.Path, "WindowsApps")) {
					return filepath.SkipDir
				}
				return nil
			}
			ext := strings.ToLower(filepath.Ext(path))
			if ext != ".exe" && ext != ".lnk" {
				return nil
			}
			info, err := d.Info()
			if err != nil {
				return nil
			}
			key := strings.ToLower(path)
			if cached, ok := old.Files[key]; ok && cached.Modified == info.ModTime().UnixNano() && cached.Size == info.Size() {
				cached.Item.Source = root.Source
				next.Files[key] = cached
				return nil
			}
			item := model.AppItem{Name: strings.TrimSuffix(d.Name(), filepath.Ext(d.Name())), Path: path, ExecPath: path, Source: root.Source, Keywords: []string{}}
			modified := info.ModTime().UnixNano()
			if ext == ".lnk" {
				resolved, err := resolver.Resolve(path)
				if err != nil {
					modified = -1 // Retry transient COM/link failures on the next refresh.
					if len(warnings) < 25 {
						warnings = append(warnings, fmt.Sprintf("快捷方式 %s: %v", path, err))
					}
				} else {
					resolved.Name = item.Name
					resolved.Path = path
					resolved.Source = root.Source
					item = resolved
				}
			}
			identity := item.ExecPath
			if identity == "" {
				identity = item.Path
			}
			item.ID = model.Identity(identity, item.Arguments)
			next.Files[key] = File{modified, info.Size(), item}
			return nil
		})
		if err != nil {
			return old, warnings, err
		}
	}
	items := make([]model.AppItem, 0, len(next.Files))
	for _, f := range next.Files {
		items = append(items, f.Item)
	}
	next.Apps = Deduplicate(items)
	return next, warnings, nil
}
func Deduplicate(items []model.AppItem) []model.AppItem {
	// Prefer friendly Start Menu shortcuts over executable filenames.
	priority := func(a model.AppItem) int {
		if strings.EqualFold(filepath.Ext(a.Path), ".lnk") {
			if a.Source == "Start Menu" {
				return 0
			}
			return 1
		}
		return 2
	}
	sort.Slice(items, func(i, j int) bool {
		pi, pj := priority(items[i]), priority(items[j])
		if pi != pj {
			return pi < pj
		}
		return items[i].Path < items[j].Path
	})
	seen := map[string]bool{}
	out := make([]model.AppItem, 0, len(items))
	for _, item := range items {
		if !seen[item.ID] {
			seen[item.ID] = true
			out = append(out, item)
		}
	}
	sort.Slice(out, func(i, j int) bool {
		if out[i].Name != out[j].Name {
			return out[i].Name < out[j].Name
		}
		return out[i].ID < out[j].ID
	})
	return out
}
