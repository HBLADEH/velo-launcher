package indexer

import (
	"context"
	"os"
	"path/filepath"
	"testing"
	"velo-launcher/internal/model"
)

type resolver struct{ calls int }

func (r *resolver) Resolve(path string) (model.AppItem, error) {
	r.calls++
	return model.AppItem{ExecPath: filepath.Join(filepath.Dir(path), "code.exe")}, nil
}
func TestIncrementalDedupAndDeletion(t *testing.T) {
	dir := t.TempDir()
	for _, name := range []string{"Code.lnk", "code.exe", "ignored.txt"} {
		if err := os.WriteFile(filepath.Join(dir, name), []byte("one"), 0600); err != nil {
			t.Fatal(err)
		}
	}
	r := &resolver{}
	roots := []Root{{dir, "Start Menu"}}
	cache, _, err := Scan(context.Background(), roots, Cache{}, r)
	if err != nil {
		t.Fatal(err)
	}
	if len(cache.Apps) != 1 || cache.Apps[0].Name != "Code" || r.calls != 1 {
		t.Fatalf("dedup: %+v calls=%d", cache.Apps, r.calls)
	}
	cache, _, err = Scan(context.Background(), roots, cache, r)
	if err != nil || r.calls != 1 {
		t.Fatal("unchanged link resolved again")
	}
	if err := os.WriteFile(filepath.Join(dir, "Code.lnk"), []byte("changed file"), 0600); err != nil {
		t.Fatal(err)
	}
	cache, _, err = Scan(context.Background(), roots, cache, r)
	if err != nil || r.calls != 2 {
		t.Fatal("changed link not resolved")
	}
	if err := os.Remove(filepath.Join(dir, "Code.lnk")); err != nil {
		t.Fatal(err)
	}
	cache, _, err = Scan(context.Background(), roots, cache, r)
	if err != nil || len(cache.Apps) != 1 || cache.Apps[0].Name != "code" {
		t.Fatal("deleted shortcut not removed")
	}
}
func TestCancelledScan(t *testing.T) {
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	if _, _, err := Scan(ctx, []Root{{t.TempDir(), "Custom"}}, Cache{}, &resolver{}); err == nil {
		t.Fatal("cancellation ignored")
	}
}
