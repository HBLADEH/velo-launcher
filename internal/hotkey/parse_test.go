package hotkey

import "testing"

func TestParse(t *testing.T) {
	for _, text := range []string{"Alt+Space", "Ctrl+Alt+K", "Win+Shift+F12"} {
		if _, err := Parse(text); err != nil {
			t.Fatal(text, err)
		}
	}
	for _, text := range []string{"Space", "Ctrl+Ctrl+A", "Ctrl+F25", "Meta+Q", "Alt+", "Shift+Unknown"} {
		if _, err := Parse(text); err == nil {
			t.Fatal("accepted", text)
		}
	}
}
