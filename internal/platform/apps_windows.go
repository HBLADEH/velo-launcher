package platform

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
	"syscall"
	"time"

	"github.com/go-ole/go-ole"
	"github.com/go-ole/go-ole/oleutil"
	"golang.org/x/sys/windows"
	"golang.org/x/sys/windows/registry"
	"velo-launcher/internal/config"
	"velo-launcher/internal/indexer"
	"velo-launcher/internal/model"
)

func Roots(c config.Config) []indexer.Root {
	result := []indexer.Root{}
	add := func(id *windows.KNOWNFOLDERID, source string) {
		if path, err := windows.KnownFolderPath(id, 0); err == nil {
			result = append(result, indexer.Root{Path: path, Source: source})
		}
	}
	add(windows.FOLDERID_Programs, "Start Menu")
	add(windows.FOLDERID_CommonPrograms, "Start Menu")
	add(windows.FOLDERID_Desktop, "Desktop")
	add(windows.FOLDERID_PublicDesktop, "Desktop")
	if c.ScanProgramFiles {
		add(windows.FOLDERID_ProgramFiles, "Program Files")
		add(windows.FOLDERID_ProgramFilesX86, "Program Files (x86)")
	}
	for _, dir := range c.CustomDirectories {
		result = append(result, indexer.Root{Path: dir, Source: "Custom"})
	}
	return result
}

type Resolver struct{ shell *ole.IDispatch }

// NewResolver and Close must run on the scanning goroutine. COM is kept on
// one locked OS thread for the whole scan rather than starting a script per link.
func NewResolver() (*Resolver, error) {
	runtime.LockOSThread()
	if err := ole.CoInitializeEx(0, ole.COINIT_APARTMENTTHREADED); err != nil {
		if e, ok := err.(*ole.OleError); !ok || e.Code() != 1 {
			runtime.UnlockOSThread()
			return nil, err
		}
	}
	u, err := oleutil.CreateObject("WScript.Shell")
	if err != nil {
		ole.CoUninitialize()
		runtime.UnlockOSThread()
		return nil, err
	}
	defer u.Release()
	d, err := u.QueryInterface(ole.IID_IDispatch)
	if err != nil {
		ole.CoUninitialize()
		runtime.UnlockOSThread()
		return nil, err
	}
	return &Resolver{d}, nil
}
func (r *Resolver) Close() { r.shell.Release(); ole.CoUninitialize(); runtime.UnlockOSThread() }
func (r *Resolver) Resolve(path string) (model.AppItem, error) {
	v, err := oleutil.CallMethod(r.shell, "CreateShortcut", path)
	if err != nil {
		return model.AppItem{}, err
	}
	defer v.Clear()
	d := v.ToIDispatch()
	if d == nil {
		return model.AppItem{}, fmt.Errorf("invalid shortcut")
	}
	get := func(name string) (string, error) {
		v, err := oleutil.GetProperty(d, name)
		if err != nil {
			return "", err
		}
		defer v.Clear()
		return v.ToString(), nil
	}
	item := model.AppItem{Keywords: []string{}}
	for key, dest := range map[string]*string{"TargetPath": &item.ExecPath, "Arguments": &item.Arguments, "WorkingDirectory": &item.WorkingDirectory, "IconLocation": &item.IconPath, "Description": &item.Description} {
		value, err := get(key)
		if err != nil {
			return item, err
		}
		*dest = value
	}
	return item, nil
}

// WindowsApps enumerates packaged launchable apps via the local Start Menu
// catalog. It does not recursively scan the protected WindowsApps directory.
func WindowsApps(ctx context.Context) ([]model.AppItem, error) {
	ctx, cancel := context.WithTimeout(ctx, 20*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, "powershell.exe", "-NoProfile", "-NonInteractive", "-Command", `[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; @(Get-StartApps | Where-Object { $_.AppID -like '*!*' } | Select-Object Name,AppID) | ConvertTo-Json -Compress`)
	command.SysProcAttr = &syscall.SysProcAttr{HideWindow: true}
	data, err := command.Output()
	if err != nil {
		return nil, fmt.Errorf("Windows Apps: %w", err)
	}
	type record struct {
		Name  string
		AppID string
	}
	var rows []record
	text := strings.TrimSpace(strings.TrimPrefix(string(data), "\ufeff"))
	if text == "" || text == "null" {
		return []model.AppItem{}, nil
	}
	if strings.HasPrefix(text, "{") {
		var row record
		if err = json.Unmarshal([]byte(text), &row); err == nil {
			rows = []record{row}
		}
	} else {
		err = json.Unmarshal([]byte(text), &rows)
	}
	if err != nil {
		return nil, err
	}
	items := make([]model.AppItem, 0, len(rows))
	for _, r := range rows {
		path := "shell:AppsFolder\\" + r.AppID
		items = append(items, model.AppItem{ID: model.Identity(path, ""), Name: r.Name, Path: path, ExecPath: path, Source: "Windows Apps", Keywords: []string{}})
	}
	return items, nil
}

func Launch(item model.AppItem) error {
	// Launch the shortcut itself to preserve shell semantics, arguments, working
	// directory, environment expansion, advertised shortcuts and elevation flags.
	path := item.Path
	if strings.HasPrefix(path, "shell:AppsFolder\\") {
		explorer := filepath.Join(os.Getenv("WINDIR"), "explorer.exe")
		return shellExecute(explorer, path, "")
	}
	if _, err := os.Stat(path); err != nil {
		return fmt.Errorf("应用已移动或删除: %w", err)
	}
	if strings.EqualFold(filepath.Ext(path), ".lnk") {
		return shellExecute(path, "", "")
	}
	return shellExecute(item.ExecPath, item.Arguments, item.WorkingDirectory)
}
func shellExecute(path, args, dir string) error {
	p, err := windows.UTF16PtrFromString(path)
	if err != nil {
		return err
	}
	a, err := windows.UTF16PtrFromString(args)
	if err != nil {
		return err
	}
	d, err := windows.UTF16PtrFromString(dir)
	if err != nil {
		return err
	}
	return windows.ShellExecute(0, nil, p, a, d, 1)
}

func SetLaunchAtStartup(enabled bool) error {
	key, _, err := registry.CreateKey(registry.CURRENT_USER, `Software\Microsoft\Windows\CurrentVersion\Run`, registry.SET_VALUE)
	if err != nil {
		return err
	}
	defer key.Close()
	exe := ""
	if enabled {
		exe, err = os.Executable()
		if err != nil {
			return err
		}
	}
	return setStartupValue(key, enabled, exe)
}

func setStartupValue(key registry.Key, enabled bool, exe string) error {
	if !enabled {
		err := key.DeleteValue("Velo")
		if err == registry.ErrNotExist {
			return nil
		}
		return err
	}
	return key.SetStringValue("Velo", `"`+exe+`" --background`)
}
