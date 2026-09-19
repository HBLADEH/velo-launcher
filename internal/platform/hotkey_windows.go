package platform

import (
	"fmt"
	"golang.org/x/sys/windows"
	"runtime"
	"sync"
	"unsafe"
	"velo-launcher/internal/hotkey"
)

var user32 = windows.NewLazySystemDLL("user32.dll")
var registerHotKey = user32.NewProc("RegisterHotKey")
var unregisterHotKey = user32.NewProc("UnregisterHotKey")
var getMessage = user32.NewProc("GetMessageW")
var peekMessage = user32.NewProc("PeekMessageW")
var postThreadMessage = user32.NewProc("PostThreadMessageW")

type message struct {
	Window  uintptr
	Message uint32
	WParam  uintptr
	LParam  uintptr
	Time    uint32
	X, Y    int32
	Private uint32
}
type keyRequest struct {
	key    hotkey.Key
	stop   bool
	result chan error
}
type Hotkey struct {
	mu       sync.Mutex
	thread   uint32
	commands chan keyRequest
	done     chan struct{}
}

func RegisterHotkey(text string, callback func()) (*Hotkey, error) {
	key, err := hotkey.Parse(text)
	if err != nil {
		return nil, err
	}
	h := &Hotkey{commands: make(chan keyRequest, 1), done: make(chan struct{})}
	ready := make(chan error, 1)
	go func() {
		runtime.LockOSThread()
		defer runtime.UnlockOSThread()
		defer close(h.done)
		h.thread = windows.GetCurrentThreadId()
		var msg message
		peekMessage.Call(uintptr(unsafe.Pointer(&msg)), 0, 0, 0, 0)
		id := uintptr(1)
		ok, _, err := registerHotKey.Call(0, id, uintptr(key.Modifiers|0x4000), uintptr(key.Code))
		if ok == 0 {
			ready <- fmt.Errorf("快捷键被占用或不可用: %w", err)
			return
		}
		defer func() { unregisterHotKey.Call(0, id) }()
		ready <- nil
		for {
			ok, _, _ := getMessage.Call(uintptr(unsafe.Pointer(&msg)), 0, 0, 0)
			if int32(ok) <= 0 {
				return
			}
			switch msg.Message {
			case 0x0312:
				go callback()
			case 0x8001:
				request := <-h.commands
				if request.stop {
					request.result <- nil
					return
				}
				if request.key == key {
					request.result <- nil
					continue
				}
				next := uintptr(3) - id
				ok, _, err := registerHotKey.Call(0, next, uintptr(request.key.Modifiers|0x4000), uintptr(request.key.Code))
				if ok == 0 {
					request.result <- fmt.Errorf("快捷键被占用或不可用: %w", err)
					continue
				}
				unregisterHotKey.Call(0, id)
				id = next
				key = request.key
				request.result <- nil
			}
		}
	}()
	if err := <-ready; err != nil {
		return nil, err
	}
	return h, nil
}
func (h *Hotkey) request(request keyRequest) error {
	h.mu.Lock()
	defer h.mu.Unlock()
	select {
	case <-h.done:
		return fmt.Errorf("快捷键服务已停止")
	default:
	}
	request.result = make(chan error, 1)
	h.commands <- request
	ok, _, err := postThreadMessage.Call(uintptr(h.thread), 0x8001, 0, 0)
	if ok == 0 {
		<-h.commands
		return err
	}
	select {
	case err := <-request.result:
		return err
	case <-h.done:
		if request.stop {
			return nil
		}
		return fmt.Errorf("快捷键服务已停止")
	}
}
func (h *Hotkey) Rebind(text string) error {
	key, err := hotkey.Parse(text)
	if err != nil {
		return err
	}
	return h.request(keyRequest{key: key})
}
func (h *Hotkey) Close() error { err := h.request(keyRequest{stop: true}); <-h.done; return err }
