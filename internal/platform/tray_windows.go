package platform

import (
	"errors"
	"fmt"
	"os"
	"runtime"
	"sync"
	"syscall"
	"time"
	"unsafe"

	"golang.org/x/sys/windows"
)

// 通知区域图标拥有独立的窗口与消息循环，因此右键菜单、资源管理器重启
// 等交互不会占用 Wails 的主窗口线程。
const (
	trayWindowClass  = "VeloLauncherTray"
	trayIconID       = 1
	trayMessage      = 0x8020 // WM_APP + 0x20：图标回调消息
	menuOpenItem     = 1
	menuSettingsItem = 2
	menuQuitItem     = 3

	wmDestroy       = 0x0002
	wmClose         = 0x0010
	wmLButtonUp     = 0x0202
	wmLButtonDblClk = 0x0203
	wmRButtonUp     = 0x0205
	wmNull          = 0x0000

	nimAdd     = 0
	nimDelete  = 2
	nifMessage = 0x1
	nifIcon    = 0x2
	nifTip     = 0x4

	mfString       = 0x0000
	mfSeparator    = 0x0800
	mfDefault      = 0x1000
	tpmRightButton = 0x0002
	tpmReturnCmd   = 0x0100

	smCXSmIcon     = 49
	idiApplication = 32512
	classExists    = 1410 // ERROR_CLASS_ALREADY_EXISTS
	closeTimeout   = 2 * time.Second
)

var kernel32 = windows.NewLazySystemDLL("kernel32.dll")

var (
	getModuleHandle       = kernel32.NewProc("GetModuleHandleW")
	registerClassEx       = user32.NewProc("RegisterClassExW")
	createWindowEx        = user32.NewProc("CreateWindowExW")
	defWindowProc         = user32.NewProc("DefWindowProcW")
	destroyWindow         = user32.NewProc("DestroyWindow")
	translateMessage      = user32.NewProc("TranslateMessage")
	dispatchMessage       = user32.NewProc("DispatchMessageW")
	postQuitMessage       = user32.NewProc("PostQuitMessage")
	postMessage           = user32.NewProc("PostMessageW")
	registerWindowMessage = user32.NewProc("RegisterWindowMessageW")
	createPopupMenu       = user32.NewProc("CreatePopupMenu")
	appendMenu            = user32.NewProc("AppendMenuW")
	destroyMenu           = user32.NewProc("DestroyMenu")
	trackPopupMenu        = user32.NewProc("TrackPopupMenu")
	setForegroundWindow   = user32.NewProc("SetForegroundWindow")
	getCursorPos          = user32.NewProc("GetCursorPos")
	getSystemMetrics      = user32.NewProc("GetSystemMetrics")
	loadIcon              = user32.NewProc("LoadIconW")
	destroyIcon           = user32.NewProc("DestroyIcon")
	shellNotifyIcon       = shell32.NewProc("Shell_NotifyIconW")
	extractIconEx         = shell32.NewProc("ExtractIconExW")
)

// TrayActions 由应用层提供；托盘线程只在用户选择菜单项后异步回调。
type TrayActions struct {
	Open     func()
	Settings func()
	Quit     func()
}

// trayOwner 供窗口过程取回托盘实例。托盘窗口的消息只在创建它的线程上
// 投递，因此这里不需要额外同步。
var trayOwner *Tray

type Tray struct {
	actions     TrayActions
	hwnd        uintptr
	icon        uintptr
	destroyable bool
	taskCreated uint32
	once        sync.Once
	done        chan struct{}
	err         error
}

type windowClassEx struct {
	Size        uint32
	Style       uint32
	Procedure   uintptr
	ClassExtra  int32
	WindowExtra int32
	Instance    uintptr
	Icon        uintptr
	Cursor      uintptr
	Background  uintptr
	MenuName    *uint16
	ClassName   *uint16
	IconSmall   uintptr
}
type cursorPoint struct{ X, Y int32 }
type notifyIconData struct {
	Size             uint32
	Window           uintptr
	ID               uint32
	Flags            uint32
	CallbackMessage  uint32
	Icon             uintptr
	Tip              [128]uint16
	State            uint32
	StateMask        uint32
	Info             [256]uint16
	TimeoutOrVersion uint32
	InfoTitle        [64]uint16
	InfoFlags        uint32
	GUID             windows.GUID
	BalloonIcon      uintptr
}

