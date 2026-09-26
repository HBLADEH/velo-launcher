package platform

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestReplaceScriptWaitsRetriesAndRestarts(t *testing.T) {
	script := ReplaceScript(`C:\Program Files\Velo\velo-launcher.exe`, `C:\Users\me\Velo\updates\velo-launcher.exe`)
	for _, fragment := range []string{
		"'C:\\Program Files\\Velo\\velo-launcher.exe'",
		"'C:\\Users\\me\\Velo\\updates\\velo-launcher.exe'",
		"Copy-Item -LiteralPath $source -Destination $target -Force",
		"Start-Sleep -Seconds 1",
		"for ($attempt = 0; $attempt -lt 180; $attempt++)",
		"Start-Process -FilePath $target",
	} {
		if !strings.Contains(script, fragment) {
			t.Fatalf("脚本缺少 %q：\n%s", fragment, script)
		}
	}
	// 覆盖失败也必须重启程序，否则更新失败会让 Velo 无法打开。
	if strings.Index(script, "Start-Process -FilePath $target") < strings.Index(script, "for ($attempt") {
		t.Fatal("重启必须发生在替换尝试之后")
	}
}

func TestInstallScriptRunsInstallerElevatedThenRestarts(t *testing.T) {
	script := InstallScript(`C:\Program Files\HBLADEH\Velo\velo-launcher.exe`, `C:\Users\me\Velo\updates\velo-launcher-amd64-installer.exe`)
	for _, fragment := range []string{
		"-ArgumentList $arguments -Verb RunAs -Wait -PassThru",
		"$parent.WaitForExit(180000)",
		"if ($process.ExitCode -ne 0)",
		"Out-File -LiteralPath $log -Encoding UTF8 -Append",
		"'C:\\Program Files\\HBLADEH\\Velo\\velo-launcher.exe'",
		"Start-Process -FilePath $target",
	} {
		if !strings.Contains(script, fragment) {
			t.Fatalf("脚本缺少 %q：\n%s", fragment, script)
		}
	}
}

func TestScriptQuotesSingleQuotedPaths(t *testing.T) {
	script := ReplaceScript(`C:\Users\o'brien\velo.exe`, `C:\Users\o'brien\updates\velo.exe`)
	if !strings.Contains(script, `'C:\Users\o''brien\velo.exe'`) {
		t.Fatalf("单引号路径未按 PowerShell 规则转义：\n%s", script)
	}
}

func TestWriteScriptAddsBOMAndWindowsLineEndings(t *testing.T) {
	path := filepath.Join(t.TempDir(), "update.ps1")
	if err := WriteScript(path, "line-1\nline-2\n"); err != nil {
		t.Fatal(err)
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if len(data) < 3 || data[0] != 0xEF || data[1] != 0xBB || data[2] != 0xBF {
		t.Fatal("缺少 UTF-8 BOM，中文路径会被 PowerShell 5.1 破坏")
	}
	body := string(data[3:])
	if body != "line-1\r\nline-2\r\n" {
		t.Fatalf("行尾未转换为 CRLF：%q", body)
	}
}

func TestDirectoryWritableDetectsReadOnlyTarget(t *testing.T) {
	if !DirectoryWritable(t.TempDir()) {
		t.Fatal("可写目录应返回 true")
	}
	if DirectoryWritable(filepath.Join(t.TempDir(), "missing")) {
		t.Fatal("不存在的目录应返回 false")
	}
}
