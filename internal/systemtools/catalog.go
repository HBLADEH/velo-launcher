// Package systemtools builds Windows shortcuts from a curated command catalogue
// and a read-only snapshot of local capabilities. Search text is never executable.
package systemtools

import (
	"path/filepath"
	"strings"

	"velo-launcher/internal/model"
)

type Environment struct {
	WindowsDir         string
	Build              uint32
	SettingsRegistered bool
	// Both machine and user SettingsPageVisibility policies must permit a page.
	PagePolicies []string
	Exists       func(string) bool
}

type definition struct {
	id, name, target, args, description string
	keywords                            []string
	required                            string
	minBuild                            uint32
}

func exe(id, name, target, args, description string, keywords ...string) definition {
	return definition{id: id, name: name, target: target, args: args, description: description, keywords: keywords}
}
func console(id, name, file, description string, keywords ...string) definition {
	d := exe(id, name, "System32/mmc.exe", "", description, keywords...)
	d.required = "System32/" + file
	return d
}
func setting(id, name, page, description string, keywords ...string) definition {
	d := exe(id, name, "ms-settings:"+page, "", description, keywords...)
	d.minBuild = 10240
	if page == "sound" {
		d.minBuild = 17134
	}
	if page == "network-status" {
		d.minBuild = 14393
	}
	return d
}

var catalogue = []definition{
	exe("calculator", "计算器", "System32/calc.exe", "", "打开 Windows 计算器", "calc", "calculator"),
	exe("explorer", "文件资源管理器", "explorer.exe", "", "浏览文件与文件夹", "explorer", "file explorer", "files", "文件管理", "资源管理器"),
	exe("taskmanager", "任务管理器", "System32/Taskmgr.exe", "", "查看进程与资源使用情况", "task manager", "taskmgr", "进程"),
	exe("terminal", "命令提示符", "System32/cmd.exe", "", "打开命令行终端", "cmd", "command prompt", "terminal", "终端", "命令行"),
	exe("control", "控制面板", "System32/control.exe", "", "打开 Windows 控制面板", "control panel"),
	{id: "apps", name: "卸载或更改程序", target: "System32/control.exe", args: "appwiz.cpl", required: "System32/appwiz.cpl", description: "管理已安装的桌面程序", keywords: []string{"apps", "uninstall", "appwiz", "应用管理", "卸载程序", "xiezai", "xz"}},
	{id: "environment", name: "环境变量", target: "System32/rundll32.exe", args: "sysdm.cpl,EditEnvironmentVariables", required: "System32/sysdm.cpl", description: "编辑用户与系统环境变量", keywords: []string{"environment", "env", "path", "系统属性环境变量"}},
	console("devices", "设备管理器", "devmgmt.msc", "查看硬件设备与驱动", "device manager", "devmgmt", "驱动"),
	exe("systemconfig", "系统配置", "System32/msconfig.exe", "", "查看启动与服务配置", "msconfig", "system configuration"),
	exe("systeminfo", "系统信息", "System32/msinfo32.exe", "", "查看硬件、组件与系统信息", "msinfo32", "system information"),
	console("computer", "计算机管理", "compmgmt.msc", "管理磁盘、服务与本地计算机", "computer management", "compmgmt"),
	console("events", "事件查看器", "eventvwr.msc", "查看 Windows 日志与系统事件", "event viewer", "eventvwr", "系统日志"),
	console("services", "服务管理", "services.msc", "查看与管理 Windows 服务", "services", "系统服务"),
	console("disks", "磁盘管理", "diskmgmt.msc", "查看磁盘、分区与卷", "disk management", "diskmgmt"),
	console("tasks", "任务计划程序", "taskschd.msc", "查看与管理计划任务", "task scheduler", "taskschd", "计划任务"),
	console("components", "组件服务", "comexp.msc", "管理 COM+ 应用程序与组件服务", "component services", "comexp", "dcomcnfg"),
	console("grouppolicy", "本地组策略编辑器", "gpedit.msc", "查看与编辑本地组策略", "group policy", "gpedit", "组策略"),
	exe("cleanup", "磁盘清理", "System32/cleanmgr.exe", "", "打开磁盘清理，由你选择要清理的项目", "disk cleanup", "cleanmgr"),
	exe("optimize", "碎片整理和优化驱动器", "System32/dfrgui.exe", "", "查看驱动器优化状态", "defragment", "optimize drives", "dfrgui", "磁盘优化"),
	exe("resources", "资源监视器", "System32/resmon.exe", "", "查看 CPU、内存、磁盘与网络使用情况", "resource monitor", "resmon"),
	exe("colorprofile", "颜色管理", "System32/colorcpl.exe", "", "管理显示器颜色配置文件", "color management", "colorcpl"),
	setting("display", "显示设置", "display", "调整分辨率、缩放与显示器布局", "display settings", "分辨率", "屏幕"),
	setting("sound", "声音设置", "sound", "管理音频输入、输出与音量", "sound settings", "音量", "麦克风"),
	setting("colors", "系统颜色设置", "personalization-colors", "调整 Windows 主题色与外观", "colors", "主题色"),
	setting("activation", "系统激活设置", "activation", "查看 Windows 激活状态", "activation", "激活"),
	setting("recovery", "系统恢复设置", "recovery", "查看 Windows 恢复选项", "recovery", "恢复"),
	setting("updates", "检查系统更新", "windowsupdate", "打开 Windows 更新页面", "windows update", "系统更新"),
	setting("about", "关于系统", "about", "查看 Windows 版本与设备规格", "about windows", "系统版本"),
	setting("storage", "存储设置", "storagesense", "查看磁盘空间与存储管理选项", "storage", "磁盘空间"),
	setting("network", "网络状态", "network-status", "查看网络连接与状态", "network status", "网络设置"),
	setting("proxy", "网络代理设置", "network-proxy", "查看与配置代理服务器", "proxy", "代理"),
	setting("vpn", "VPN 设置", "network-vpn", "管理 VPN 连接", "vpn"),
	setting("printers", "打印机和扫描仪", "printers", "管理打印设备与扫描仪", "printers", "scanners"),
	setting("mouse", "鼠标设置", "mousetouchpad", "调整鼠标按钮与滚动选项", "mouse settings"),
	setting("datetime", "日期和时间", "dateandtime", "调整日期、时间与时区", "date and time", "时区"),
	setting("defaultapps", "默认应用", "defaultapps", "选择文件与链接的默认打开应用", "default apps", "默认程序"),
	setting("notifications", "通知设置", "notifications", "管理应用与系统通知", "notifications"),
	setting("signin", "登录选项", "signinoptions", "管理账户登录方式", "sign in options", "登录设置"),
	{id: "startup", name: "启动应用", target: "ms-settings:startupapps", minBuild: 17134, description: "管理登录时自动启动的应用", keywords: []string{"startup apps", "开机启动"}},
	// Shared experiences was removed from the Settings UI in Windows 11.
	{id: "shared", name: "共享体验设置", target: "ms-settings:crossdevice", minBuild: 15063, description: "管理 Windows 10 跨设备共享体验", keywords: []string{"shared experiences", "跨设备"}},
}

