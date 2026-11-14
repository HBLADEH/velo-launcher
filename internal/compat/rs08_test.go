package compat

import (
	"path/filepath"
	"testing"
)

func TestSharedRS08GoExpectations(t *testing.T) {
	f, err := Load(filepath.Join("..", "..", "tests", "fixtures", "compat", "rs08.json"))
	if err != nil {
		t.Fatal(err)
	}
	if err := Check(f); err != nil {
		t.Fatal(err)
	}
	directory := filepath.Join(t.TempDir(), "go-data")
	if err := Emit(f, directory); err != nil {
		t.Fatal(err)
	}
	if err := Verify(f, directory, false); err != nil {
		t.Fatal(err)
	}
	if err := Rewrite(f, directory); err != nil {
		t.Fatal(err)
	}
	if err := Verify(f, directory, true); err != nil {
		t.Fatal(err)
	}
}
