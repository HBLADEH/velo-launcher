use std::borrow::Cow;
use std::mem::size_of;
use std::ptr::null_mut;
use windows::Win32::Foundation::SIZE as ShellSize;
use windows::Win32::UI::Shell::{
    IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_ICONONLY,
};
use windows::core::PCWSTR;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::UI::Shell::{SHFILEINFOW, SHGFI_ICON, SHGetFileInfoW};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DI_MASK, DI_NORMAL, DestroyIcon, DrawIconEx, HICON,
};

use super::shell::{Apartment, wide};
use crate::core::icons::{Pixels, SIZE};
use crate::core::indexer::extension;
use crate::core::model::AppItem;
use crate::core::{DataError, Result};

struct Bitmap(HBITMAP);
impl Drop for Bitmap {
    fn drop(&mut self) {
        // SAFETY: this wrapper owns the bitmap; selections are restored first.
        unsafe {
            DeleteObject(self.0);
        }
    }
}
struct Dc(HDC);
impl Drop for Dc {
    fn drop(&mut self) {
        // SAFETY: owned memory DC, released on its creating worker.
        unsafe {
            DeleteDC(self.0);
        }
    }
}
struct Icon(HICON);
impl Drop for Icon {
    fn drop(&mut self) {
        // SAFETY: SHGetFileInfo transferred ownership of this HICON.
        unsafe {
            DestroyIcon(self.0);
        }
    }
}
struct Selected(HDC, HGDIOBJ);
impl Drop for Selected {
    fn drop(&mut self) {
        // SAFETY: DC and previous selection outlive this guard.
        unsafe {
            SelectObject(self.0, self.1);
        }
    }
}

fn dc() -> Result<Dc> {
    // SAFETY: owned compatible memory DC, released by Dc on this worker.
    let handle = unsafe { CreateCompatibleDC(null_mut()) };
    if handle.is_null() {
        return Err(DataError::new(
            "create icon DC",
            std::io::Error::last_os_error(),
        ));
    }
    Ok(Dc(handle))
}

fn header(width: i32, height: i32) -> BITMAPINFO {
    BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            ..Default::default()
        },
        ..Default::default()
    }
}

fn straight_rgba(bytes: &mut [u8]) {
    for pixel in bytes.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
        let alpha = u32::from(pixel[3]);
        if alpha > 0 && alpha < 255 {
            for channel in &mut pixel[..3] {
                *channel = (u32::from(*channel) * 255 / alpha).min(255) as u8;
            }
        }
    }
}

fn shell_image(path: &str) -> Result<Pixels> {
    let path = wide(path)?;
    // SAFETY: caller holds the STA. Shell bitmap ownership passes to Bitmap;
    // GetDIBits reads an unselected HBITMAP into a dimension-checked buffer.
    unsafe {
        let factory: IShellItemImageFactory =
            SHCreateItemFromParsingName(PCWSTR(path.as_ptr()), None)
                .map_err(|e| DataError::new("open Shell icon factory", e))?;
        let bitmap = factory
            .GetImage(
                ShellSize {
                    cx: SIZE as i32,
                    cy: SIZE as i32,
                },
                SIIGBF_ICONONLY,
            )
            .map_err(|e| DataError::new("extract Shell icon", e))?;
        let bitmap = Bitmap(bitmap.0);
        let mut dimensions = BITMAP::default();
        if GetObjectW(
            bitmap.0,
            size_of::<BITMAP>() as i32,
            (&mut dimensions as *mut BITMAP).cast(),
        ) == 0
            || dimensions.bmWidth <= 0
            || dimensions.bmHeight <= 0
            || dimensions.bmWidth > 256
            || dimensions.bmHeight > 256
        {
            return Err(DataError::new(
                "read icon dimensions",
                "invalid Shell bitmap",
            ));
        }
        let width = dimensions.bmWidth;
        let height = dimensions.bmHeight;
        let mut rgba = vec![0; (width * height * 4) as usize];
        let dc = dc()?;
        if GetDIBits(
            dc.0,
            bitmap.0,
            0,
            height as u32,
            rgba.as_mut_ptr().cast(),
            &mut header(width, height),
            DIB_RGB_COLORS,
        ) != height
        {
            return Err(DataError::new(
                "read Shell icon pixels",
                std::io::Error::last_os_error(),
            ));
        }
        straight_rgba(&mut rgba);
        Ok(Pixels {
            width: width as u32,
            height: height as u32,
            rgba,
        })
    }
}

