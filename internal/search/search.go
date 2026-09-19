// Package search ranks a pre-normalized memory-only snapshot.
package search

import (
	"sort"
	"strings"
	"unicode"
	"unicode/utf8"
	"velo-launcher/internal/model"
)

type entry struct {
	item  model.AppItem
	name  string
	terms []string
}
type Index struct{ entries []entry }
type Result struct {
	model.AppItem
	Score float64 `json:"score"`
}

func Normalize(s string) string {
	return strings.Join(strings.Fields(strings.ToLower(strings.TrimSpace(s))), " ")
}

func New(items []model.AppItem) *Index {
	i := &Index{entries: make([]entry, 0, len(items))}
	for _, app := range items {
		name := Normalize(app.Name)
		terms := strings.FieldsFunc(name, func(r rune) bool { return !unicode.IsLetter(r) && !unicode.IsDigit(r) })
		var initials strings.Builder
		for _, word := range terms {
			for _, r := range word {
				initials.WriteRune(r)
				break
			}
		}
		terms = append(terms, initials.String())
		for _, k := range app.Keywords {
			terms = append(terms, Normalize(k))
		}
		// Common local aliases; broader user aliases can live in Keywords later.
		if strings.Contains(name, "wechat") || strings.Contains(name, "微信") {
			terms = append(terms, "wx", "weixin", "微信")
		}
		i.entries = append(i.entries, entry{app, name, terms})
	}
	return i
}
func (i *Index) Query(query string, limit int, fuzzy bool, weights map[string]float64, weight float64) []Result {
	query = Normalize(query)
	if limit < 1 {
		return []Result{}
	}
	type ranked struct {
		entry *entry
		score float64
	}
	better := func(a, b ranked) bool {
		if a.score != b.score {
			return a.score > b.score
		}
		if a.entry.name != b.entry.name {
			return a.entry.name < b.entry.name
		}
		return a.entry.item.ID < b.entry.item.ID
	}
	top := make([]ranked, 0, limit)
	for n := range i.entries {
		e := &i.entries[n]
		score := match(query, e.name, e.terms, fuzzy)
		if score < 0 {
			continue
		}
		candidate := ranked{e, float64(score) + weights[e.item.ID]*weight}
		position := sort.Search(len(top), func(n int) bool { return better(candidate, top[n]) })
		if position >= limit {
			continue
		}
		if len(top) < limit {
			top = append(top, ranked{})
		}
		copy(top[position+1:], top[position:len(top)-1])
		top[position] = candidate
	}
	out := make([]Result, len(top))
	for n, candidate := range top {
		out[n] = Result{candidate.entry.item, candidate.score}
	}
	return out
}
func match(q, name string, terms []string, fuzzy bool) int {
	if q == "" {
		return 0
	}
	if q == name {
		return 1000
	}
	if strings.HasPrefix(name, q) {
		return 800
	}
	for _, t := range terms {
		if strings.HasPrefix(t, q) {
			return 650
		}
	}
	if strings.Contains(name, q) {
		return 500
	}
	if !fuzzy {
		return -1
	}
	// Multi-word queries can match tokens in any order.
	if strings.Contains(q, " ") {
		parts := strings.Fields(q)
		all := true
		for _, p := range parts {
			if !strings.Contains(name, p) {
				all = false
				break
			}
		}
		if all {
			return 450
		}
	}
	qr := []rune(q)
	if len(qr) < 2 {
		return -1
	}
	j := 0
	gaps := 0
	for _, r := range name {
		if j < len(qr) && r == qr[j] {
			j++
		} else if j > 0 && j < len(qr) {
			gaps++
		}
	}
	if j == len(qr) {
		return 350 - min(gaps, 100)
	}
	if len(qr) >= 3 {
		for _, t := range terms {
			length := utf8.RuneCountInString(t)
			if length >= len(qr)-1 && length <= len(qr)+1 && oneEdit(qr, []rune(t)) {
				return 250
			}
		}
		length := utf8.RuneCountInString(name)
		if length >= len(qr)-1 && length <= len(qr)+1 && oneEdit(qr, []rune(name)) {
			return 250
		}
	}
	return -1
}

// oneEdit accepts one insertion, deletion, substitution or adjacent transposition.
func oneEdit(a, b []rune) bool {
	if len(a)-len(b) > 1 || len(b)-len(a) > 1 {
		return false
	}
	i, j, edits := 0, 0, 0
	for i < len(a) && j < len(b) {
		if a[i] == b[j] {
			i++
			j++
			continue
		}
		edits++
		if edits > 1 {
			return false
		}
		if len(a) == len(b) {
			if i+1 < len(a) && a[i] == b[j+1] && a[i+1] == b[j] {
				i += 2
				j += 2
			} else {
				i++
				j++
			}
		} else if len(a) > len(b) {
			i++
		} else {
			j++
		}
	}
	if i < len(a) || j < len(b) {
		edits++
	}
	return edits <= 1
}
