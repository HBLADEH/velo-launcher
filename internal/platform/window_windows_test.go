package platform

import "testing"

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
