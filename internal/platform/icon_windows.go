package platform

import (
	"fmt"
	"image"
	"os"
	"path/filepath"
	"strings"
	"syscall"
	"unsafe"

	"golang.org/x/sys/windows"
	"velo-launcher/internal/model"
)

var shell32 = windows.NewLazySystemDLL("shell32.dll")
var gdi32 = windows.NewLazySystemDLL("gdi32.dll")

// iconSize 是缓存 PNG 的边长。前端按 32 CSS px 显示，高 DPI 缩放后仍需要
// 更大的源图，因此提取边长取 64。
const iconSize = 64

type shellFileInfo struct {
	Icon        uintptr
	Index       int32
	Attributes  uint32
	DisplayName [260]uint16
	TypeName    [80]uint16
}
type bitmapInfo struct {
	Size                   uint32
	Width, Height          int32
	Planes, BitCount       uint16
	Compression, SizeImage uint32
	XPels, YPels           int32
	Used, Important        uint32
}
type bitmap struct {
	Type       int32
	Width      int32
	Height     int32
	WidthBytes int32
	Planes     uint16
	BitsPixel  uint16
	Bits       uintptr
}

var (
	createShellItem    = shell32.NewProc("SHCreateItemFromParsingName")
	shellGetFileInfo   = shell32.NewProc("SHGetFileInfoW")
	createCompatibleDC = gdi32.NewProc("CreateCompatibleDC")
	deleteDC           = gdi32.NewProc("DeleteDC")
	createDIBSection   = gdi32.NewProc("CreateDIBSection")
	selectObject       = gdi32.NewProc("SelectObject")
	deleteObject       = gdi32.NewProc("DeleteObject")
	getDIBits          = gdi32.NewProc("GetDIBits")
	getObject          = gdi32.NewProc("GetObjectW")
	flushGDI           = gdi32.NewProc("GdiFlush")
	drawIconEx         = user32.NewProc("DrawIconEx")
)

// ExtractIcon asks the shell for the sharpest icon it has. IShellItemImageFactory
// covers shortcuts, executables and packaged apps uniformly and renders from the
// best matching frame instead of a fixed 32 px bitmap.
func ExtractIcon(item model.AppItem) (image.Image, error) {
	var reason error
	for _, path := range iconSources(item) {
		img, err := extractIcon(path)
		if err == nil {
			return img, nil
		}
		if reason == nil {
			reason = err
		}
	}
	return nil, fmt.Errorf("图标不可用: %w", reason)
}

// iconSources 优先使用快捷方式的目标程序：Shell 会给 .lnk 叠加"快捷方式
// 箭头"，而启动台显示的是应用本身的图标。
func iconSources(item model.AppItem) []string {
	paths := []string{}
	if strings.EqualFold(filepath.Ext(item.Path), ".lnk") {
		switch exec := item.ExecPath; {
		case strings.HasPrefix(exec, "shell:"):
			paths = append(paths, exec)
		case exec != "":
			if info, err := os.Stat(exec); err == nil && !info.IsDir() {
				paths = append(paths, exec)
			}
		}
	}
	if item.Path != "" {
		paths = append(paths, item.Path)
	}
	return paths
}

func extractIcon(path string) (image.Image, error) {
	img, shellErr := extractShellImage(path, iconSize)
	if shellErr == nil {
		return img, nil
	}
	img, fileErr := extractFileIcon(path, iconSize)
	if fileErr == nil {
		return img, nil
	}
	return nil, fmt.Errorf("%v / %v", shellErr, fileErr)
}