func Discover(env Environment) []model.AppItem {
	items := make([]model.AppItem, 0, len(catalogue))
	for _, d := range catalogue {
		path, args := d.target, d.args
		if strings.HasPrefix(path, "ms-settings:") {
			if !env.SettingsRegistered || env.Build < d.minBuild || (d.id == "shared" && env.Build >= 22000) || !pageVisible(strings.TrimPrefix(path, "ms-settings:"), env.PagePolicies) {
				continue
			}
		} else {
			path = filepath.Join(env.WindowsDir, filepath.FromSlash(path))
			if !env.Exists(path) {
				continue
			}
			if d.required != "" {
				required := filepath.Join(env.WindowsDir, filepath.FromSlash(d.required))
				if !env.Exists(required) {
					continue
				}
				if strings.HasSuffix(d.required, ".msc") {
					args = `"` + required + `"`
				}
			}
		}
		items = append(items, model.AppItem{ID: "system:" + d.id, Name: d.name, Path: path, ExecPath: path, Arguments: args, WorkingDirectory: filepath.Join(env.WindowsDir, "System32"), Source: "System", Description: d.description, Keywords: append([]string{}, d.keywords...)})
	}
	return items
}

func pageVisible(page string, policies []string) bool {
	for _, policy := range policies {
		mode, list, ok := strings.Cut(strings.ToLower(strings.TrimSpace(policy)), ":")
		if !ok {
			continue
		}
		found := false
		for _, candidate := range strings.Split(list, ";") {
			if strings.TrimSpace(candidate) == page {
				found = true
				break
			}
		}
		if (mode == "hide" && found) || (mode == "showonly" && !found) {
			return false
		}
	}
	return true
}

// Settings launches must be an exact entry in the catalogue, with no arguments.
func IsSettingsItem(item model.AppItem) bool {
	if item.Source != "System" || item.Arguments != "" || item.Path != item.ExecPath {
		return false
	}
	for _, d := range catalogue {
		if strings.HasPrefix(d.target, "ms-settings:") && item.ID == "system:"+d.id && item.Path == d.target {
			return true
		}
	}
	return false
}
