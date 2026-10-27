package app

import (
	"fmt"
	"io"
	"log/slog"
	"path/filepath"
	"testing"
	"time"

	"velo-launcher/internal/history"
	"velo-launcher/internal/indexer"
	"velo-launcher/internal/model"
	"velo-launcher/internal/search"
	"velo-launcher/internal/storage"
)

var benchmarkHome Home
var benchmarkSearch []search.Result

// Build through New, outside the timer, to include the real cached service,
// history scoring, pin handling and mixed search path without scanning disk.
// System tools are fixed so installed Windows capabilities cannot change input.
func benchmarkService(b *testing.B, count int, withHistory bool) *Service {
	b.Helper()
	dir := b.TempDir()
	items := make([]model.AppItem, count)
	for i := range items {
		items[i] = model.AppItem{
			ID: fmt.Sprintf("app-%05d", i), Name: fmt.Sprintf("Application %05d Visual Studio Editor 编辑器", i),
			Path: fmt.Sprintf(`C:\VeloBenchmark\app-%05d.exe`, i), Source: "Start Menu",
		}
	}
	tools := []model.AppItem{
		{ID: "tool-calc", Name: "计算器", Source: "System", Keywords: []string{"calc", "calculator"}},
		{ID: "tool-editor", Name: "系统编辑器", Source: "System", Keywords: []string{"editor"}},
		{ID: "tool-settings", Name: "设置", Source: "System", Keywords: []string{"settings"}},
	}
	if err := storage.Write(filepath.Join(dir, "index.json"), indexer.Cache{Version: 1, Apps: items}); err != nil {
		b.Fatal(err)
	}
	pinned := append([]model.AppItem{}, items[:4]...)
	pinned = append(pinned, tools[0])
	if err := storage.Write(filepath.Join(dir, "application-library.json"), applicationLibrary{Version: 1, Pinned: pinned}); err != nil {
		b.Fatal(err)
	}
	if withHistory {
		entries := make(map[string]history.Entry, count+len(tools))
		last := time.Now().Add(-24 * time.Hour)
		for i, item := range append(append([]model.AppItem{}, items...), tools...) {
			entries[item.ID] = history.Entry{Count: 1 + i%50, Last: last, Queries: map[string]int{"editor": i % 7, "vs": i % 3, "edtor": i % 2}}
		}
		if err := storage.Write(filepath.Join(dir, "history.json"), entries); err != nil {
			b.Fatal(err)
		}
	}
	s, err := New(dir, slog.New(slog.NewTextHandler(io.Discard, nil)))
	if err != nil {
		b.Fatal(err)
	}
	s.tools = tools
	s.replace(s.cache.Apps)
	if s.State().Count != count || len(s.Home().Pinned) != 5 || len(s.Home().Frequent) != 12 || len(s.Search("editor")) != s.config.MaxResults {
		b.Fatal("invalid benchmark fixture")
	}
	return s
}

func BenchmarkServiceHome(b *testing.B) {
	for _, count := range []int{100, 1000, 5000} {
		for _, withHistory := range []bool{false, true} {
			b.Run(fmt.Sprintf("apps=%d/history=%t", count, withHistory), func(b *testing.B) {
				s := benchmarkService(b, count, withHistory)
				b.ReportAllocs()
				b.ResetTimer()
				for n := 0; n < b.N; n++ {
					benchmarkHome = s.Home()
				}
			})
		}
	}
}

func BenchmarkServiceSearch(b *testing.B) {
	for _, count := range []int{100, 1000, 5000} {
		for _, withHistory := range []bool{false, true} {
			s := benchmarkService(b, count, withHistory)
			// Match the existing kernel queries and add a system-tool lookup.
			for _, query := range []string{"editor", "vs", "edtor", "", "zzzz", "计算器"} {
				name := query
				if name == "" {
					name = "empty"
				}
				b.Run(fmt.Sprintf("apps=%d/history=%t/query=%s", count, withHistory, name), func(b *testing.B) {
					b.ReportAllocs()
					b.ResetTimer()
					for n := 0; n < b.N; n++ {
						benchmarkSearch = s.Search(query)
					}
				})
			}
		}
	}
}