fn file_icon(path: &str) -> Result<Pixels> {
    let path = wide(path)?;
    // SAFETY: HICON/DC/DIB are owned and RAII-released. Selection is restored
    // before deleting the DIB. The DIB buffer is valid until Bitmap drops.
    unsafe {
        let mut info = SHFILEINFOW::default();
        if SHGetFileInfoW(
            path.as_ptr(),
            0,
            &mut info,
            size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON,
        ) == 0
            || info.hIcon.is_null()
        {
            return Err(DataError::new("read file icon", "Shell returned no icon"));
        }
        let icon = Icon(info.hIcon);
        let dc = dc()?;
        let mut bits = null_mut();
        let bitmap = Bitmap(CreateDIBSection(
            dc.0,
            &header(SIZE as i32, SIZE as i32),
            DIB_RGB_COLORS,
            &mut bits,
            null_mut(),
            0,
        ));
        if bitmap.0.is_null() || bits.is_null() {
            return Err(DataError::new(
                "create icon bitmap",
                std::io::Error::last_os_error(),
            ));
        }
        let previous = SelectObject(dc.0, bitmap.0);
        if previous.is_null() || previous as isize == -1 {
            return Err(DataError::new(
                "select icon bitmap",
                std::io::Error::last_os_error(),
            ));
        }
        let selected = Selected(dc.0, previous);
        let pixels = std::slice::from_raw_parts_mut(bits.cast::<u8>(), (SIZE * SIZE * 4) as usize);
        pixels.fill(0);
        if DrawIconEx(
            dc.0,
            0,
            0,
            icon.0,
            SIZE as i32,
            SIZE as i32,
            0,
            null_mut(),
            DI_NORMAL,
        ) == 0
        {
            return Err(DataError::new(
                "draw file icon",
                std::io::Error::last_os_error(),
            ));
        }
        GdiFlush();
        let mut rgba = pixels.to_vec();
        straight_rgba(&mut rgba);
        if !rgba.as_chunks::<4>().0.iter().any(|p| p[3] > 0) {
            pixels.fill(0);
            if DrawIconEx(
                dc.0,
                0,
                0,
                icon.0,
                SIZE as i32,
                SIZE as i32,
                0,
                null_mut(),
                DI_MASK,
            ) == 0
            {
                return Err(DataError::new(
                    "read legacy icon mask",
                    std::io::Error::last_os_error(),
                ));
            }
            GdiFlush();
            for (color, mask) in rgba
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
                .zip(pixels.as_chunks::<4>().0)
            {
                if mask[0] == 0 {
                    color[3] = 255;
                }
            }
        }
        drop(selected);
        Ok(Pixels {
            width: SIZE,
            height: SIZE,
            rgba,
        })
    }
}

pub fn extract(_apartment: &Apartment, item: &AppItem) -> Result<Pixels> {
    let mut sources = Vec::new();
    if extension(&item.path, "lnk")
        && (item.exec_path.starts_with("shell:")
            || std::fs::metadata(&item.exec_path).is_ok_and(|m| m.is_file()))
    {
        sources.push(item.exec_path.as_str());
    }
    if !item.path.is_empty() {
        sources.push(&item.path);
    }
    for path in sources {
        // Imported files use canonical Win32 paths. Shell icon APIs reject the
        // extended-length spelling even for ordinary short files; adapt only
        // this FFI argument, preserving the saved identity and cache signature.
        let path: Cow<'_, str> = if let Some(unc) = path.strip_prefix(r"\\?\UNC\") {
            format!(r"\\{unc}").into()
        } else {
            path.strip_prefix(r"\\?\").unwrap_or(path).into()
        };
        if let Ok(pixels) = shell_image(&path).or_else(|_| file_icon(&path)) {
            return Ok(pixels);
        }
    }
    Err(DataError::new(
        "extract application icon",
        "no available Shell icon",
    ))
}
