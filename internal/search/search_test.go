package search

import (
	"fmt"
	"testing"
	"velo-launcher/internal/model"
)

func apps(names ...string) []model.AppItem {
	out := []model.AppItem{}
	for i, name := range names {
		out = append(out, model.AppItem{ID: fmt.Sprint(i), Name: name})
	}
	return out
}
func TestMatching(t *testing.T) {
	idx := New(apps("Visual Studio Code", "Windows Terminal", "WeChat", "Calculator", "中文应用"))
	for _, tt := range []struct {
		query, want string
		fuzzy       bool
	}{
		{" CODE ", "Visual Studio Code", false}, {"term", "Windows Terminal", false}, {"wx", "WeChat", false}, {"vsc", "Visual Studio Code", true}, {"clcltr", "Calculator", true}, {"calclator", "Calculator", true}, {"calcualtor", "Calculator", true}, {"中文", "中文应用", false},
	} {
		t.Run(tt.query, func(t *testing.T) {
			got := idx.Query(tt.query, 8, tt.fuzzy, nil, 1)
			if len(got) == 0 || got[0].Name != tt.want {
				t.Fatalf("got %+v, want %s", got, tt.want)
			}
		})
	}
	if got := idx.Query("calclator", 8, false, nil, 1); len(got) != 0 {
		t.Fatal("fuzzy must be optional")
	}
	if got := idx.Query("unrelated", 8, true, nil, 1); len(got) != 0 {
		t.Fatal("unrelated result")
	}
}
func TestRankingAndHistory(t *testing.T) {
	idx := New(apps("Codec", "My Code", "Code", "Chrome"))
	got := idx.Query("code", 8, true, nil, 1)
	if len(got) != 3 || got[0].Name != "Code" || got[1].Name != "Codec" || got[2].Name != "My Code" {
		t.Fatalf("ranking: %+v", got)
	}
	got = idx.Query("c", 1, true, map[string]float64{"3": 200}, 1)
	if len(got) != 1 || got[0].Name != "Chrome" {
		t.Fatal("history not applied")
	}
	if len(idx.Query("", 2, true, nil, 1)) != 2 {
		t.Fatal("limit")
	}
	if len(idx.Query("", 0, true, nil, 1)) != 0 {
		t.Fatal("zero limit")
	}
}
func BenchmarkSearch1000Apps(b *testing.B) {
	items := make([]model.AppItem, 1000)
	for i := range items {
		items[i] = model.AppItem{ID: fmt.Sprint(i), Name: fmt.Sprintf("Application %d Visual Studio Editor", i)}
	}
	idx := New(items)
	for _, query := range []string{"editor", "vs", "edtor", "", "zzzz"} {
		b.Run(query, func(b *testing.B) {
			b.ReportAllocs()
			for n := 0; n < b.N; n++ {
				idx.Query(query, 8, true, nil, 1)
			}
		})
	}
}
