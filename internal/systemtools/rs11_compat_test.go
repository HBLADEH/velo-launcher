package systemtools

import (
	"encoding/json"
	"os"
	"path/filepath"
	"reflect"
	"testing"
)

// The Rust catalogue is data extracted from the existing Go definitions. Keep
// this comparison against the private source so additions cannot silently drift.
func TestRS11RustCatalogue(t *testing.T) {
	type entry struct {
		ID          string   `json:"id"`
		Name        string   `json:"name"`
		Target      string   `json:"target"`
		Args        string   `json:"args"`
		Description string   `json:"description"`
		Keywords    []string `json:"keywords"`
		Required    string   `json:"required"`
		MinBuild    uint32   `json:"min_build"`
	}
	want := make([]entry, 0, len(catalogue))
	for _, d := range catalogue {
		want = append(want, entry{d.id, d.name, d.target, d.args, d.description, d.keywords, d.required, d.minBuild})
	}
	path := filepath.Join("..", "..", "rust", "src", "core", "system_tools.json")
	if os.Getenv("VELO_RS11_GENERATE_CATALOGUE") == "1" {
		data, err := json.MarshalIndent(want, "", "  ")
		if err != nil {
			t.Fatal(err)
		}
		if err = os.WriteFile(path, append(data, '\n'), 0600); err != nil {
			t.Fatal(err)
		}
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	var got []entry
	if err = json.Unmarshal(data, &got); err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatal("Rust system tool definitions differ from the Go catalogue")
	}
}
