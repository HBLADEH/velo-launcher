package logging

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestSessionLogs(t *testing.T) {
	dir := t.TempDir()
	for _, message := range []string{"first session", "second session", "third session"} {
		logger, closeLog, err := Open(dir)
		if err != nil {
			t.Fatal(err)
		}
		logger.Info(message)
		closeLog()
	}
	for file, message := range map[string]string{"velo.log": "third session", "velo.previous.log": "second session"} {
		data, err := os.ReadFile(filepath.Join(dir, "logs", file))
		if err != nil {
			t.Fatal(err)
		}
		if !strings.Contains(string(data), message) {
			t.Fatalf("%s: got %s", file, data)
		}
	}
}

func TestInvalidLogDirectory(t *testing.T) {
	dir := t.TempDir()
	if err := os.WriteFile(filepath.Join(dir, "logs"), []byte("file"), 0600); err != nil {
		t.Fatal(err)
	}
	if _, _, err := Open(dir); err == nil {
		t.Fatal("expected directory error")
	}
}