func NewTray(actions TrayActions) (*Tray, error) {
	tray := &Tray{actions: actions, done: make(chan struct{})}
	ready := make(chan error, 1)
	go func() {
		runtime.LockOSThread()
		defer runtime.UnlockOSThread()
		defer close(tray.done)
		tray.err = tray.serve(ready)
	}()
	if err := <-ready; err != nil {
		<-tray.done
		return nil, err
	}
	return tray, nil
}

// Close 移除图标并结束消息循环。重复调用或线程已退出时立即返回。
func (t *Tray) Close() error {
	t.once.Do(func() { postMessage.Call(t.hwnd, wmClose, 0, 0) })
	select {
	case <-t.done:
		return t.err
	case <-time.After(closeTimeout):
		return fmt.Errorf("托盘线程未在超时内退出")
	}
}

func (t *Tray) serve(ready chan<- error) error {
	instance, _, _ := getModuleHandle.Call(0)
	className, err := windows.UTF16FromString(trayWindowClass)
	if err != nil {
		ready <- err
		return nil
	}
	title, err := windows.UTF16FromString("Velo")
	if err != nil {
		ready <- err
		return nil
	}
	class := windowClassEx{Size: uint32(unsafe.Sizeof(windowClassEx{})), Procedure: syscall.NewCallback(trayProcedure), Instance: instance, ClassName: &className[0]}
	if atom, _, callErr := registerClassEx.Call(uintptr(unsafe.Pointer(&class))); atom == 0 && !errors.Is(callErr, syscall.Errno(classExists)) {
		ready <- fmt.Errorf("托盘窗口类注册失败: %w", callErr)
		return nil
	}
	// 顶层但从不显示：既不会出现在任务栏，又能收到资源管理器的广播消息。
	hwnd, _, callErr := createWindowEx.Call(0, uintptr(unsafe.Pointer(&className[0])), uintptr(unsafe.Pointer(&title[0])), 0, 0, 0, 0, 0, 0, 0, instance, 0)
	if hwnd == 0 {
		ready <- fmt.Errorf("托盘窗口创建失败: %w", callErr)
		return nil
	}
	defer destroyWindow.Call(hwnd)
	t.hwnd = hwnd
	trayOwner = t // 托盘窗口的消息只投递给创建它的线程
	defer func() { trayOwner = nil }()
	created, _ := windows.UTF16FromString("TaskbarCreated")
	registered, _, _ := registerWindowMessage.Call(uintptr(unsafe.Pointer(&created[0])))
	t.taskCreated = uint32(registered)
	icon, destroyable, err := trayIconHandle()
	if err != nil {
		ready <- err
		return nil
	}
	t.icon, t.destroyable = icon, destroyable
	if !t.addIcon(hwnd) {
		if destroyable {
			destroyIcon.Call(icon)
		}
		ready <- fmt.Errorf("托盘图标注册失败")
		return nil
	}
	ready <- nil
	var msg message
	for {
		ok, _, _ := getMessage.Call(uintptr(unsafe.Pointer(&msg)), 0, 0, 0)
		if int32(ok) <= 0 {
			return nil
		}
		translateMessage.Call(uintptr(unsafe.Pointer(&msg)))
		dispatchMessage.Call(uintptr(unsafe.Pointer(&msg)))
	}
}

func (t *Tray) addIcon(hwnd uintptr) bool {
	data := notifyIconData{Window: hwnd, ID: trayIconID, Flags: nifMessage | nifIcon | nifTip, CallbackMessage: trayMessage, Icon: t.icon}
	data.Size = uint32(unsafe.Sizeof(data))
	copy(data.Tip[:], windows.StringToUTF16("Velo · 启动台"))
	ok, _, _ := shellNotifyIcon.Call(nimAdd, uintptr(unsafe.Pointer(&data)))
	return ok != 0
}

func (t *Tray) removeIcon() {
	data := notifyIconData{Window: t.hwnd, ID: trayIconID}
	data.Size = uint32(unsafe.Sizeof(data))
	shellNotifyIcon.Call(nimDelete, uintptr(unsafe.Pointer(&data)))
	if t.destroyable && t.icon != 0 {
		destroyIcon.Call(t.icon)
		t.icon = 0
	}
}

