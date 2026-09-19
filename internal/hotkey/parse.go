package hotkey

import (
	"fmt"
	"strconv"
	"strings"
)

type Key struct {
	Modifiers uint32
	Code      uint32
}

func Parse(text string) (Key, error) {
	parts := strings.Split(strings.ToUpper(strings.TrimSpace(text)), "+")
	var key Key
	if len(parts) < 2 {
		return key, fmt.Errorf("快捷键须包含 Ctrl、Alt、Shift 或 Win 修饰键")
	}
	for _, part := range parts[:len(parts)-1] {
		var flag uint32
		switch strings.TrimSpace(part) {
		case "ALT":
			flag = 1
		case "CTRL", "CONTROL":
			flag = 2
		case "SHIFT":
			flag = 4
		case "WIN", "SUPER":
			flag = 8
		default:
			return key, fmt.Errorf("未知修饰键: %s", part)
		}
		if key.Modifiers&flag != 0 {
			return key, fmt.Errorf("重复修饰键: %s", part)
		}
		key.Modifiers |= flag
	}
	last := strings.TrimSpace(parts[len(parts)-1])
	switch last {
	case "SPACE":
		key.Code = 0x20
	case "ENTER":
		key.Code = 0x0D
	case "TAB":
		key.Code = 9
	default:
		if len(last) == 1 && ((last[0] >= 'A' && last[0] <= 'Z') || (last[0] >= '0' && last[0] <= '9')) {
			key.Code = uint32(last[0])
		} else if strings.HasPrefix(last, "F") {
			n, err := strconv.Atoi(last[1:])
			if err != nil || n < 1 || n > 24 {
				return key, fmt.Errorf("无效功能键")
			}
			key.Code = uint32(0x70 + n - 1)
		} else {
			return key, fmt.Errorf("支持字母、数字、Space、Enter、Tab 或 F1–F24")
		}
	}
	return key, nil
}
