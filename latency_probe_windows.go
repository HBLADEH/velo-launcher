//go:build windows && latencyprobe && !bindings

package main

// This observer is absent from normal builds. It never synthesizes input.
// The keyboard hook retains only an armed Alt+Space event, never typed text.
import (
	"crypto/rand"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"net"
	"net/http"
	"os"
	"path/filepath"
	"runtime"
	"sync"
	"sync/atomic"
	"time"
	"unsafe"

	"golang.org/x/sys/windows"
)

var lpUser32 = windows.NewLazySystemDLL("user32.dll")
var lpKernel32 = windows.NewLazySystemDLL("kernel32.dll")
var latencyObserver *latencyRecorder

type latencyPending struct {
	ID, Kind string
	Start    time.Time
	EventMS  uint32
	Injected bool
}

type latencyDOMState struct {
	Stage           string  `json:"stage"`
	DocumentFocused bool    `json:"documentFocused"`
	InputFocused    bool    `json:"inputFocused"`
	Writable        bool    `json:"writable"`
	Displayed       bool    `json:"displayed"`
	BrowserTimeMS   float64 `json:"browserTimeMS"`
}

type latencyRecord struct {
	ID, Kind                  string
	ProcessID                 int
	ExecutableSHA256          string
	BootID                    string
	StartedAtUTC              string
	AcknowledgedAtUTC         string
	UpperBoundMilliseconds    float64
	Valid                     bool
	Reason                    string
	ClockUncertaintyMS        float64
	KeyboardEventMilliseconds uint32
	InjectedKeyboardEvent     bool
	DOM                       latencyDOMState
	NativeVisible             bool
	NativeActive              bool
	ForegroundProcessID       uint32
}

type latencyRecorder struct {
	mu         sync.Mutex
	dir, token string
	app        *App
	pending    *latencyPending
	records    []latencyRecord
	armed      atomic.Bool
	ready      bool
	lastDOM    latencyDOMState
	armID      string
	tickBound  time.Duration
	mapBound   time.Duration
	hookError  string
}

func init() {
	dir := os.Getenv("VELO_LATENCY_DIR")
	if dir == "" {
		// Only the opt-in probe build reads this direct-GUI-launch sidecar.
		if executable, err := os.Executable(); err == nil {
			if data, err := os.ReadFile(executable + ".probe.json"); err == nil {
				var environment map[string]string
				if json.Unmarshal(data, &environment) == nil {
					for key, value := range environment {
						if key == "LOCALAPPDATA" || key == "VELO_LATENCY_DIR" || key == "VELO_LATENCY_KIND" || key == "VELO_LATENCY_SHA256" || key == "VELO_LATENCY_BOOT_ID" {
							_ = os.Setenv(key, value)
						}
					}
				}
			}
		}
		dir = os.Getenv("VELO_LATENCY_DIR")
	}
	if dir == "" {
		return
	}
	if !filepath.IsAbs(dir) {
		panic("VELO_LATENCY_DIR must be absolute")
	}
	if err := os.MkdirAll(dir, 0700); err != nil {
		panic(err)
	}
	r := &latencyRecorder{dir: dir, records: []latencyRecord{}}
	latencyObserver = r
	var created, exited, kernel, user windows.Filetime
	if err := windows.GetProcessTimes(windows.CurrentProcess(), &created, &exited, &kernel, &user); err != nil {
		panic(err)
	}
	before := time.Now()
	var wall windows.Filetime
	lpKernel32.NewProc("GetSystemTimePreciseAsFileTime").Call(uintptr(unsafe.Pointer(&wall)))
	after := time.Now()
	r.mapBound = after.Sub(before) + 100*time.Microsecond
	// Filetime maps actual OS process creation onto Go's monotonic clock.
	start := before.Add(time.Duration(created.Nanoseconds() - wall.Nanoseconds())).Add(-r.mapBound)
	kind := os.Getenv("VELO_LATENCY_KIND")
	if kind == "" {
		kind = "WarmStartup"
	}
	r.pending = &latencyPending{ID: "startup", Kind: kind, Start: start}
	var adjustment, increment, disabled uint32
	ok, _, _ := lpKernel32.NewProc("GetSystemTimeAdjustment").Call(uintptr(unsafe.Pointer(&adjustment)), uintptr(unsafe.Pointer(&increment)), uintptr(unsafe.Pointer(&disabled)))
	r.tickBound = 17 * time.Millisecond
	if ok != 0 && increment > 0 {
		r.tickBound = time.Duration(increment)*100*time.Nanosecond + time.Millisecond
	}
	secret := make([]byte, 24)
	if _, err := rand.Read(secret); err != nil {
		panic(err)
	}
	r.token = hex.EncodeToString(secret)
	listener, err := net.Listen("tcp4", "127.0.0.1:0")
	if err != nil {
		panic(err)
	}
	info := map[string]any{
		"ProcessId": os.Getpid(), "URL": "http://" + listener.Addr().String(), "Token": r.token,
		"ProcessCreationUTC":  time.Unix(0, created.Nanoseconds()).UTC().Format(time.RFC3339Nano),
		"StartupClockBoundMS": float64(r.mapBound) / float64(time.Millisecond),
		"HotkeyClockBoundMS":  float64(r.tickBound) / float64(time.Millisecond),
	}
	data, _ := json.MarshalIndent(info, "", "  ")
	if err := os.WriteFile(filepath.Join(dir, "control.json"), data, 0600); err != nil {
		panic(err)
	}
	go r.keyboardHook()
	go func() {
		server := &http.Server{Handler: http.HandlerFunc(r.control), ReadHeaderTimeout: 3 * time.Second}
		_ = server.Serve(listener)
	}()
}

