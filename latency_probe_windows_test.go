//go:build windows && latencyprobe && !bindings

package main

import (
	"testing"
	"time"
)

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
