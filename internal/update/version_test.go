package update

import "testing"

func TestVersionComparisonUsesNumbersAndPrerelease(t *testing.T) {
	cases := []struct {
		newer string
		older string
	}{
		{"0.9.0", "0.8.0-beta.2"},
		{"0.10.0", "0.9.0"},
		{"0.8.0", "0.8.0-beta.2"},
		{"0.8.0-beta.2", "0.8.0-beta.1"},
		{"0.8.0-beta.10", "0.8.0-beta.2"},
		{"0.8.0-beta.2", "0.8.0-alpha.9"},
		{"0.8.0-rc.1", "0.8.0-beta.9"},
		{"1.0.0", "0.9.9"},
		{"0.9.1", "0.9.0"},
	}
	for _, c := range cases {
		newer, err := ParseVersion(c.newer)
		if err != nil {
			t.Fatalf("解析 %q: %v", c.newer, err)
		}
		older, err := ParseVersion(c.older)
		if err != nil {
			t.Fatalf("解析 %q: %v", c.older, err)
		}
		if !newer.Newer(older) {
			t.Fatalf("%s 应比 %s 新", c.newer, c.older)
		}
		if older.Newer(newer) || older.Compare(newer) >= 0 {
			t.Fatalf("%s 不应比 %s 新", c.older, c.newer)
		}
	}
}

func TestParseVersionAcceptsReleaseTags(t *testing.T) {
	for text, want := range map[string]string{
		"v0.9.0":         "0.9.0",
		"V0.9.0":         "0.9.0",
		" 0.9 ":          "0.9.0",
		"1.2.3+build.7":  "1.2.3",
		"0.8.0-beta.2":   "0.8.0-beta.2",
		"0.8.0-RC.1":     "0.8.0-RC.1",
		"0.9.0-beta.0":   "0.9.0-beta.0",
		"2":              "2.0.0",
		"v0.10.0-beta.1": "0.10.0-beta.1",
	} {
		parsed, err := ParseVersion(text)
		if err != nil {
			t.Fatalf("解析 %q: %v", text, err)
		}
		if parsed.String() != want {
			t.Fatalf("解析 %q = %q，想要 %q", text, parsed.String(), want)
		}
	}
}

func TestParseVersionRejectsGarbage(t *testing.T) {
	for _, text := range []string{"", "   ", "abc", "0.9.x", "0.9.0-", "-1.0.0", "0.9.0-beta..2", "1.2.3.4"} {
		if parsed, err := ParseVersion(text); err == nil {
			t.Fatalf("%q 应解析失败，得到 %q", text, parsed.String())
		}
	}
}
