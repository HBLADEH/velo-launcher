// Package update 解析 GitHub release，比较版本并下载校验更新文件。
package update

import (
	"fmt"
	"strconv"
	"strings"
)

// Version 是遵循语义化版本的数字段加可选预发布段，例如 0.9.0 或 0.8.0-beta.2。
// 更新比较必须按数字而不是字符串字典序，否则 0.10.0 会被当作旧版本。
type Version struct {
	Major int
	Minor int
	Patch int
	Pre   []string
}

// ParseVersion 接受 v0.9.0、0.9、0.8.0-beta.2、1.2.3+build 等形式。
func ParseVersion(text string) (Version, error) {
	raw := strings.TrimSpace(text)
	raw = strings.TrimPrefix(raw, "v")
	raw = strings.TrimPrefix(raw, "V")
	if raw == "" {
		return Version{}, fmt.Errorf("空版本号")
	}
	// 构建元数据不参与比较。
	if cut := strings.IndexAny(raw, "+"); cut >= 0 {
		raw = raw[:cut]
	}
	core, pre, hasPre := strings.Cut(raw, "-")
	if hasPre && pre == "" {
		return Version{}, fmt.Errorf("无法解析版本号 %q", text)
	}
	parts := strings.Split(core, ".")
	if len(parts) == 0 || len(parts) > 3 {
		return Version{}, fmt.Errorf("无法解析版本号 %q", text)
	}
	numbers := make([]int, 3)
	for n, part := range parts {
		value, err := strconv.Atoi(part)
		if err != nil || value < 0 {
			return Version{}, fmt.Errorf("无法解析版本号 %q", text)
		}
		numbers[n] = value
	}
	version := Version{Major: numbers[0], Minor: numbers[1], Patch: numbers[2]}
	if pre != "" {
		for _, identifier := range strings.Split(pre, ".") {
			if identifier == "" {
				return Version{}, fmt.Errorf("无法解析版本号 %q", text)
			}
			version.Pre = append(version.Pre, identifier)
		}
	}
	return version, nil
}

func (v Version) String() string {
	text := fmt.Sprintf("%d.%d.%d", v.Major, v.Minor, v.Patch)
	if len(v.Pre) > 0 {
		text += "-" + strings.Join(v.Pre, ".")
	}
	return text
}

// Compare 返回 -1、0 或 1，预发布版本低于同号正式版本。
func (v Version) Compare(other Version) int {
	for _, pair := range [][2]int{{v.Major, other.Major}, {v.Minor, other.Minor}, {v.Patch, other.Patch}} {
		if cmp := compareInt(pair[0], pair[1]); cmp != 0 {
			return cmp
		}
	}
	return comparePre(v.Pre, other.Pre)
}

// Newer 报告当前版本是否比 other 更新，供 release 过滤使用。
func (v Version) Newer(other Version) bool { return v.Compare(other) > 0 }

func comparePre(a, b []string) int {
	switch {
	case len(a) == 0 && len(b) == 0:
		return 0
	case len(a) == 0:
		return 1
	case len(b) == 0:
		return -1
	}
	for n := 0; n < len(a) && n < len(b); n++ {
		if cmp := compareIdentifier(a[n], b[n]); cmp != 0 {
			return cmp
		}
	}
	return compareInt(len(a), len(b))
}

// compareIdentifier 实现 semver 规则：数字段按数值比较且低于字母段。
func compareIdentifier(a, b string) int {
	aNumber, aErr := strconv.Atoi(a)
	bNumber, bErr := strconv.Atoi(b)
	switch {
	case aErr == nil && bErr == nil:
		return compareInt(aNumber, bNumber)
	case aErr == nil:
		return -1
	case bErr == nil:
		return 1
	default:
		return strings.Compare(a, b)
	}
}

func compareInt(a, b int) int {
	switch {
	case a < b:
		return -1
	case a > b:
		return 1
	default:
		return 0
	}
}
