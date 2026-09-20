package indexer

import (
	"testing"

	"velo-launcher/internal/model"
)

func TestNoiseDetection(t *testing.T) {
	for _, name := range []string{
		"Uninstall 7-Zip", "7-Zip Help", "Visual Studio Code 帮助", "unins000",
		"卸载 Visual Studio Code", "Visual Studio Code 更新程序", "About GIMP",
		"Notepad++ 网站", "Microsoft Edge Update", "Readme", "Release Notes",
		"X 安装程序", "用户手册", "命令提示符 在线帮助",
	} {
		if !IsNoise(name) {
			t.Errorf("%q 应被识别为辅助项", name)
		}
	}
	// 名称分词后整词匹配，包含相同前缀的真实应用不能被误伤。
	for _, name := range []string{
		"Visual Studio Code", "7-Zip File Manager", "Helpdesk", "UpdateTool",
		"微信", "NVIDIA app", "Windows Terminal", "小米妙享", "Python 3.12",
		"Adobe Photoshop 2024", "InstallShield",
	} {
		if IsNoise(name) {
			t.Errorf("%q 不应被识别为辅助项", name)
		}
	}
}

func TestVisibleFiltersNoiseAndMergesDuplicates(t *testing.T) {
	const code = `C:\Users\me\AppData\Local\Programs\Microsoft VS Code\Code.exe`
	items := []model.AppItem{
		{ID: "deep", Name: "7z", Source: "Program Files", Path: `C:\Program Files\NVIDIA Corporation\NVIDIA app\7z.exe`, ExecPath: `C:\Program Files\NVIDIA Corporation\NVIDIA app\7z.exe`},
		{ID: "shallow", Name: "7z", Source: "Program Files", Path: `C:\Program Files\7-Zip\7z.exe`, ExecPath: `C:\Program Files\7-Zip\7z.exe`},
		{ID: "help", Name: "7-Zip Help", Source: "Start Menu", Path: `C:\ProgramData\Microsoft\Windows\Start Menu\Programs\7-Zip\7-Zip Help.lnk`},
		{ID: "user", Name: "Visual Studio Code", Source: "Start Menu", Path: `C:\Users\me\AppData\Roaming\Microsoft\Windows\Start Menu\Programs\Visual Studio Code\Visual Studio Code.lnk`, ExecPath: code},
		{ID: "common", Name: "Visual Studio Code", Source: "Start Menu", Path: `C:\ProgramData\Microsoft\Windows\Start Menu\Programs\Visual Studio Code\Visual Studio Code.lnk`, ExecPath: code},
	}
	visible := Visible(items, true)
	if len(visible) != 2 {
		t.Fatalf("过滤后应保留 2 条，实际 %+v", visible)
	}
	if visible[0].Name != "7z" || visible[0].ID != "shallow" {
		t.Fatalf("重名应用应保留路径更浅的副本: %+v", visible[0])
	}
	if visible[1].Name != "Visual Studio Code" || visible[1].ID != "common" {
		t.Fatalf("多个开始菜单快捷方式应合并为一条: %+v", visible[1])
	}
	// 关闭过滤后辅助项仍然可用。
	if all := Visible(items, false); len(all) != 3 {
		t.Fatalf("关闭过滤后应保留 3 条，实际 %+v", all)
	}
}

func TestVisibleKeepsSameNameDifferentTargets(t *testing.T) {
	items := []model.AppItem{
		{ID: "packaged", Name: "Phone Link", Source: "Windows Apps", Path: `shell:AppsFolder\Microsoft.YourPhone_8wekyb3d8bbwe!App`, ExecPath: `shell:AppsFolder\Microsoft.YourPhone_8wekyb3d8bbwe!App`},
		{ID: "shortcut", Name: "Phone Link", Source: "Start Menu", Path: `C:\ProgramData\Microsoft\Windows\Start Menu\Programs\Phone Link.lnk`, ExecPath: `C:\Program Files\Phone Link\PhoneLink.exe`},
	}
	if visible := Visible(items, true); len(visible) != 2 {
		t.Fatalf("目标不同的同名应用都应保留: %+v", visible)
	}
}
