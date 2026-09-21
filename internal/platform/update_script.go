package platform

import (
	"os"
	"strings"
)

// DirectoryWritable 通过实际写入判断能否在不提权的情况下替换程序文件。
// 安装在 Program Files 时返回 false，更新需要改用安装包。
func DirectoryWritable(dir string) bool {
	file, err := os.CreateTemp(dir, ".velo-writable-*")
	if err != nil {
		return false
	}
	name := file.Name()
	file.Close()
	return os.Remove(name) == nil
}

// ReplaceScript 生成便携版更新脚本：等待 Velo 退出后覆盖可执行文件并重启。
// 覆盖失败时同样重启原程序，避免更新失败后应用无法启动。
func ReplaceScript(current, downloaded string) string {
	return strings.Join([]string{
		"$ErrorActionPreference = 'SilentlyContinue'",
		"$target = " + powerShellLiteral(current),
		"$source = " + powerShellLiteral(downloaded),
		"$updated = $false",
		// Velo 退出前可执行文件处于占用状态，覆盖会失败，因此重试而不是放弃。
		"for ($attempt = 0; $attempt -lt 180; $attempt++) {",
		"  try { Copy-Item -LiteralPath $source -Destination $target -Force -ErrorAction Stop; $updated = $true; break } catch { Start-Sleep -Seconds 1 }",
		"}",
		"Start-Process -FilePath $target",
		"if ($updated) { Remove-Item -LiteralPath $source -Force }",
		"Remove-Item -LiteralPath $MyInvocation.MyCommand.Path -Force",
	}, "\r\n") + "\r\n"
}

// InstallScript 生成安装版更新脚本：静默安装新版本，然后启动安装目录中的程序。
// 目标位置不可写说明安装包当初写入了 Program Files，因此需要提升并由 Windows 提示 UAC。
func InstallScript(target, installer string) string {
	return strings.Join([]string{
		"$ErrorActionPreference = 'SilentlyContinue'",
		"$target = " + powerShellLiteral(target),
		"$installer = " + powerShellLiteral(installer),
		"Start-Process -FilePath $installer -ArgumentList '/S' -Verb RunAs -Wait",
		"Start-Process -FilePath $target",
		"Remove-Item -LiteralPath $installer -Force",
		"Remove-Item -LiteralPath $MyInvocation.MyCommand.Path -Force",
	}, "\r\n") + "\r\n"
}

// WriteScript 以 UTF-8 BOM 保存 PowerShell 脚本：Windows PowerShell 5.1 在缺少
// BOM 时按本地 ANSI 代码页解码，用户名含中文时脚本里的路径会被破坏。
func WriteScript(path, content string) error {
	normalized := strings.ReplaceAll(strings.ReplaceAll(content, "\r\n", "\n"), "\n", "\r\n")
	payload := append([]byte{0xEF, 0xBB, 0xBF}, normalized...)
	return os.WriteFile(path, payload, 0600)
}

// powerShellLiteral 输出单引号字面量，内部单引号按 PowerShell 规则翻倍。
func powerShellLiteral(value string) string {
	return "'" + strings.ReplaceAll(value, "'", "''") + "'"
}
