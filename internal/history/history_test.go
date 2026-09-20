package history

import (
	"os"
	"path/filepath"
	"sync"
	"testing"
	"time"
)

func TestPersistenceAndQueryWeights(t *testing.T) {
	path := filepath.Join(t.TempDir(), "history.json")
	s, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	now := time.Now()
	for i := 0; i < 3; i++ {
		if err = s.Record("code", "c", now); err != nil {
			t.Fatal(err)
		}
	}
	s, err = Open(path)
	if err != nil {
		t.Fatal(err)
	}
	if s.entries["code"].Count != 3 {
		t.Fatal("count not persisted")
	}
	if s.Scores("c", now)["code"] <= s.Scores("x", now)["code"] {
		t.Fatal("query weight missing")
	}
	if s.Scores("c", now.Add(30*24*time.Hour))["code"] >= s.Scores("c", now)["code"] {
		t.Fatal("recency does not decay")
	}
}

func TestScoresDoNotWaitForPersistence(t *testing.T) {
	s, err := Open(filepath.Join(t.TempDir(), "history.json"))
	if err != nil {
		t.Fatal(err)
	}
	entered, release := make(chan struct{}), make(chan struct{})
	done := make(chan error, 1)
	s.persist = func(string, any) error { close(entered); <-release; return nil }
	go func() { done <- s.Record("code", "c", time.Now()) }()
	<-entered
	scores := make(chan map[string]float64, 1)
	go func() { scores <- s.Scores("c", time.Now()) }()
	select {
	case result := <-scores:
		if len(result) != 0 {
			t.Error("uncommitted history became visible")
		}
	case <-time.After(time.Second):
		t.Error("search waited for disk persistence")
	}
	close(release)
	if err := <-done; err != nil {
		t.Fatal(err)
	}
	if s.Scores("c", time.Now())["code"] == 0 {
		t.Fatal("committed history not published")
	}
}

func TestConcurrentRecordsPreserveAllLaunches(t *testing.T) {
	s, err := Open(filepath.Join(t.TempDir(), "history.json"))
	if err != nil {
		t.Fatal(err)
	}
	var group sync.WaitGroup
	for n := 0; n < 20; n++ {
		group.Add(1)
		go func() {
			defer group.Done()
			if err := s.Record("code", "c", time.Now()); err != nil {
				t.Error(err)
			}
		}()
	}
	group.Wait()
	loaded, err := Open(s.path)
	if err != nil {
		t.Fatal(err)
	}
	if loaded.entries["code"].Count != 20 || loaded.entries["code"].Queries["c"] != 20 {
		t.Fatalf("lost updates: %+v", loaded.entries["code"])
	}
}
func TestWriteFailureRollsBack(t *testing.T) {
	dir := t.TempDir()
	s, err := Open(filepath.Join(dir, "history.json"))
	if err != nil {
		t.Fatal(err)
	}
	if err := os.Mkdir(s.path, 0700); err != nil {
		t.Fatal(err)
	}
	if err := s.Record("id", "q", time.Now()); err == nil {
		t.Fatal("expected persistence failure")
	}
	if len(s.entries) != 0 {
		t.Fatal("failed write mutated history")
	}
}
