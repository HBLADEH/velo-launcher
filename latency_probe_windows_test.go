//go:build windows && latencyprobe && !bindings

package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func TestActualInputMarkerPreservesLegacyReadinessObservation(t *testing.T) {
	previous := latencyObserver
	t.Cleanup(func() { latencyObserver = previous })
	r := &latencyRecorder{dir: t.TempDir(), pending: &latencyPending{ID: "legacy-ready", Kind: "HotkeyActivation"}}
	latencyObserver = r
	t.Setenv("VELO_LATENCY_ACTUAL_INPUT", "1")
	app := &App{}
	if !app.LatencyProbeMark(latencyDOMState{Stage: "input", InputTrusted: true, UTF8Bytes: 1}) {
		t.Fatal("actual-input observation was not written")
	}
	if r.pending == nil || r.pending.ID != "legacy-ready" || len(r.records) != 0 {
		t.Fatal("input markers must not consume the older focus-ready measurement")
	}
	data, err := os.ReadFile(filepath.Join(r.dir, "accepted-input.log"))
	if err != nil || !strings.Contains(string(data), "event=input_accepted") || !strings.Contains(string(data), "utf8_bytes=1") {
		t.Fatalf("actual character marker missing: %v", err)
	}
	t.Setenv("VELO_LATENCY_ACTUAL_INPUT", "")
	if !app.LatencyProbeMark(latencyDOMState{Stage: "input", InputTrusted: true, UTF8Bytes: 2}) {
		t.Fatal("disabled actual-input stage must remain harmless")
	}
	after, err := os.ReadFile(filepath.Join(r.dir, "accepted-input.log"))
	if err != nil || string(after) != string(data) {
		t.Fatal("legacy probe should not write actual-input markers unless opted in")
	}
}

func TestLatencyEventLowerBound(t *testing.T) {
	at := time.Now()
	for _, test := range []struct {
		now, event uint32
		elapsed    time.Duration
	}{
		{100, 100, 17 * time.Millisecond},
		{120, 100, 37 * time.Millisecond},
		{5, ^uint32(0) - 4, 27 * time.Millisecond},
	} {
		start := latencyEventLowerBound(at, test.now, test.event, 17*time.Millisecond)
		if got := at.Sub(start); got != test.elapsed {
			t.Fatalf("clock delta: got %v, want %v", got, test.elapsed)
		}
	}
}
