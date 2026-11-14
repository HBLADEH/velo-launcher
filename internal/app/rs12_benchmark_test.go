package app

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"testing"
	"time"

	"velo-launcher/internal/config"
	"velo-launcher/internal/history"
	"velo-launcher/internal/model"
	"velo-launcher/internal/search"
	"velo-launcher/internal/storage"
)

// Opt-in measurements share serialized inputs with the Rust Release example.
// No scanner, UI, startup time or fixture construction is inside the timer.
func TestRS12SearchMeasurement(t *testing.T) {
	directory := os.Getenv("VELO_RS12_BENCH_DIR")
	if directory == "" {
		t.Skip("set VELO_RS12_BENCH_DIR to a fresh local evidence directory")
	}
	if err := os.Mkdir(directory, 0700); err != nil {
		t.Fatal(err)
	}
	now := time.Date(2026, 11, 4, 12, 0, 0, 0, time.UTC)
	items := make([]model.AppItem, 10000)
	entries := make(map[string]history.Entry, len(items))
	for i := range items {
		items[i] = model.AppItem{ID: fmt.Sprintf("app-%05d", i), Name: fmt.Sprintf("Application %05d Visual Studio Editor 编辑器", i), Path: fmt.Sprintf(`C:\VeloBenchmark\app-%05d.exe`, i), Source: "Start Menu"}
		entries[items[i].ID] = history.Entry{Count: 1 + i%50, Last: now.Add(-24 * time.Hour), Queries: map[string]int{"editor": i % 7, "vs": i % 3, "edtor": i % 2}}
	}
	tools := []model.AppItem{
		{ID: "tool-calc", Name: "计算器", Source: "System", Keywords: []string{"calc", "calculator"}},
		{ID: "tool-editor", Name: "系统编辑器", Source: "System", Keywords: []string{"editor"}},
		{ID: "tool-settings", Name: "设置", Source: "System", Keywords: []string{"settings"}},
	}
	c := config.Defaults()
	queries := []string{"editor", "vs", "edtor", "", "zzzz", "计算器"}
	fixture := struct {
		Now     time.Time                `json:"now"`
		Apps    []model.AppItem          `json:"apps"`
		Tools   []model.AppItem          `json:"tools"`
		Config  config.Config            `json:"config"`
		History map[string]history.Entry `json:"history"`
		Queries []string                 `json:"queries"`
	}{now, items, tools, c, entries, queries}
	if err := storage.Write(filepath.Join(directory, "fixture.json"), fixture); err != nil {
		t.Fatal(err)
	}
	type record struct {
		Count                   int      `json:"count"`
		History                 bool     `json:"history"`
		Layer                   string   `json:"layer"`
		Query                   string   `json:"query"`
		Round                   int      `json:"round"`
		Iterations              int      `json:"iterations"`
		ElapsedNanoseconds      int64    `json:"elapsed_nanoseconds"`
		NanosecondsPerOperation float64  `json:"nanoseconds_per_operation"`
		ResultIDs               []string `json:"result_ids"`
	}
	records := []record{}
	for _, count := range []int{1000, 10000} {
		for _, learned := range []bool{false, true} {
			historyPath := filepath.Join(t.TempDir(), "history.json")
			subset := map[string]history.Entry{}
			if learned {
				for _, item := range items[:count] {
					subset[item.ID] = entries[item.ID]
				}
			}
			if err := storage.Write(historyPath, subset); err != nil {
				t.Fatal(err)
			}
			h, err := history.Open(historyPath)
			if err != nil {
				t.Fatal(err)
			}
			pinned := append(append([]model.AppItem{}, items[:4]...), tools[0])
			s := &Service{config: c, history: h, quick: pinned, tools: tools}
			s.replace(items[:count])
			kernel := search.New(items[:count])
			for _, query := range queries {
				weights := h.Scores(query, now)
				for _, layer := range []string{"kernel", "service"} {
					call := func() []search.Result {
						if layer == "kernel" {
							return kernel.Query(query, c.MaxResults, c.Search.Fuzzy, weights, c.Search.HistoryWeight)
						}
						return s.searchAt(query, now)
					}
					for warm := 0; warm < 8; warm++ {
						benchmarkSearch = call()
					}
					for round := 1; round <= 3; round++ {
						const iterations = 256
						started := time.Now()
						for n := 0; n < iterations; n++ {
							benchmarkSearch = call()
						}
						elapsed := time.Since(started).Nanoseconds()
						ids := []string{}
						for _, result := range benchmarkSearch {
							ids = append(ids, result.ID)
						}
						records = append(records, record{count, learned, layer, query, round, iterations, elapsed, float64(elapsed) / iterations, ids})
					}
				}
			}
		}
	}
	data, err := json.MarshalIndent(records, "", "  ")
	if err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(directory, "go.json"), data, 0600); err != nil {
		t.Fatal(err)
	}
}
