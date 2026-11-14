// rs09-tables freezes the Go baseline's Unicode rules and first pronunciations.
// Run from the repository root; these are build inputs, never a runtime service.
package main

import (
	"bytes"
	"encoding/binary"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"unicode"

	"github.com/mozillazg/go-pinyin"
)

func generate() (map[string][]byte, error) {
	files := map[string][]byte{}
	var lower bytes.Buffer
	for r := rune(0); r <= unicode.MaxRune; r++ {
		if c := unicode.ToLower(r); c != r {
			_ = binary.Write(&lower, binary.LittleEndian, uint32(r))
			_ = binary.Write(&lower, binary.LittleEndian, uint32(c))
		}
	}
	files["lower.bin"] = lower.Bytes()
	for name, predicate := range map[string]func(rune) bool{
		"word": func(r rune) bool { return unicode.IsLetter(r) || unicode.IsDigit(r) },
		"han":  func(r rune) bool { return unicode.Is(unicode.Han, r) },
	} {
		var ranges bytes.Buffer
		for r := rune(0); r <= unicode.MaxRune; r++ {
			if !predicate(r) {
				continue
			}
			start := r
			for r < unicode.MaxRune && predicate(r+1) {
				r++
			}
			_ = binary.Write(&ranges, binary.LittleEndian, uint32(start))
			_ = binary.Write(&ranges, binary.LittleEndian, uint32(r))
		}
		files[name+".bin"] = ranges.Bytes()
	}
	keys := make([]int, 0, len(pinyin.PinyinDict))
	for r := range pinyin.PinyinDict {
		keys = append(keys, r)
	}
	sort.Ints(keys)
	var index, syllables bytes.Buffer
	offsets := map[string]uint32{}
	for _, key := range keys {
		parts := pinyin.SinglePinyin(rune(key), pinyin.NewArgs())
		if len(parts) == 0 {
			return nil, fmt.Errorf("empty dictionary pronunciation")
		}
		part := parts[0]
		offset, exists := offsets[part]
		if !exists {
			offset = uint32(syllables.Len())
			offsets[part] = offset
			syllables.WriteString(part)
			syllables.WriteByte(0)
		}
		_ = binary.Write(&index, binary.LittleEndian, uint32(key))
		_ = binary.Write(&index, binary.LittleEndian, offset)
	}
	files["pinyin.bin"], files["syllables.bin"] = index.Bytes(), syllables.Bytes()
	files["version.txt"] = []byte(fmt.Sprintf("Go Unicode %s; go-pinyin %s; Normal; Heteronym=false; first pronunciation\n", unicode.Version, pinyin.Version))
	return files, nil
}

func main() {
	if len(os.Args) != 2 || (os.Args[1] != "write" && os.Args[1] != "check") {
		panic("usage: go run ./cmd/rs09-tables write|check")
	}
	files, err := generate()
	if err != nil {
		panic(err)
	}
	root := "rust/src/core/search_data"
	for name, data := range files {
		path := filepath.Join(root, name)
		if os.Args[1] == "write" {
			if err := os.MkdirAll(root, 0755); err != nil {
				panic(err)
			}
			if err := os.WriteFile(path, data, 0644); err != nil {
				panic(err)
			}
		} else if old, err := os.ReadFile(path); err != nil || !bytes.Equal(old, data) {
			panic("stale search table: " + name)
		}
	}
	fmt.Println("RS-09 Unicode and pinyin tables verified")
}
