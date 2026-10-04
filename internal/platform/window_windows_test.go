package platform

import "testing"

func TestDraggedWindowPositionSurvivesResizeAndRecall(t *testing.T) {
	for _, tt := range []struct {
		name               string
		work, bounds, want rect
	}{
		{"unchanged", rect{0, 0, 1920, 1040}, rect{100, 70, 740, 730}, rect{100, 70, 740, 730}},
		{"grow near bottom", rect{0, 0, 1920, 1040}, rect{100, 800, 740, 1460}, rect{100, 380, 740, 1040}},
		{"left monitor", rect{-1920, -200, 0, 840}, rect{-1700, -100, -1060, 560}, rect{-1700, -100, -1060, 560}},
		{"removed monitor", rect{0, 0, 1920, 1040}, rect{-1700, -100, -1060, 560}, rect{0, 0, 640, 660}},
		{"small display", rect{0, 0, 600, 500}, rect{300, 200, 940, 860}, rect{0, 0, 600, 500}},
	} {
		t.Run(tt.name, func(t *testing.T) {
			x, y, width, height := keepWindowPlacement(tt.work, tt.bounds.Left, tt.bounds.Top, tt.bounds.Right-tt.bounds.Left, tt.bounds.Bottom-tt.bounds.Top)
			if got := (rect{x, y, x + width, y + height}); got != tt.want {
				t.Fatalf("got %+v, want %+v", got, tt.want)
			}
		})
	}
}

func TestFullscreenMonitorBounds(t *testing.T) {
	monitor := rect{-1920, -200, 0, 880}
	for _, tt := range []struct {
		bounds rect
		want   bool
	}{
		{monitor, true}, {rect{-1921, -201, 1, 881}, true},
		{rect{-1920, -200, 0, 840}, false}, {rect{0, 0, 1920, 1080}, false},
	} {
		if got := coversMonitor(tt.bounds, monitor); got != tt.want {
			t.Errorf("%+v: %v", tt.bounds, got)
		}
	}
	if coversMonitor(rect{}, rect{}) {
		t.Fatal("invalid monitor accepted")
	}
}

func TestWindowPlacementOnSmallAndSecondaryMonitors(t *testing.T) {
	for _, tt := range []struct {
		name          string
		work          rect
		width, height int32
		want          rect
	}{
		{"primary", rect{0, 0, 1920, 1040}, 640, 540, rect{640, 208, 1280, 748}},
		{"small", rect{0, 0, 800, 560}, 640, 620, rect{80, 0, 720, 560}},
		{"left", rect{-1920, 0, 0, 1040}, 640, 540, rect{-1280, 208, -640, 748}},
		{"above", rect{0, -1080, 1920, -40}, 640, 540, rect{640, -872, 1280, -332}},
		{"high dpi", rect{0, 0, 1200, 760}, 1280, 1080, rect{0, 0, 1200, 760}},
	} {
		t.Run(tt.name, func(t *testing.T) {
			x, y, w, h := windowPlacement(tt.work, tt.width, tt.height)
			if got := (rect{x, y, x + w, y + h}); got != tt.want {
				t.Fatalf("got %+v, want %+v", got, tt.want)
			}
		})
	}
}
