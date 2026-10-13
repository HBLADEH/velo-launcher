package platform

import (
	"fmt"
	"os/exec"
	"path/filepath"
	"syscall"
	"unsafe"

	"golang.org/x/sys/windows"
	"velo-launcher/internal/model"
)

func launchSystemAction(item model.AppItem) error {
	switch item.ID {
	case "system:empty-recycle-bin":
		return emptyRecycleBin(queryRecycleBin, func() uintptr {
			// Flags=0 retains the Shell's confirmation, progress and sound.
			hr, _, _ := windows.NewLazySystemDLL("shell32.dll").NewProc("SHEmptyRecycleBinW").Call(0, 0, 0)
			return hr
		}, notifyEmptyRecycleBin)
	case "system:lock":
		ok, _, err := user32.NewProc("LockWorkStation").Call()
		if ok == 0 {
			return fmt.Errorf("锁屏失败: %w", err)
		}
		return nil
	case "system:shutdown", "system:restart":
		message, _ := windows.UTF16PtrFromString("确定要" + item.Name + "吗？请先保存正在编辑的文件。")
		title, _ := windows.UTF16PtrFromString("Velo · " + item.Name)
		answer, _, _ := user32.NewProc("MessageBoxW").Call(0, uintptr(unsafe.Pointer(message)), uintptr(unsafe.Pointer(title)), 0x00010000|0x00000100|0x30|0x01)
		if answer != 1 {
			return nil
		}
		root, err := windows.GetWindowsDirectory()
		if err != nil {
			return err
		}
		flag := "/s"
		if item.ID == "system:restart" {
			flag = "/r"
		}
		cmd := exec.Command(filepath.Join(root, "System32", "shutdown.exe"), flag, "/t", "0")
		cmd.SysProcAttr = &syscall.SysProcAttr{HideWindow: true}
		return cmd.Run()
	}
	return fmt.Errorf("不支持的系统操作")
}

// SHQUERYRBINFO is packed on Win32 and naturally aligned on Win64. Go's
// alignment matches both layouts: 20 bytes on 386 and 24 bytes on amd64/arm64.
type recycleBinInfo struct {
	cbSize uint32
	size   int64
	items  int64
}

func queryRecycleBin(info *recycleBinInfo) uintptr {
	hr, _, _ := windows.NewLazySystemDLL("shell32.dll").NewProc("SHQueryRecycleBinW").Call(0, uintptr(unsafe.Pointer(info)))
	return hr
}

func notifyEmptyRecycleBin() error {
	message, _ := windows.UTF16PtrFromString("回收站已为空，无需清理。")
	title, _ := windows.UTF16PtrFromString("Velo · 清空回收站")
	// MB_OK | MB_ICONINFORMATION | MB_SETFOREGROUND. The focused OK button
	// supports Space and Enter without a separate confirmation or cancel action.
	answer, _, err := user32.NewProc("MessageBoxW").Call(0, uintptr(unsafe.Pointer(message)), uintptr(unsafe.Pointer(title)), 0x00010000|0x40)
	if answer == 0 {
		return fmt.Errorf("显示回收站提示失败: %w", err)
	}
	return nil
}

func emptyRecycleBin(query func(*recycleBinInfo) uintptr, empty func() uintptr, notifyEmpty func() error) error {
	isEmpty := func() bool {
		info := recycleBinInfo{}
		info.cbSize = uint32(unsafe.Sizeof(info))
		hr := query(&info)
		return int32(hr) >= 0 && info.items == 0
	}
	// An empty bin gets an informational notice without invoking SHEmptyRecycleBinW.
	// A failed query must not suppress the actual operation.
	if isEmpty() {
		return notifyEmpty()
	}
	hr := empty()
	// The bin can become empty while the Shell confirmation is open, or the
	// initial query can fail. Recheck E_UNEXPECTED before reporting a failure.
	if uint32(hr) == 0x8000FFFF && isEmpty() {
		return nil
	}
	if int32(hr) < 0 && uint32(hr) != 0x800704C7 {
		return fmt.Errorf("清空回收站失败: 0x%08X", uint32(hr))
	}
	return nil
}
