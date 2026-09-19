package platform

import (
	"fmt"
	"golang.org/x/sys/windows"
	"image"
	"strings"
	"syscall"
	"unsafe"
)

var shell32 = windows.NewLazySystemDLL("shell32.dll")
var gdi32 = windows.NewLazySystemDLL("gdi32.dll")

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

// ExtractIcon asks the local shell for an icon and renders it into a small
// top-down DIB. All native handles are released on the scanning COM thread.
func ExtractIcon(path string) (image.Image, error) {
	if strings.HasPrefix(path, "shell:AppsFolder\\") {
		return extractShellImage(path)
	}
	var info shellFileInfo
	flags := uintptr(0x100) // SHGFI_ICON, large shell icon
	p, err := windows.UTF16PtrFromString(path)
	if err != nil {
		return nil, err
	}
	ok, _, _ := shell32.NewProc("SHGetFileInfoW").Call(uintptr(unsafe.Pointer(p)), 0, uintptr(unsafe.Pointer(&info)), unsafe.Sizeof(info), flags)
	if ok == 0 || info.Icon == 0 {
		return nil, fmt.Errorf("shell icon unavailable")
	}
	defer user32.NewProc("DestroyIcon").Call(info.Icon)
	dc, _, _ := gdi32.NewProc("CreateCompatibleDC").Call(0)
	if dc == 0 {
		return nil, fmt.Errorf("icon DC unavailable")
	}
	defer gdi32.NewProc("DeleteDC").Call(dc)
	header := bitmapInfo{Size: 40, Width: 32, Height: -32, Planes: 1, BitCount: 32}
	var bits unsafe.Pointer
	bitmap, _, _ := gdi32.NewProc("CreateDIBSection").Call(dc, uintptr(unsafe.Pointer(&header)), 0, uintptr(unsafe.Pointer(&bits)), 0, 0)
	if bitmap == 0 || bits == nil {
		return nil, fmt.Errorf("icon bitmap unavailable")
	}
	defer gdi32.NewProc("DeleteObject").Call(bitmap)
	previous, _, _ := gdi32.NewProc("SelectObject").Call(dc, bitmap)
	defer gdi32.NewProc("SelectObject").Call(dc, previous)
	pixels := unsafe.Slice((*byte)(bits), 32*32*4)
	clear(pixels)
	ok, _, _ = user32.NewProc("DrawIconEx").Call(dc, 0, 0, info.Icon, 32, 32, 0, 0, 3)
	if ok == 0 {
		return nil, fmt.Errorf("draw icon failed")
	}
	gdi32.NewProc("GdiFlush").Call()
	output := image.NewNRGBA(image.Rect(0, 0, 32, 32))
	hasAlpha := false
	for n := 0; n < len(pixels); n += 4 {
		b, g, r, a := pixels[n], pixels[n+1], pixels[n+2], pixels[n+3]
		if a != 0 {
			hasAlpha = true
		}
		if a > 0 && a < 255 {
			r = byte(min(255, int(r)*255/int(a)))
			g = byte(min(255, int(g)*255/int(a)))
			b = byte(min(255, int(b)*255/int(a)))
		}
		output.Pix[n] = r
		output.Pix[n+1] = g
		output.Pix[n+2] = b
		output.Pix[n+3] = a
	}
	if !hasAlpha {
		clear(pixels)
		user32.NewProc("DrawIconEx").Call(dc, 0, 0, info.Icon, 32, 32, 0, 0, 1)
		gdi32.NewProc("GdiFlush").Call()
		for n := 0; n < len(pixels); n += 4 {
			if pixels[n] == 0 {
				output.Pix[n+3] = 255
			}
		}
	}
	return output, nil
}

type imageFactory struct{ vtable *imageFactoryVTable }
type imageFactoryVTable struct{ QueryInterface, AddRef, Release, GetImage uintptr }

// Packaged apps expose their icons through IShellItemImageFactory; the older
// SHGetFileInfo path frequently has no HICON for these virtual shell items.
func extractShellImage(path string) (image.Image, error) {
	name, err := windows.UTF16PtrFromString(path)
	if err != nil {
		return nil, err
	}
	iid := windows.GUID{Data1: 0xbcc18b79, Data2: 0xba16, Data3: 0x442f, Data4: [8]byte{0x80, 0xc4, 0x8a, 0x59, 0xc3, 0x0c, 0x46, 0x3b}}
	var factory *imageFactory
	hr, _, _ := shell32.NewProc("SHCreateItemFromParsingName").Call(uintptr(unsafe.Pointer(name)), 0, uintptr(unsafe.Pointer(&iid)), uintptr(unsafe.Pointer(&factory)))
	if int32(hr) < 0 || factory == nil {
		return nil, fmt.Errorf("shell image factory: %#x", hr)
	}
	defer syscall.SyscallN(factory.vtable.Release, uintptr(unsafe.Pointer(factory)))
	var bitmap uintptr
	hr, _, _ = syscall.SyscallN(factory.vtable.GetImage, uintptr(unsafe.Pointer(factory)), uintptr(uint64(32)<<32|32), 4, uintptr(unsafe.Pointer(&bitmap)))
	if int32(hr) < 0 || bitmap == 0 {
		return nil, fmt.Errorf("shell image: %#x", hr)
	}
	defer gdi32.NewProc("DeleteObject").Call(bitmap)
	dc, _, _ := gdi32.NewProc("CreateCompatibleDC").Call(0)
	if dc == 0 {
		return nil, fmt.Errorf("image DC unavailable")
	}
	defer gdi32.NewProc("DeleteDC").Call(dc)
	header := bitmapInfo{Size: 40, Width: 32, Height: -32, Planes: 1, BitCount: 32}
	img := image.NewNRGBA(image.Rect(0, 0, 32, 32))
	rows, _, _ := gdi32.NewProc("GetDIBits").Call(dc, bitmap, 0, 32, uintptr(unsafe.Pointer(&img.Pix[0])), uintptr(unsafe.Pointer(&header)), 0)
	if rows == 0 {
		return nil, fmt.Errorf("read shell image failed")
	}
	for n := 0; n < len(img.Pix); n += 4 {
		img.Pix[n], img.Pix[n+2] = img.Pix[n+2], img.Pix[n]
		a := int(img.Pix[n+3])
		if a > 0 && a < 255 {
			for c := 0; c < 3; c++ {
				img.Pix[n+c] = byte(min(255, int(img.Pix[n+c])*255/a))
			}
		}
	}
	return img, nil
}
