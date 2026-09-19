package platform

import (
	"fmt"
	"golang.org/x/sys/windows"
	"os"
	"slices"
	"unsafe"
)

const WindowClass = "VeloLauncherWindow"

type DesktopWindow struct{ handle uintptr }
type rect struct{ Left, Top, Right, Bottom int32 }
type monitorInfo struct {
	Size          uint32
	Monitor, Work rect
	Flags         uint32
}

func FindWindow() (*DesktopWindow, error) {
	class, _ := windows.UTF16PtrFromString(WindowClass)
	h, _, _ := user32.NewProc("FindWindowW").Call(uintptr(unsafe.Pointer(class)), 0)
	if h == 0 {
		return nil, fmt.Errorf("找不到 Velo 窗口")
	}
	var pid uint32
	user32.NewProc("GetWindowThreadProcessId").Call(h, uintptr(unsafe.Pointer(&pid)))
	if pid != windows.GetCurrentProcessId() {
		return nil, fmt.Errorf("Velo 已在运行")
	}
	if !slices.Contains(os.Args, "--diagnostics") {
		style, _, _ := user32.NewProc("GetWindowLongPtrW").Call(h, ^uintptr(19))
		user32.NewProc("SetWindowLongPtrW").Call(h, ^uintptr(19), (style|0x80)&^0x40000)
		user32.NewProc("SetWindowPos").Call(h, 0, 0, 0, 0, 0, 0x0027)
	}
	return &DesktopWindow{h}, nil
}
func (w *DesktopWindow) Visible() bool {
	v, _, _ := user32.NewProc("IsWindowVisible").Call(w.handle)
	return v != 0
}
func (w *DesktopWindow) Active() bool {
	foreground, _, _ := user32.NewProc("GetForegroundWindow").Call()
	owner, _, _ := user32.NewProc("GetAncestor").Call(foreground, 3)
	return foreground == w.handle || owner == w.handle
}
func (w *DesktopWindow) Hide() { user32.NewProc("ShowWindow").Call(w.handle, 0) }
func (w *DesktopWindow) Show() {
	// Use the monitor under the pointer and its work area, including taskbar
	// exclusions and negative coordinates on secondary monitors.
	var point struct{ X, Y int32 }
	user32.NewProc("GetCursorPos").Call(uintptr(unsafe.Pointer(&point)))
	packed := uint64(uint32(point.X)) | uint64(uint32(point.Y))<<32
	monitor, _, _ := user32.NewProc("MonitorFromPoint").Call(uintptr(packed), 2)
	info := monitorInfo{Size: uint32(unsafe.Sizeof(monitorInfo{}))}
	user32.NewProc("GetMonitorInfoW").Call(monitor, uintptr(unsafe.Pointer(&info)))
	var bounds rect
	user32.NewProc("GetWindowRect").Call(w.handle, uintptr(unsafe.Pointer(&bounds)))
	x := info.Work.Left + (info.Work.Right-info.Work.Left-(bounds.Right-bounds.Left))/2
	y := info.Work.Top + (info.Work.Bottom-info.Work.Top)/5
	user32.NewProc("SetWindowPos").Call(w.handle, ^uintptr(0), uintptr(x), uintptr(y), 0, 0, 0x0041)
	user32.NewProc("ShowWindow").Call(w.handle, 9)
	user32.NewProc("SetForegroundWindow").Call(w.handle)
}
