package platform

import (
	"testing"
	"time"
)

func TestHotkeyConflictAndRebind(t *testing.T) {
	called := make(chan struct{}, 1)
	h, err := RegisterHotkey("Ctrl+Alt+Shift+F24", func() { called <- struct{}{} })
	if err != nil {
		t.Fatal(err)
	}
	defer h.Close()
	other, err := RegisterHotkey("Ctrl+Alt+Shift+F23", func() {})
	if err != nil {
		t.Fatal(err)
	}
	defer other.Close()
	if err := h.Rebind("Ctrl+Alt+Shift+F23"); err == nil {
		t.Fatal("conflict accepted")
	}
	postThreadMessage.Call(uintptr(h.thread), 0x0312, 1, 0)
	select {
	case <-called:
	case <-time.After(time.Second):
		t.Fatal("hotkey message not dispatched after conflict")
	}
	if err := h.Rebind("Ctrl+Alt+Shift+F22"); err != nil {
		t.Fatal(err)
	}
}
