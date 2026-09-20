package indexer

import (
	"regexp"
	"strings"
	"unicode"

	"velo-launcher/internal/model"
)

// 启动台只应该列出"能启动应用"的入口。参考同类启动器的默认行为，卸载、
// 帮助、更新等辅助入口会被隐藏，用户在设置中关闭过滤后仍可看到全部结果。
var (
	noiseTokens = map[string]bool{
		"uninstall": true, "uninstaller": true, "uninst": true,
		"help": true, "readme": true, "documentation": true, "docs": true, "manual": true,
		"website": true, "homepage": true, "changelog": true, "license": true, "licence": true,
		"repair": true, "modify": true, "updater": true, "update": true,
		"install": true, "installer": true, "setup": true,
		"about": true, "support": true,
	}
	noisePhrases = []string{
		"release notes", "what's new", "read me", "help center",
	}
	// 中文没有词边界，只能按子串判断。
	noiseChinese = []string{
		"卸载", "移除", "帮助", "说明", "自述", "文档", "手册",
		"网站", "官网", "主页", "在线帮助", "更新日志", "新特性", "发行说明",
		"许可", "修复", "更改", "更新", "安装", "关于",
	}
	// Windows 安装器生成的 unins000.exe 一类文件。
	uninstallerName = regexp.MustCompile(`^unins\d*$`)
)

// IsNoise 判断条目名称是否属于辅助入口。名称分词后整词匹配，
// 因此 "Helpdesk"、"UpdateTool" 这类真实程序不会被误伤。
func IsNoise(name string) bool {
	lowered := strings.ToLower(strings.TrimSpace(name))
	if lowered == "" {
		return false
	}
	if uninstallerName.MatchString(lowered) {
		return true
	}
	for _, phrase := range noisePhrases {
		if strings.Contains(lowered, phrase) {
			return true
		}
	}
	for _, token := range strings.FieldsFunc(lowered, func(r rune) bool { return !unicode.IsLetter(r) && !unicode.IsDigit(r) }) {
		if noiseTokens[token] {
			return true
		}
	}
	for _, word := range noiseChinese {
		if strings.Contains(lowered, word) {
			return true
		}
	}
	return false
}

// Visible 返回搜索索引真正使用的条目：先隐藏辅助入口，再合并重名副本。
func Visible(items []model.AppItem, filterNoise bool) []model.AppItem {
	if !filterNoise {
		return Deduplicate(items)
	}
	kept := make([]model.AppItem, 0, len(items))
	for _, item := range items {
		if !IsNoise(item.Name) {
			kept = append(kept, item)
		}
	}
	return Deduplicate(kept)
}
