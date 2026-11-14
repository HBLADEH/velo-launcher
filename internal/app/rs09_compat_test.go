package app

import (
	"fmt"
	"os"
	"path/filepath"
	"reflect"
	"testing"
	"time"

	"velo-launcher/internal/compat"
	"velo-launcher/internal/history"
	"velo-launcher/internal/search"
	"velo-launcher/internal/storage"
)

// Use the actual private service at a fixed clock. Only the time boundary was
// factored out; no independent reimplementation is used to predict home order.
func TestRS09Behavior(t *testing.T) {
	path := "../../tests/fixtures/compat/rs09.json"
	f, err := compat.LoadBehavior(path)
	if err != nil {
		t.Fatal(err)
	}
	refresh := os.Getenv("VELO_RS09_REFRESH") == "1"
	if err := compat.CheckBehavior(&f, refresh); err != nil {
		t.Fatal(err)
	}
	now, err := time.Parse(time.RFC3339Nano, f.FixedTime)
	if err != nil {
		t.Fatal(err)
	}
	for n := range f.Services {
		c := &f.Services[n]
		dir := t.TempDir()
		historyPath := filepath.Join(dir, "history.json")
		if err := storage.Write(historyPath, c.History); err != nil {
			t.Fatal(err)
		}
		h, err := history.Open(historyPath)
		if err != nil {
			t.Fatal(err)
		}
		s := &Service{dir: dir, config: c.Config, history: h, custom: c.Library.Applications, quick: c.Library.Pinned, tools: c.Tools}
		s.cache.Apps = c.Apps
		s.replace(c.Apps)
		states := []compat.ServiceState{}
		snapshot := func() {
			home := s.homeAt(now)
			state := compat.ServiceState{Library: compat.Library{Version: 1, Applications: s.custom, Pinned: s.quick}, Home: compat.HomeResult{Pinned: home.Pinned, Frequent: home.Frequent, Tools: home.Tools}, Searches: [][]search.Result{}}
			for _, query := range c.Queries {
				state.Searches = append(state.Searches, s.searchAt(query, now))
			}
			states = append(states, state)
		}
		snapshot()
		for _, pin := range c.Pins {
			err := s.SetPinned(pin.ID, pin.Pinned)
			if (err != nil) != pin.Error {
				t.Fatalf("pin error mismatch: %s", c.Name)
			}
			snapshot()
		}
		if refresh {
			c.Expected = states
		} else if !reflect.DeepEqual(states, c.Expected) {
			t.Fatalf("service mismatch: %s", c.Name)
		}
	}
	if refresh {
		if err := storage.Write(path, f); err != nil {
			t.Fatal(err)
		}
	}
}

func TestRS09TiedHistoryEviction(t *testing.T) {
	path := filepath.Join(t.TempDir(), "history.json")
	queries := map[string]int{}
	for n := 0; n < 100; n++ {
		queries[fmt.Sprintf("q%03d", n)] = 1
	}
	if err := storage.Write(path, map[string]history.Entry{"used": {Queries: queries}}); err != nil {
		t.Fatal(err)
	}
	store, err := history.Open(path)
	if err != nil {
		t.Fatal(err)
	}
	if err := store.Record("used", "new", time.Date(2026, 11, 3, 0, 0, 0, 0, time.UTC)); err != nil {
		t.Fatal(err)
	}
	var saved map[string]history.Entry
	if err := storage.Read(path, &saved); err != nil {
		t.Fatal(err)
	}
	if len(saved["used"].Queries) != 100 || saved["used"].Queries["new"] != 1 {
		t.Fatal("query cap/current-query retention changed")
	}
	removed := 0
	for query := range queries {
		if _, exists := saved["used"].Queries[query]; !exists {
			removed++
		}
	}
	if removed != 1 {
		t.Fatal("must remove one of the tied old queries")
	}
}
