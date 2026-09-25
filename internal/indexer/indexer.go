package indexer

import (
	"context"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"slices"
	"sort"
	"strings"
	"velo-launcher/internal/model"
)

type Root struct {
	Path   string
	Source string
	// MaxDepth 限制根目录下的层级：可启动的主程序通常位于安装目录顶层，
	// 深层可执行文件多为组件或工具。0 表示不限制。
	MaxDepth int
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
			if beyondDepth(root, path) {
				return nil
			}
			info, err := d.Info()
			if err != nil {
				return nil
			}
			key := strings.ToLower(path)
			if cached, ok := old.Files[key]; ok && cached.Modified == info.ModTime().UnixNano() && cached.Size == info.Size() {
				cached.Item.Source = root.Source
				// Targets can disappear without the shortcut itself changing.
				if validDesktopShortcut(cached.Item) {
					next.Files[key] = cached
				}
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
			if !validDesktopShortcut(item) {
				return nil
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
		if validDesktopShortcut(f.Item) {
			items = append(items, f.Item)
		}
	}
	next.Apps = Deduplicate(items)
	return next, warnings, nil
}

// Desktop discovery only accepts file-target shortcuts, not empty shell links,
// failed resolutions, or broken targets. The .lnk itself remains the launch path
// so Windows preserves arguments, working directory and elevation semantics.
func validDesktopShortcut(item model.AppItem) bool {
	if item.Source != "Desktop" || !strings.EqualFold(filepath.Ext(item.Path), ".lnk") {
		return true
	}
	if !filepath.IsAbs(item.ExecPath) || strings.EqualFold(item.ExecPath, item.Path) {
		return false
	}
	info, err := os.Stat(item.ExecPath)
	return err == nil && !info.IsDir()
}

// beyondDepth 只保留浅层程序：安装目录顶层是应用主程序，深层可执行文件
// 通常是捆绑组件或开发工具（如 Git\usr\bin、Windows Kits\...\bin）。
func beyondDepth(root Root, path string) bool {
	if root.MaxDepth <= 0 {
		return false
	}
	relative, err := filepath.Rel(root.Path, filepath.Dir(path))
	if err != nil || relative == "." {
		return false
	}
	return strings.Count(relative, string(filepath.Separator))+1 > root.MaxDepth
}

// Deduplicate keeps one entry per application. Desktop shortcuts win over Start
// Menu entries and executable filenames; shallower paths win over helper copies
// bundled inside another program, and the ID still separates different
// profiles/commands of the same executable.
func Deduplicate(items []model.AppItem) []model.AppItem {
	priority := func(a model.AppItem) int {
		if a.Source == "Windows Apps" {
			return 3
		}
		if strings.EqualFold(filepath.Ext(a.Path), ".lnk") {
			if a.Source == "Desktop" {
				return 0
			}
			if a.Source == "Start Menu" {
				return 1
			}
			return 2
		}
		return 4
	}
	rank := func(a model.AppItem) (int, int, string) {
		exec := a.ExecPath
		if exec == "" {
			exec = a.Path
		}
		return priority(a), strings.Count(filepath.Clean(exec), string(filepath.Separator)), strings.ToLower(a.Path)
	}
	sort.Slice(items, func(i, j int) bool {
		pi, di, pathi := rank(items[i])
		pj, dj, pathj := rank(items[j])
		if pi != pj {
			return pi < pj
		}
		if di != dj {
			return di < dj
		}
		if pathi != pathj {
			return pathi < pathj
		}
		return items[i].ID < items[j].ID
	})
	seenID := map[string]int{}
	seenName := map[string]bool{}
	out := make([]model.AppItem, 0, len(items))
	for _, item := range items {
		if n, ok := seenID[item.ID]; ok {
			// Keep alternate shortcut names searchable without changing launch identity.
			for _, alias := range append([]string{item.Name}, item.Keywords...) {
				if alias != "" && alias != out[n].Name && !slices.Contains(out[n].Keywords, alias) {
					out[n].Keywords = append(slices.Clone(out[n].Keywords), alias)
				}
			}
			continue
		}
		name := duplicateKey(item)
		if seenName[name] {
			continue
		}
		seenID[item.ID] = len(out)
		seenName[name] = true
		out = append(out, item)
	}
	sort.Slice(out, func(i, j int) bool {
		if out[i].Name != out[j].Name {
			return out[i].Name < out[j].Name
		}
		return out[i].ID < out[j].ID
	})
	return out
}

// duplicateKey groups the same program published from several locations, for
// example one shortcut in the user Start Menu and one in the common Start Menu.
// The target keeps different programs that merely share a name apart.
func duplicateKey(item model.AppItem) string {
	target := item.ExecPath
	if target == "" {
		target = item.Path
	}
	return strings.ToLower(strings.Join(strings.Fields(item.Name), " ")) + "\x00" + strings.ToLower(filepath.Base(target))
}
