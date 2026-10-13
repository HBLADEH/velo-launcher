package platform

import (
	"errors"
	"os"
	"testing"
	"unsafe"
)

func TestEmptyRecycleBin(t *testing.T) {
	wantSize := uint32(20)
	if unsafe.Sizeof(uintptr(0)) == 8 {
		wantSize = 24
	}
	for _, tt := range []struct {
		name       string
		size       int64
		items      int64
		queryHR    uintptr
		emptyHR    uintptr
		wantEmpty  bool
		wantNotice bool
		wantError  string
	}{
		{name: "already empty", emptyHR: 0x8000FFFF, wantNotice: true},
		{name: "contains items", size: 1024, items: 1, wantEmpty: true},
		{name: "zero-byte file", items: 1, wantEmpty: true},
		{name: "64-bit item count", items: 1 << 32, wantEmpty: true},
		{name: "cancelled", items: 1, emptyHR: 0x800704C7, wantEmpty: true},
		{name: "query failed", queryHR: 0x80004005, wantEmpty: true},
		{name: "empty failed", items: 1, emptyHR: 0x80070005, wantEmpty: true, wantError: "清空回收站失败: 0x80070005"},
		{name: "query and empty failed", queryHR: 0x80004005, emptyHR: 0x80004005, wantEmpty: true, wantError: "清空回收站失败: 0x80004005"},
	} {
		t.Run(tt.name, func(t *testing.T) {
			emptyCalled := false
			noticeCalled := false
			err := emptyRecycleBin(func(info *recycleBinInfo) uintptr {
				if info.cbSize != wantSize {
					t.Fatalf("SHQUERYRBINFO size = %d, want %d", info.cbSize, wantSize)
				}
				info.size = tt.size
				info.items = tt.items
				return tt.queryHR
			}, func() uintptr {
				emptyCalled = true
				return tt.emptyHR
			}, func() error {
				noticeCalled = true
				return nil
			})
			if emptyCalled != tt.wantEmpty {
				t.Fatalf("empty called = %t, want %t", emptyCalled, tt.wantEmpty)
			}
			if noticeCalled != tt.wantNotice {
				t.Fatalf("empty-bin notice called = %t, want %t", noticeCalled, tt.wantNotice)
			}
			if tt.wantError == "" {
				if err != nil {
					t.Fatal(err)
				}
			} else if err == nil || err.Error() != tt.wantError {
				t.Fatalf("error = %v, want %q", err, tt.wantError)
			}
		})
	}
}

func TestEmptyRecycleBinUnexpectedResult(t *testing.T) {
	const unexpected = uintptr(0x8000FFFF)
	for _, tt := range []struct {
		name      string
		initialHR uintptr
		remaining int64
		finalHR   uintptr
		wantError bool
	}{
		{name: "became empty while confirming"},
		{name: "initial query failed but bin is now empty", initialHR: 0x80004005},
		{name: "files remain", remaining: 1, wantError: true},
		{name: "64-bit item count remains", remaining: 1 << 32, wantError: true},
		{name: "cannot confirm empty", finalHR: 0x80004005, wantError: true},
	} {
		t.Run(tt.name, func(t *testing.T) {
			queries := 0
			emptyCalled := false
			err := emptyRecycleBin(func(info *recycleBinInfo) uintptr {
				queries++
				if !emptyCalled {
					info.items = 1
					return tt.initialHR
				}
				info.items = tt.remaining
				return tt.finalHR
			}, func() uintptr {
				emptyCalled = true
				return unexpected
			}, func() error {
				t.Fatal("the Shell's confirmation must not be followed by an empty-bin notice")
				return nil
			})
			if tt.wantError {
				if err == nil || err.Error() != "清空回收站失败: 0x8000FFFF" {
					t.Fatalf("error = %v, want the original Shell error", err)
				}
			} else if err != nil {
				t.Fatalf("empty recycle bin reported an error: %v", err)
			}
			if !emptyCalled || queries != 2 {
				t.Fatalf("empty called = %t, queries = %d; want an empty attempt and a fresh query", emptyCalled, queries)
			}
		})
	}
}

func TestEmptyRecycleBinNoticeFailure(t *testing.T) {
	want := errors.New("notice unavailable")
	err := emptyRecycleBin(func(*recycleBinInfo) uintptr { return 0 }, func() uintptr {
		t.Fatal("an empty bin must not invoke the empty operation")
		return 0
	}, func() error { return want })
	if !errors.Is(err, want) {
		t.Fatalf("error = %v, want %v", err, want)
	}
}

// Query the real Shell through the production binding, but never empty the bin.
func TestEmptyRecycleBinNativeQuery(t *testing.T) {
	if os.Getenv("VELO_INTEGRATION") != "1" {
		t.Skip("set VELO_INTEGRATION=1 for the read-only native recycle bin check")
	}
	notified := false
	err := emptyRecycleBin(func(info *recycleBinInfo) uintptr {
		hr := queryRecycleBin(info)
		if int32(hr) < 0 {
			t.Fatalf("native query failed: HRESULT=0x%08X, cbSize=%d", uint32(hr), info.cbSize)
		}
		t.Logf("native query: cbSize=%d, items=%d, bytes=%d", info.cbSize, info.items, info.size)
		if info.items != 0 {
			t.Skip("native empty-bin check requires an already empty recycle bin")
		}
		return hr
	}, func() uintptr {
		t.Fatal("an already empty bin reached the empty operation")
		return 0x8000FFFF
	}, func() error {
		notified = true
		return nil
	})
	if err != nil {
		t.Fatal(err)
	}
	if !notified {
		t.Fatal("an already empty bin did not show a notice")
	}
}
