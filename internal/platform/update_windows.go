package platform

import (
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"syscall"

	"golang.org/x/sys/windows/registry"
)

// 安装包把卸载信息写在这个键下（见 build/windows/installer/wails_tools.nsh）。
const uninstallKey = `Software\Microsoft\Windows\CurrentVersion\Uninstall\HBLADEHVelo`

const (
	createNoWindow        = 0x08000000
	createNewProcessGroup = 0x00000200
)

// RunDetachedScript 启动独立于 Velo 的 PowerShell 完成替换或安装。调用方随后
// 应当退出，让脚本可以覆盖正在运行的可执行文件。
func RunDetachedScript(path string) error {
	command := exec.Command("powershell.exe", "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-WindowStyle", "Hidden", "-File", path)
	// Windows PowerShell 5.1 在 DETACHED_PROCESS 下可能启动后立即退出，
	// 即使 Start 返回成功也不会执行脚本。CREATE_NO_WINDOW 保留可用的
	// 控制台语义且不显示窗口，子进程仍可在 Velo 退出后继续运行。
	command.SysProcAttr = &syscall.SysProcAttr{HideWindow: true, CreationFlags: createNoWindow | createNewProcessGroup}
	if err := command.Start(); err != nil {
		return err
	}
	return command.Process.Release()
}

// InstalledExecutable 由安装信息回到安装目录，用于静默安装后启动新版本。
// 便携版没有卸载键，返回 false。
func InstalledExecutable() (string, bool) {
	for _, root := range []registry.Key{registry.LOCAL_MACHINE, registry.CURRENT_USER} {
		key, err := registry.OpenKey(root, uninstallKey, registry.QUERY_VALUE|registry.WOW64_64KEY)
		if err != nil {
			continue
		}
		raw, _, err := key.GetStringValue("UninstallString")
		key.Close()
		if err != nil {
			continue
		}
		exe := filepath.Join(filepath.Dir(strings.Trim(strings.TrimSpace(raw), `"`)), "velo-launcher.exe")
		if info, err := os.Stat(exe); err == nil && !info.IsDir() {
			return exe, true
		}
	}
	return "", false
}