// trayIconHandle 优先从 Velo 可执行文件提取图标，保证托盘与程序图标一致。
func trayIconHandle() (uintptr, bool, error) {
	exe, err := os.Executable()
	if err != nil {
		return 0, false, err
	}
	path, err := windows.UTF16PtrFromString(exe)
	if err != nil {
		return 0, false, err
	}
	var large, small uintptr
	count, _, _ := extractIconEx.Call(uintptr(unsafe.Pointer(path)), 0, uintptr(unsafe.Pointer(&large)), uintptr(unsafe.Pointer(&small)), 1)
	if count == 0 || (large == 0 && small == 0) {
		if icon, _, _ := loadIcon.Call(0, idiApplication); icon != 0 {
			return icon, false, nil // 共享图标由系统持有，不能销毁。
		}
		return 0, false, fmt.Errorf("托盘图标不可用")
	}
	// 通知区域使用小图标；高 DPI 下小图标会被放大，此时改用大图标缩放。
	size, _, _ := getSystemMetrics.Call(smCXSmIcon)
	keep, drop := large, small
	if size == 0 || size <= 16 {
		keep, drop = small, large
	}
	if keep == 0 {
		keep, drop = drop, 0
	}
	if drop != 0 {
		destroyIcon.Call(drop)
	}
	return keep, true, nil
}

func trayProcedure(hwnd, message, wparam, lparam uintptr) uintptr {
	if tray := trayOwner; tray != nil {
		if handled, result := tray.handle(hwnd, message, wparam, lparam); handled {
			return result
		}
	}
	result, _, _ := defWindowProc.Call(hwnd, message, wparam, lparam)
	return result
}

func (t *Tray) handle(hwnd, message, wparam, lparam uintptr) (bool, uintptr) {
	switch {
	case t.taskCreated != 0 && uint32(message) == t.taskCreated:
		t.addIcon(hwnd) // 资源管理器重启后需要重新注册
		return true, 0
	case message == trayMessage:
		switch uint32(lparam) {
		case wmLButtonUp, wmLButtonDblClk:
			t.invoke(t.actions.Open)
		case wmRButtonUp:
			t.showMenu(hwnd)
		}
		return true, 0
	case message == wmClose:
		destroyWindow.Call(hwnd)
		return true, 0
	case message == wmDestroy:
		t.removeIcon()
		postQuitMessage.Call(0)
		return true, 0
	}
	return false, 0
}

func (t *Tray) showMenu(hwnd uintptr) {
	menu, _, _ := createPopupMenu.Call()
	if menu == 0 {
		return
	}
	defer destroyMenu.Call(menu)
	for _, entry := range []struct {
		id    uintptr
		flags uintptr
		text  string
	}{
		{menuOpenItem, mfString | mfDefault, "打开 Velo"},
		{menuSettingsItem, mfString, "设置…"},
		{0, mfSeparator, ""},
		{menuQuitItem, mfString, "退出"},
	} {
		if entry.flags&mfSeparator != 0 {
			appendMenu.Call(menu, mfSeparator, 0, 0)
			continue
		}
		text, err := windows.UTF16FromString(entry.text)
		if err != nil {
			continue
		}
		appendMenu.Call(menu, entry.flags, entry.id, uintptr(unsafe.Pointer(&text[0])))
	}
	var point cursorPoint
	getCursorPos.Call(uintptr(unsafe.Pointer(&point)))
	// 菜单要求拥有者在前台，否则点击其他窗口时不会关闭。
	setForegroundWindow.Call(hwnd)
	command, _, _ := trackPopupMenu.Call(menu, tpmRightButton|tpmReturnCmd, uintptr(point.X), uintptr(point.Y), 0, hwnd, 0)
	postMessage.Call(hwnd, wmNull, 0, 0)
	switch command {
	case menuOpenItem:
		t.invoke(t.actions.Open)
	case menuSettingsItem:
		t.invoke(t.actions.Settings)
	case menuQuitItem:
		t.invoke(t.actions.Quit)
	}
}

// invoke 让动作离开消息循环线程，界面操作不会阻塞托盘图标。
func (t *Tray) invoke(action func()) {
	if action == nil {
		return
	}
	go action()
}
