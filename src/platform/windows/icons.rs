use std::borrow::Cow;
use std::mem::size_of;
use std::ptr::null_mut;
use windows::Win32::Foundation::SIZE as ShellSize;
use windows::Win32::UI::Shell::{
    IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_BIGGERSIZEOK, SIIGBF_ICONONLY,
};
use windows::core::PCWSTR;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::UI::Shell::{SHDefExtractIconW, SHFILEINFOW, SHGFI_ICON, SHGetFileInfoW};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DI_MASK, DI_NORMAL, DestroyIcon, DrawIconEx, GetSystemMetrics, HICON, SM_CXICON,
};

use super::shell::{Apartment, wide};
use crate::core::icons::{Pixels, SIZE, icon_location};
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
        // SAFETY: the extraction API transferred ownership of this HICON.
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
                SIIGBF_ICONONLY | SIIGBF_BIGGERSIZEOK,
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
    let mut info = SHFILEINFOW::default();
    // SAFETY: both buffers live through the call. SHGetFileInfo transfers the icon.
    let found = unsafe {
        SHGetFileInfoW(
            path.as_ptr(),
            0,
            &mut info,
            size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON,
        ) != 0
    };
    if !found || info.hIcon.is_null() {
        return Err(DataError::new("read file icon", "Shell returned no icon"));
    }
    // SHGFI_ICON supplies a large system icon. Keep its original sampling and
    // let the icon worker resample it for the destination.
    // SAFETY: this reads a system metric and does not retain any pointers.
    let edge = unsafe { GetSystemMetrics(SM_CXICON) }.clamp(1, SIZE as i32) as u32;
    paint_icon(Icon(info.hIcon), edge)
}

fn resource_icon(path: &str, index: i32) -> Result<Pixels> {
    let path = wide(&shell_path(path))?;
    let mut handle = null_mut();
    // SAFETY: path and output live through extraction. A returned HICON is owned.
    let result =
        unsafe { SHDefExtractIconW(path.as_ptr(), index, 0, &mut handle, null_mut(), SIZE) };
    let icon = (!handle.is_null()).then(|| Icon(handle));
    windows::core::HRESULT(result)
        .ok()
        .map_err(|error| DataError::new("read icon resource", error))?;
    let icon =
        icon.ok_or_else(|| DataError::new("read icon resource", "Shell returned no icon"))?;
    paint_icon(icon, SIZE)
}

fn paint_icon(icon: Icon, edge: u32) -> Result<Pixels> {
    // SAFETY: HICON/DC/DIB are owned. Restore selection before deleting the
    // DIB. Its buffer is valid until Bitmap drops.
    unsafe {
        let dc = dc()?;
        let mut bits = null_mut();
        let bitmap = Bitmap(CreateDIBSection(
            dc.0,
            &header(edge as i32, edge as i32),
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
        let pixels = std::slice::from_raw_parts_mut(bits.cast::<u8>(), (edge * edge * 4) as usize);
        pixels.fill(0);
        if DrawIconEx(
            dc.0,
            0,
            0,
            icon.0,
            edge as i32,
            edge as i32,
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
                edge as i32,
                edge as i32,
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
            width: edge,
            height: edge,
            rgba,
        })
    }
}

fn shell_path(path: &str) -> Cow<'_, str> {
    // Shell icon APIs reject extended-length spellings for otherwise ordinary
    // paths. Adapt the FFI argument without changing the saved identity.
    if let Some(unc) = path.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{unc}").into()
    } else {
        path.strip_prefix(r"\\?\").unwrap_or(path).into()
    }
}

pub fn extract(_apartment: &Apartment, item: &AppItem) -> Result<Pixels> {
    let (resource, index) = icon_location(&item.icon_path);
    if let Some(index) = index
        && let Ok(pixels) = resource_icon(resource, index)
    {
        return Ok(pixels);
    }
    let image = (!resource.is_empty() && index.is_none()).then_some(resource);
    // A shortcut may specify an app icon even when its target is a launcher or updater.
    let target = (extension(&item.path, "lnk") && item.exec_path != item.path)
        .then_some(item.exec_path.as_str())
        .filter(|path| {
            path.starts_with("shell:") || std::fs::metadata(path).is_ok_and(|m| m.is_file())
        });
    for path in image.into_iter().chain([item.path.as_str()]).chain(target) {
        if path.is_empty() {
            continue;
        }
        let path = shell_path(path);
        if let Ok(pixels) = shell_image(&path).or_else(|_| file_icon(&path)) {
            return Ok(pixels);
        }
    }
    Err(DataError::new(
        "extract application icon",
        "no available Shell icon",
    ))
}