func (r *latencyRecorder) keyboardHook() {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	type keyData struct {
		VK, Scan, Flags, Time uint32
		Extra                 uintptr
	}
	callback := windows.NewCallback(func(code int32, message, pointer uintptr) uintptr {
		if code == 0 && (message == 0x100 || message == 0x104) && r.armed.Load() {
			key := *(*keyData)(unsafe.Pointer(pointer))
			if key.VK == 0x20 && key.Flags&0x20 != 0 {
				at := time.Now()
				uptime, _, _ := lpKernel32.NewProc("GetTickCount64").Call()
				start := latencyEventLowerBound(at, uint32(uptime), key.Time, r.tickBound)
				r.mu.Lock()
				if r.armed.Swap(false) {
					r.pending = &latencyPending{ID: r.armID, Kind: "HotkeyActivation", Start: start, EventMS: key.Time, Injected: key.Flags&0x10 != 0}
				}
				r.mu.Unlock()
			}
		}
		next, _, _ := lpUser32.NewProc("CallNextHookEx").Call(0, uintptr(code), message, pointer)
		return next
	})
	module, _, _ := lpKernel32.NewProc("GetModuleHandleW").Call(0)
	hook, _, err := lpUser32.NewProc("SetWindowsHookExW").Call(13, callback, module, 0)
	if hook == 0 {
		r.mu.Lock()
		r.hookError = err.Error()
		r.mu.Unlock()
		return
	}
	defer lpUser32.NewProc("UnhookWindowsHookEx").Call(hook)
	var message struct {
		Window         uintptr
		Message        uint32
		WParam, LParam uintptr
		Time           uint32
		X, Y           int32
		Private        uint32
	}
	for {
		result, _, _ := lpUser32.NewProc("GetMessageW").Call(uintptr(unsafe.Pointer(&message)), 0, 0, 0)
		if int32(result) <= 0 {
			return
		}
	}
}

// Unsigned subtraction handles the 32-bit event-clock rollover. Subtracting
// the clock quantum makes the result a lower bound on the event's real time.
func latencyEventLowerBound(at time.Time, nowMS, eventMS uint32, quantum time.Duration) time.Time {
	return at.Add(-time.Duration(nowMS-eventMS)*time.Millisecond - quantum)
}

