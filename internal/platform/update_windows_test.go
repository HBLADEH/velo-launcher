package platform

import (
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func TestRunDetachedScriptExecutes(t *testing.T) {
	dir := t.TempDir()
	marker := filepath.Join(dir, "started.txt")
	script := filepath.Join(dir, "probe.ps1")
	if err := WriteScript(script, "[IO.File]::WriteAllText("+powerShellLiteral(marker)+", 'started')"); err != nil {
		t.Fatal(err)
	}
	if err := RunDetachedScript(script); err != nil {
		t.Fatal(err)
	}
	deadline := time.Now().Add(10 * time.Second)
	for time.Now().Before(deadline) {
		if data, err := os.ReadFile(marker); err == nil && string(data) == "started" {
			return
		}
		time.Sleep(100 * time.Millisecond)
	}
	t.Fatal("PowerShell started but did not execute the script")
}

// Use a short-lived parent to verify the updater survives the launcher exiting.
func TestRunDetachedScriptSurvivesParentExit(t *testing.T) {
	if path := os.Getenv("VELO_UPDATE_TEST_SCRIPT"); path != "" {
		if err := RunDetachedScript(path); err != nil {
			t.Fatal(err)
		}
		return
	}
	dir := t.TempDir()
	marker := filepath.Join(dir, "after-exit.txt")
	script := filepath.Join(dir, "probe.ps1")
	if err := WriteScript(script, "Start-Sleep -Seconds 1\n[IO.File]::WriteAllText("+powerShellLiteral(marker)+", 'started')"); err != nil {
		t.Fatal(err)
	}
	child := exec.Command(os.Args[0], "-test.run=^TestRunDetachedScriptSurvivesParentExit$")
	child.Env = append(os.Environ(), "VELO_UPDATE_TEST_SCRIPT="+script)
	if output, err := child.CombinedOutput(); err != nil {
		t.Fatalf("parent: %v: %s", err, output)
	}
	deadline := time.Now().Add(10 * time.Second)
	for time.Now().Before(deadline) {
		if _, err := os.Stat(marker); err == nil {
			return
		}
		time.Sleep(100 * time.Millisecond)
	}
	t.Fatal("update script did not survive parent exit")
}

func TestInstallScriptHandlesInstallerResult(t *testing.T) {
	for _, result := range []string{"0", "42", "cancelled"} {
		t.Run(result, func(t *testing.T) {
			dir := t.TempDir()
			installer := filepath.Join(dir, "installer.exe")
			script := filepath.Join(dir, "install.ps1")
			target := filepath.Join(dir, "custom install", "velo.exe")
			if err := os.WriteFile(installer, []byte("placeholder"), 0600); err != nil {
				t.Fatal(err)
			}
			// Stub process APIs so this exercises PowerShell control flow without
			// starting an installer, requesting elevation, or restarting Velo.
			stub := "function Get-Process { return $null }\n" +
				"function Start-Process { param($FilePath, $ArgumentList, $Verb, [switch]$Wait, [switch]$PassThru, $ErrorAction)\n" +
				" if ($Verb -eq 'RunAs') {\n" +
				"  if ($ArgumentList -ne " + powerShellLiteral("/S /D="+filepath.Dir(target)) + ") { throw 'wrong install directory' }\n"
			if result == "cancelled" {
				stub += "  throw 'UAC cancelled'\n"
			} else {
				stub += "  return [pscustomobject]@{ExitCode=" + result + "}\n"
			}
			stub += " }\n}\n"
			if err := WriteScript(script, stub+InstallScript(target, installer)); err != nil {
				t.Fatal(err)
			}
			output, err := exec.Command("powershell.exe", "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File", script).CombinedOutput()
			if result == "0" {
				if err != nil {
					t.Fatalf("%v: %s", err, output)
				}
				for _, path := range []string{installer, script} {
					if _, err := os.Stat(path); !os.IsNotExist(err) {
						t.Fatalf("successful install left %s", path)
					}
				}
			} else {
				if err == nil {
					t.Fatal("installer failure reported success")
				}
				for _, path := range []string{installer, script} {
					if _, err := os.Stat(path); err != nil {
						t.Fatalf("failed install must preserve %s: %v", path, err)
					}
				}
				log, err := os.ReadFile(script + ".log")
				if err != nil || !strings.Contains(string(log), map[string]string{"42": "42", "cancelled": "UAC cancelled"}[result]) {
					t.Fatalf("missing failure details: %s, %v", log, err)
				}
			}
		})
	}
}
