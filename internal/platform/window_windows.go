package platform

import (
	"fmt"
	"golang.org/x/sys/windows"
	"os"
	"slices"
	"unsafe"
)

const WindowClass = "VeloLauncherWindow"

type DesktopWindow struct {
	handle     uintptr
	positioned bool
}
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
	return &DesktopWindow{handle: h}, nil
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

// Compare the foreground window with its own monitor, not the primary display.
// Maximized windows with a normal caption and the desktop are not fullscreen.
func ForegroundFullscreen() bool {
	h, _, _ := user32.NewProc("GetForegroundWindow").Call()
	if h == 0 {
		return false
	}
	var class [256]uint16
	user32.NewProc("GetClassNameW").Call(h, uintptr(unsafe.Pointer(&class[0])), 256)
	switch windows.UTF16ToString(class[:]) {
	case "Progman", "WorkerW", "Shell_TrayWnd", "Shell_SecondaryTrayWnd", WindowClass:
		return false
	}
	style, _, _ := user32.NewProc("GetWindowLongPtrW").Call(h, ^uintptr(15))
	if style&0x00C00000 == 0x00C00000 {
		return false
	}
	var bounds rect
	ok, _, _ := user32.NewProc("GetWindowRect").Call(h, uintptr(unsafe.Pointer(&bounds)))
	if ok == 0 {
		return false
	}
	monitor, _, _ := user32.NewProc("MonitorFromWindow").Call(h, 2)
	info := monitorInfo{Size: uint32(unsafe.Sizeof(monitorInfo{}))}
	ok, _, _ = user32.NewProc("GetMonitorInfoW").Call(monitor, uintptr(unsafe.Pointer(&info)))
	return ok != 0 && coversMonitor(bounds, info.Monitor)
}

func coversMonitor(bounds, monitor rect) bool {
	return monitor.Right > monitor.Left && monitor.Bottom > monitor.Top &&
		bounds.Left <= monitor.Left && bounds.Top <= monitor.Top && bounds.Right >= monitor.Right && bounds.Bottom >= monitor.Bottom
}
func (w *DesktopWindow) Resize(width, height int) {
	monitor, _, _ := user32.NewProc("MonitorFromWindow").Call(w.handle, 2)
	info := monitorInfo{Size: uint32(unsafe.Sizeof(monitorInfo{}))}
	ok, _, _ := user32.NewProc("GetMonitorInfoW").Call(monitor, uintptr(unsafe.Pointer(&info)))
	if ok == 0 {
		return
	}
	dpi, _, _ := user32.NewProc("GetDpiForWindow").Call(w.handle)
	if dpi == 0 {
		dpi = 96
	}
	x, y, cx, cy := windowPlacement(info.Work, int32(width*int(dpi)/96), int32(height*int(dpi)/96))
	var bounds rect
	if ok, _, _ := user32.NewProc("GetWindowRect").Call(w.handle, uintptr(unsafe.Pointer(&bounds))); w.positioned && ok != 0 {
		x, y, cx, cy = keepWindowPlacement(info.Work, bounds.Left, bounds.Top, cx, cy)
	}
	user32.NewProc("SetWindowPos").Call(w.handle, 0, uintptr(x), uintptr(y), uintptr(cx), uintptr(cy), 0x0014)
}
func (w *DesktopWindow) Show() {
	// First show follows the pointer. Later shows retain the position chosen by
	// dragging, clamped to the nearest remaining monitor if a display is removed.
	monitor, _, _ := user32.NewProc("MonitorFromWindow").Call(w.handle, 2)
	if !w.positioned {
		var point struct{ X, Y int32 }
		user32.NewProc("GetCursorPos").Call(uintptr(unsafe.Pointer(&point)))
		packed := uint64(uint32(point.X)) | uint64(uint32(point.Y))<<32
		monitor, _, _ = user32.NewProc("MonitorFromPoint").Call(uintptr(packed), 2)
	}
	info := monitorInfo{Size: uint32(unsafe.Sizeof(monitorInfo{}))}
	user32.NewProc("GetMonitorInfoW").Call(monitor, uintptr(unsafe.Pointer(&info)))
	var bounds rect
	user32.NewProc("GetWindowRect").Call(w.handle, uintptr(unsafe.Pointer(&bounds)))
	if info.Work.Right > info.Work.Left && info.Work.Bottom > info.Work.Top {
		x, y, width, height := windowPlacement(info.Work, bounds.Right-bounds.Left, bounds.Bottom-bounds.Top)
		if w.positioned {
			x, y, width, height = keepWindowPlacement(info.Work, bounds.Left, bounds.Top, width, height)
		}
		if ok, _, _ := user32.NewProc("SetWindowPos").Call(w.handle, ^uintptr(0), uintptr(x), uintptr(y), uintptr(width), uintptr(height), 0x0040); ok != 0 {
			w.positioned = true
		}
	}
	user32.NewProc("ShowWindow").Call(w.handle, 9)
	user32.NewProc("SetForegroundWindow").Call(w.handle)
}

func keepWindowPlacement(work rect, x, y, width, height int32) (int32, int32, int32, int32) {
	width = min(max(1, width), work.Right-work.Left)
	height = min(max(1, height), work.Bottom-work.Top)
	x = min(max(work.Left, x), work.Right-width)
	y = min(max(work.Top, y), work.Bottom-height)
	return x, y, width, height
}

// Keep the entire launcher in the work area on small/high-DPI monitors.
// Coordinates may be negative on monitors left of or above the primary one.
func windowPlacement(work rect, width, height int32) (int32, int32, int32, int32) {
	width = min(max(1, width), work.Right-work.Left)
	height = min(max(1, height), work.Bottom-work.Top)
	x := work.Left + (work.Right-work.Left-width)/2
	y := min(work.Top+(work.Bottom-work.Top)/5, work.Bottom-height)
	return x, y, width, height
}
