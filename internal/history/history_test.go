package history

import (
	"os"
	"path/filepath"
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
