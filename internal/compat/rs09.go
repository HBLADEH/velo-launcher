package compat

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"reflect"
	"time"

	"velo-launcher/internal/config"
	"velo-launcher/internal/history"
	"velo-launcher/internal/model"
	"velo-launcher/internal/search"
	"velo-launcher/internal/storage"
)

type NormalizationCase struct {
	Name     string `json:"name"`
	Input    string `json:"input"`
	Expected string `json:"expected"`
}
type QueryCase struct {
	Query    string             `json:"query"`
	Limit    int                `json:"limit"`
	Fuzzy    bool               `json:"fuzzy"`
	Weights  map[string]float64 `json:"weights"`
	Weight   float64            `json:"weight"`
	Expected []search.Result    `json:"expected"`
}
type SearchCase struct {
	Name    string          `json:"name"`
	Apps    []model.AppItem `json:"apps"`
	Queries []QueryCase     `json:"queries"`
}
type RecordCase struct {
	ID      string `json:"id"`
	Query   string `json:"query"`
	Time    string `json:"time"`
	Success bool   `json:"success"`
}
type HistoryCase struct {
	Name         string                        `json:"name"`
	Initial      map[string]history.Entry      `json:"initial"`
	Records      []RecordCase                  `json:"records"`
	ScoreQueries []string                      `json:"score_queries"`
	Expected     map[string]history.Entry      `json:"expected"`
	Scores       map[string]map[string]float64 `json:"scores"`
}
type PinCase struct {
	ID     string `json:"id"`
	Pinned bool   `json:"pinned"`
	Error  bool   `json:"error"`
}
type HomeResult struct {
	Pinned   []model.AppItem `json:"pinned"`
	Frequent []model.AppItem `json:"frequent"`
	Tools    []model.AppItem `json:"tools"`
}
type ServiceState struct {
	Library  Library           `json:"library"`
	Home     HomeResult        `json:"home"`
	Searches [][]search.Result `json:"searches"`
}
type ServiceCase struct {
	Name     string                   `json:"name"`
	Apps     []model.AppItem          `json:"apps"`
	Tools    []model.AppItem          `json:"tools"`
	Library  Library                  `json:"library"`
	History  map[string]history.Entry `json:"history"`
	Config   config.Config            `json:"config"`
	Queries  []string                 `json:"queries"`
	Pins     []PinCase                `json:"pins"`
	Expected []ServiceState           `json:"expected"`
}
type BehaviorFixture struct {
	SchemaVersion  int                 `json:"schema_version"`
	FixedTime      string              `json:"fixed_time"`
	Normalizations []NormalizationCase `json:"normalizations"`
	Searches       []SearchCase        `json:"searches"`
	Histories      []HistoryCase       `json:"histories"`
	Services       []ServiceCase       `json:"services"`
}

func LoadBehavior(path string) (BehaviorFixture, error) {
	var f BehaviorFixture
	err := storage.Read(path, &f)
	if err == nil && f.SchemaVersion != 1 {
		err = fmt.Errorf("unsupported behavior fixture")
	}
	return f, err
}

func CheckBehavior(f *BehaviorFixture, refresh bool) error {
	now, err := time.Parse(time.RFC3339Nano, f.FixedTime)
	if err != nil {
		return err
	}
	check := func(actual, expected any, name string) error {
		a, _ := json.Marshal(actual)
		b, _ := json.Marshal(expected)
		if !reflect.DeepEqual(a, b) {
			return fmt.Errorf("behavior mismatch: %s", name)
		}
		return nil
	}
	for n := range f.Normalizations {
		c := &f.Normalizations[n]
		got := search.Normalize(c.Input)
		if refresh {
			c.Expected = got
		} else if got != c.Expected {
			return fmt.Errorf("normalization: %s", c.Name)
		}
	}
	for n := range f.Searches {
		c := &f.Searches[n]
		index := search.New(c.Apps)
		for q := range c.Queries {
			query := &c.Queries[q]
			got := index.Query(query.Query, query.Limit, query.Fuzzy, query.Weights, query.Weight)
			if refresh {
				query.Expected = got
			} else if err := check(got, query.Expected, c.Name); err != nil {
				return err
			}
		}
	}
	for n := range f.Histories {
		c := &f.Histories[n]
		dir, err := os.MkdirTemp("", "velo-rs09-history-")
		if err != nil {
			return err
		}
		err = func() error {
			defer os.RemoveAll(dir)
			path := filepath.Join(dir, "history.json")
			if err := storage.Write(path, c.Initial); err != nil {
				return err
			}
			store, err := history.Open(path)
			if err != nil {
				return err
			}
			for _, record := range c.Records {
				at, err := time.Parse(time.RFC3339Nano, record.Time)
				if err != nil {
					return err
				}
				if record.Success {
					if err := store.Record(record.ID, search.Normalize(record.Query), at); err != nil {
						return err
					}
				}
			}
			var got map[string]history.Entry
			if err := storage.Read(path, &got); err != nil {
				return err
			}
			if refresh {
				c.Expected = got
			} else if err := check(got, c.Expected, c.Name); err != nil {
				return err
			}
			scores := map[string]map[string]float64{}
			for _, query := range c.ScoreQueries {
				scores[query] = store.Scores(search.Normalize(query), now)
			}
			if refresh {
				c.Scores = scores
			} else {
				return check(scores, c.Scores, c.Name)
			}
			return nil
		}()
		if err != nil {
			return err
		}
	}
	return nil
}
