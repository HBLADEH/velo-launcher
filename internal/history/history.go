package history

import (
	"math"
	"os"
	"sync"
	"time"
	"velo-launcher/internal/storage"
)

type Entry struct {
	Count   int            `json:"launch_count"`
	Last    time.Time      `json:"last_launch_time"`
	Queries map[string]int `json:"query_history"`
}
type Store struct {
	mu      sync.RWMutex
	path    string
	entries map[string]Entry
}

func Open(path string) (*Store, error) {
	s := &Store{path: path, entries: map[string]Entry{}}
	err := storage.Read(path, &s.entries)
	if os.IsNotExist(err) {
		err = nil
	}
	if s.entries == nil {
		s.entries = map[string]Entry{}
	}
	return s, err
}
func (s *Store) Record(id, query string, now time.Time) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	e := s.entries[id]
	// Copy the map so persistence failure cannot mutate the old state.
	queries := make(map[string]int, len(e.Queries)+1)
	for q, n := range e.Queries {
		queries[q] = n
	}
	if query != "" {
		queries[query]++
	}
	// Bound query history per application without running background cleanup.
	if len(queries) > 100 {
		var key string
		min := int(^uint(0) >> 1)
		for q, n := range queries {
			if q != query && n < min {
				key = q
				min = n
			}
		}
		delete(queries, key)
	}
	previous, existed := s.entries[id]
	s.entries[id] = Entry{Count: e.Count + 1, Last: now, Queries: queries}
	if err := storage.Write(s.path, s.entries); err != nil {
		if existed {
			s.entries[id] = previous
		} else {
			delete(s.entries, id)
		}
		return err
	}
	return nil
}
func (s *Store) Scores(query string, now time.Time) map[string]float64 {
	s.mu.RLock()
	defer s.mu.RUnlock()
	result := make(map[string]float64, len(s.entries))
	for id, e := range s.entries {
		age := math.Max(0, now.Sub(e.Last).Hours()/24)
		result[id] = math.Min(100, math.Log2(float64(e.Count)+1)*15) + 35/(1+age) + math.Min(220, float64(e.Queries[query])*35)
	}
	return result
}