// LatencyProbeMark is bound only in a latencyprobe build, without changing the
// normal App or generated bindings. Readiness is reported by the actual page.
func (a *App) LatencyProbeMark(state latencyDOMState) bool {
	r := latencyObserver
	if r == nil {
		return false
	}
	ack := time.Now()
	visible, active, foregroundPID := latencyNativeState()
	r.mu.Lock()
	r.app = a
	if state.Stage != "attach" {
		r.lastDOM = state
	}
	if state.Stage != "ready" {
		r.ready = false
		r.mu.Unlock()
		return true
	}
	r.ready = state.DocumentFocused && state.InputFocused && state.Writable && state.Displayed
	if !r.ready || r.pending == nil {
		r.mu.Unlock()
		return true
	}
	pending := r.pending
	bound := r.mapBound
	if pending.Kind == "HotkeyActivation" {
		bound = r.tickBound
	}
	record := latencyRecord{
		ID: pending.ID, Kind: pending.Kind, ProcessID: os.Getpid(),
		ExecutableSHA256: os.Getenv("VELO_LATENCY_SHA256"), BootID: os.Getenv("VELO_LATENCY_BOOT_ID"),
		StartedAtUTC: pending.Start.UTC().Format(time.RFC3339Nano), AcknowledgedAtUTC: ack.UTC().Format(time.RFC3339Nano),
		UpperBoundMilliseconds: float64(ack.Sub(pending.Start)) / float64(time.Millisecond),
		Valid:                  visible && active, DOM: state, NativeVisible: visible, NativeActive: active,
		ForegroundProcessID: foregroundPID,
		ClockUncertaintyMS:  float64(bound) / float64(time.Millisecond), KeyboardEventMilliseconds: pending.EventMS, InjectedKeyboardEvent: pending.Injected,
	}
	if !record.Valid {
		record.Reason = "Input-ready DOM report arrived without an active visible native window."
	}
	r.pending = nil
	r.appendLocked(record)
	r.mu.Unlock()
	return true
}

func latencyNativeState() (bool, bool, uint32) {
	class, _ := windows.UTF16PtrFromString("VeloLauncherWindow")
	window, _, _ := lpUser32.NewProc("FindWindowW").Call(uintptr(unsafe.Pointer(class)), 0)
	var owner, foregroundPID uint32
	lpUser32.NewProc("GetWindowThreadProcessId").Call(window, uintptr(unsafe.Pointer(&owner)))
	foreground, _, _ := lpUser32.NewProc("GetForegroundWindow").Call()
	lpUser32.NewProc("GetWindowThreadProcessId").Call(foreground, uintptr(unsafe.Pointer(&foregroundPID)))
	visible, _, _ := lpUser32.NewProc("IsWindowVisible").Call(window)
	return window != 0 && owner == uint32(os.Getpid()) && visible != 0, foregroundPID == uint32(os.Getpid()), foregroundPID
}

func latencyNativeFocus() map[string]any {
	class, _ := windows.UTF16PtrFromString("VeloLauncherWindow")
	window, _, _ := lpUser32.NewProc("FindWindowW").Call(uintptr(unsafe.Pointer(class)), 0)
	var owner uint32
	thread, _, _ := lpUser32.NewProc("GetWindowThreadProcessId").Call(window, uintptr(unsafe.Pointer(&owner)))
	if window == 0 || owner != uint32(os.Getpid()) {
		return map[string]any{"Available": false}
	}
	var info struct {
		Size, Flags                                   uint32
		Active, Focus, Capture, Menu, MoveSize, Caret uintptr
		CaretRect                                     [4]int32
	}
	info.Size = uint32(unsafe.Sizeof(info))
	ok, _, _ := lpUser32.NewProc("GetGUIThreadInfo").Call(thread, uintptr(unsafe.Pointer(&info)))
	var focusedClass [256]uint16
	lpUser32.NewProc("GetClassNameW").Call(info.Focus, uintptr(unsafe.Pointer(&focusedClass[0])), 256)
	return map[string]any{"Available": ok != 0, "Window": window, "FocusedWindow": info.Focus, "FocusedClass": windows.UTF16ToString(focusedClass[:])}
}