func extractFileIcon(path string, size int) (image.Image, error) {
	var info shellFileInfo
	p, err := windows.UTF16PtrFromString(path)
	if err != nil {
		return nil, err
	}
	ok, _, _ := shellGetFileInfo.Call(uintptr(unsafe.Pointer(p)), 0, uintptr(unsafe.Pointer(&info)), unsafe.Sizeof(info), 0x100) // SHGFI_ICON, large shell icon
	if ok == 0 || info.Icon == 0 {
		return nil, fmt.Errorf("shell icon unavailable")
	}
	defer destroyIcon.Call(info.Icon)
	dc, _, _ := createCompatibleDC.Call(0)
	if dc == 0 {
		return nil, fmt.Errorf("icon DC unavailable")
	}
	defer deleteDC.Call(dc)
	header := bitmapInfo{Size: 40, Width: int32(size), Height: -int32(size), Planes: 1, BitCount: 32}
	var bits unsafe.Pointer
	handle, _, _ := createDIBSection.Call(dc, uintptr(unsafe.Pointer(&header)), 0, uintptr(unsafe.Pointer(&bits)), 0, 0)
	if handle == 0 || bits == nil {
		return nil, fmt.Errorf("icon bitmap unavailable")
	}
	defer deleteObject.Call(handle)
	previous, _, _ := selectObject.Call(dc, handle)
	defer selectObject.Call(dc, previous)
	pixels := unsafe.Slice((*byte)(bits), size*size*4)
	clear(pixels)
	ok, _, _ = drawIconEx.Call(dc, 0, 0, info.Icon, uintptr(size), uintptr(size), 0, 0, 3)
	if ok == 0 {
		return nil, fmt.Errorf("draw icon failed")
	}
	flushGDI.Call()
	output := image.NewNRGBA(image.Rect(0, 0, size, size))
	copy(output.Pix, pixels)
	normalizeBGRA(output)
	if !hasAlpha(output) {
		// 只有掩码的旧式图标：按不透明掩码重绘，避免整幅透明。
		clear(pixels)
		drawIconEx.Call(dc, 0, 0, info.Icon, uintptr(size), uintptr(size), 0, 0, 1)
		flushGDI.Call()
		for n := 3; n < len(pixels); n += 4 {
			if pixels[n-3] == 0 {
				output.Pix[n] = 255
			}
		}
	}
	return output, nil
}

type imageFactory struct{ vtable *imageFactoryVTable }
type imageFactoryVTable struct{ QueryInterface, AddRef, Release, GetImage uintptr }

// Packaged apps expose their icons through IShellItemImageFactory; the older
// SHGetFileInfo path frequently has no HICON for these virtual shell items.
func extractShellImage(path string, size int) (image.Image, error) {
	name, err := windows.UTF16PtrFromString(path)
	if err != nil {
		return nil, err
	}
	iid := windows.GUID{Data1: 0xbcc18b79, Data2: 0xba16, Data3: 0x442f, Data4: [8]byte{0x80, 0xc4, 0x8a, 0x59, 0xc3, 0x0c, 0x46, 0x3b}}
	var factory *imageFactory
	hr, _, _ := createShellItem.Call(uintptr(unsafe.Pointer(name)), 0, uintptr(unsafe.Pointer(&iid)), uintptr(unsafe.Pointer(&factory)))
	if int32(hr) < 0 || factory == nil {
		return nil, fmt.Errorf("shell image factory: %#x", hr)
	}
	defer syscall.SyscallN(factory.vtable.Release, uintptr(unsafe.Pointer(factory)))
	var handle uintptr
	requested := uintptr(uint64(size)<<32 | uint64(size))
	hr, _, _ = syscall.SyscallN(factory.vtable.GetImage, uintptr(unsafe.Pointer(factory)), requested, 4, uintptr(unsafe.Pointer(&handle))) // SIIGBF_ICONONLY
	if int32(hr) < 0 || handle == 0 {
		return nil, fmt.Errorf("shell image: %#x", hr)
	}
	defer deleteObject.Call(handle)
	var dimensions bitmap
	if ok, _, _ := getObject.Call(handle, unsafe.Sizeof(dimensions), uintptr(unsafe.Pointer(&dimensions))); ok == 0 || dimensions.Width <= 0 || dimensions.Height <= 0 {
		return nil, fmt.Errorf("shell image size unavailable")
	}
	width, height := int(dimensions.Width), int(dimensions.Height)
	dc, _, _ := createCompatibleDC.Call(0)
	if dc == 0 {
		return nil, fmt.Errorf("image DC unavailable")
	}
	defer deleteDC.Call(dc)
	header := bitmapInfo{Size: 40, Width: int32(width), Height: -int32(height), Planes: 1, BitCount: 32}
	img := image.NewNRGBA(image.Rect(0, 0, width, height))
	if rows, _, _ := getDIBits.Call(dc, handle, 0, uintptr(height), uintptr(unsafe.Pointer(&img.Pix[0])), uintptr(unsafe.Pointer(&header)), 0); rows == 0 {
		return nil, fmt.Errorf("read shell image failed")
	}
	normalizeBGRA(img)
	return img, nil
}

// normalizeBGRA converts premultiplied BGRA pixels into straight RGBA.
func normalizeBGRA(img *image.NRGBA) {
	for n := 0; n < len(img.Pix); n += 4 {
		img.Pix[n], img.Pix[n+2] = img.Pix[n+2], img.Pix[n]
		a := int(img.Pix[n+3])
		if a > 0 && a < 255 {
			for c := 0; c < 3; c++ {
				img.Pix[n+c] = byte(min(255, int(img.Pix[n+c])*255/a))
			}
		}
	}
}
func hasAlpha(img *image.NRGBA) bool {
	for n := 3; n < len(img.Pix); n += 4 {
		if img.Pix[n] != 0 {
			return true
		}
	}
	return false
}
