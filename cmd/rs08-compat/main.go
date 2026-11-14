// rs08-compat is an offline oracle, not part of the shipped launcher.
package main

import (
	"fmt"
	"os"

	"velo-launcher/internal/compat"
)

func run(args []string) error {
	if len(args) < 2 {
		return fmt.Errorf("usage: rs08-compat check|refresh|emit|verify|verify-edited|rewrite FIXTURE [DIRECTORY]")
	}
	if args[0] == "refresh" {
		return compat.Refresh(args[1])
	}
	f, err := compat.Load(args[1])
	if err != nil {
		return err
	}
	if err := compat.Check(f); err != nil {
		return err
	}
	if args[0] == "check" {
		return nil
	}
	if len(args) != 3 {
		return fmt.Errorf("explicit fixture directory required")
	}
	switch args[0] {
	case "emit":
		return compat.Emit(f, args[2])
	case "verify":
		return compat.Verify(f, args[2], false)
	case "verify-edited":
		return compat.Verify(f, args[2], true)
	case "rewrite":
		return compat.Rewrite(f, args[2])
	default:
		return fmt.Errorf("unknown command")
	}
}

func main() {
	if err := run(os.Args[1:]); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
	fmt.Println("RS-08 Go data contract passed")
}