func (r *latencyRecorder) appendLocked(record latencyRecord) {
	r.records = append(r.records, record)
	file, err := os.OpenFile(filepath.Join(r.dir, "events.jsonl"), os.O_CREATE|os.O_APPEND|os.O_WRONLY, 0600)
	if err != nil {
		return
	}
	defer file.Close()
	_ = json.NewEncoder(file).Encode(record)
}

func (r *latencyRecorder) control(writer http.ResponseWriter, request *http.Request) {
	if request.Header.Get("Authorization") != "Bearer "+r.token {
		http.Error(writer, "unauthorized", http.StatusUnauthorized)
		return
	}
	writer.Header().Set("Content-Type", "application/json")
	r.mu.Lock()
	a := r.app
	r.mu.Unlock()
	switch request.URL.Path {
	case "/state":
		visible, active, foregroundPID := latencyNativeState()
		r.mu.Lock()
		defer r.mu.Unlock()
		state := map[string]any{"ProcessId": os.Getpid(), "Ready": r.ready, "HookError": r.hookError, "Armed": r.armed.Load(), "Records": r.records, "Attached": a != nil,
			"LastDOM": r.lastDOM, "NativeVisible": visible, "NativeActive": active, "ForegroundProcessID": foregroundPID, "NativeFocus": latencyNativeFocus()}
		if a != nil {
			state["Status"] = a.GetStatus()
		}
		_ = json.NewEncoder(writer).Encode(state)
	case "/prepare":
		if request.Method != http.MethodPost || a == nil {
			http.Error(writer, "not ready", http.StatusConflict)
			return
		}
		if a.service.Settings().Hotkey != "Alt+Space" {
			http.Error(writer, "probe currently supports the configured Alt+Space chord only", http.StatusConflict)
			return
		}
		a.Hide()
		r.mu.Lock()
		defer r.mu.Unlock()
		if r.pending != nil || r.armed.Load() || r.hookError != "" {
			http.Error(writer, "an observation is pending, or the keyboard hook failed", http.StatusConflict)
			return
		}
		r.armID = request.URL.Query().Get("id")
		if r.armID == "" {
			http.Error(writer, "id required", http.StatusBadRequest)
			return
		}
		r.ready = false
		r.armed.Store(true)
		_ = json.NewEncoder(writer).Encode(map[string]any{"Armed": true, "ID": r.armID})
	case "/expire":
		if request.Method != http.MethodPost {
			http.Error(writer, "POST required", http.StatusMethodNotAllowed)
			return
		}
		r.mu.Lock()
		defer r.mu.Unlock()
		id := r.armID
		kind := "HotkeyActivation"
		if r.pending != nil {
			id, kind = r.pending.ID, r.pending.Kind
		}
		r.appendLocked(latencyRecord{ID: id, Kind: kind, ProcessID: os.Getpid(), ExecutableSHA256: os.Getenv("VELO_LATENCY_SHA256"), BootID: os.Getenv("VELO_LATENCY_BOOT_ID"), Valid: false, Reason: "Timed out waiting for a confirmed input-ready event.", AcknowledgedAtUTC: time.Now().UTC().Format(time.RFC3339Nano)})
		r.pending = nil
		r.armed.Store(false)
		_ = json.NewEncoder(writer).Encode(map[string]any{"Expired": id})
	case "/quit":
		if request.Method != http.MethodPost || a == nil {
			http.Error(writer, "not ready", http.StatusConflict)
			return
		}
		_ = json.NewEncoder(writer).Encode(map[string]any{"Quitting": true})
		go func() { time.Sleep(50 * time.Millisecond); a.Quit() }()
	default:
		http.Error(writer, fmt.Sprintf("unknown probe route %q", request.URL.Path), http.StatusNotFound)
	}
}
