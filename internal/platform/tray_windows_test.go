package platform

import (
	"os"
	"testing"
	"time"
)

// 托盘图标需要真实的资源管理器通知区域，因此只在集成模式下运行：
// VELO_INTEGRATION=1 go test ./internal/platform -run TestTray -v
func TestTrayLifecycle(t *testing.T) {
	if os.Getenv("VELO_INTEGRATION") != "1" {
		t.Skip("set VELO_INTEGRATION=1 to create a real notification-area icon")
	}
	tray, err := NewTray(TrayActions{Open: func() {}, Settings: func() {}, Quit: func() {}})
	if err != nil {
		t.Fatal(err)
	}
	time.Sleep(500 * time.Millisecond)
	if err := tray.Close(); err != nil {
		t.Fatal(err)
	}
	if err := tray.Close(); err != nil {
		t.Fatal("托盘重复关闭应是无副作用的", err)
	}
}
