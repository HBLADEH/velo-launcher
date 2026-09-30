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
		// Flags=0 retains the Shell's confirmation, progress and sound.
		hr, _, _ := windows.NewLazySystemDLL("shell32.dll").NewProc("SHEmptyRecycleBinW").Call(0, 0, 0)
		if int32(hr) < 0 && uint32(hr) != 0x800704C7 {
			return fmt.Errorf("清空回收站失败: 0x%08X", uint32(hr))
		}
		return nil
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
