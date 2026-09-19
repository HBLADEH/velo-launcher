package platform

import (
	"errors"
	"golang.org/x/sys/windows"
	"unsafe"
)

// AcquireInstance guards data/log files before Wails initializes. A second
// launch raises the existing native window without opening the data files.
func AcquireInstance() (bool, func(), error) {
	name, _ := windows.UTF16PtrFromString(`Local\VeloLauncher-6ab2e7d8`)
	h, err := windows.CreateMutex(nil, false, name)
	if errors.Is(err, windows.ERROR_ALREADY_EXISTS) {
		windows.CloseHandle(h)
		class, _ := windows.UTF16PtrFromString(WindowClass)
		window, _, _ := user32.NewProc("FindWindowW").Call(uintptr(unsafe.Pointer(class)), 0)
		if window != 0 {
			(&DesktopWindow{window}).Show()
		}
		return false, func() {}, nil
	}
	if err != nil {
		return false, nil, err
	}
	return true, func() { windows.CloseHandle(h) }, nil
}
