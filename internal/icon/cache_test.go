package icon

import (
	"context"
	"image"
	"net/http/httptest"
	"os"
	"path/filepath"
	"testing"
	"velo-launcher/internal/model"
)

func TestReuseInvalidationAndServing(t *testing.T) {
	dir := t.TempDir()
	exe := filepath.Join(dir, "app.exe")
	if err := os.WriteFile(exe, []byte("one"), 0600); err != nil {
		t.Fatal(err)
	}
	items := []model.AppItem{{ID: "id", Path: exe, ExecPath: exe}}
	calls := 0
	extract := func(string) (image.Image, error) { calls++; return image.NewNRGBA(image.Rect(0, 0, 32, 32)), nil }
	for i := 0; i < 2; i++ {
		if err := Populate(context.Background(), dir, items, extract); err != nil {
			t.Fatal(err)
		}
	}
	if calls != 1 {
		t.Fatal("cache not reused")
	}
	old := items[0].IconURL
	if err := os.WriteFile(exe, []byte("changed"), 0600); err != nil {
		t.Fatal(err)
	}
	if err := Populate(context.Background(), dir, items, extract); err != nil {
		t.Fatal(err)
	}
	if calls != 2 || items[0].IconURL == old {
		t.Fatal("cache not invalidated")
	}
	h := Handler(dir)
	retained := httptest.NewRecorder()
	h.ServeHTTP(retained, httptest.NewRequest("GET", old, nil))
	if retained.Code != 200 {
		t.Fatal("unaccepted scan removed published icon", retained.Code)
	}
	if err := Prune(dir, items); err != nil {
		t.Fatal(err)
	}
	w := httptest.NewRecorder()
	h.ServeHTTP(w, httptest.NewRequest("GET", items[0].IconURL, nil))
	if w.Code != 200 || w.Header().Get("Content-Type") != "image/png" {
		t.Fatal("icon not served", w.Code)
	}
	for _, path := range []string{"/icons/../../config.json", "/icons/other.png", old} {
		w := httptest.NewRecorder()
		h.ServeHTTP(w, httptest.NewRequest("GET", path, nil))
		if w.Code != 404 {
			t.Fatal("unexpected readable path", path, w.Code)
		}
	}
}
