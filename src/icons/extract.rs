use std::path::Path;

use egui::ColorImage;
use windows::core::PCWSTR;
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits,
    ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HGDIOBJ,
};
use windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES;
use windows::Win32::UI::Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON};
use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, DrawIconEx, DI_NORMAL, HICON};

use crate::util;

pub fn extract_file_icon(path: &Path, size: u32) -> Option<ColorImage> {
    if path.as_os_str().is_empty() {
        return None;
    }
    let wide = util::wide(&path.to_string_lossy());
    let mut shfi = SHFILEINFOW::default();
    unsafe {
        let _ = SHGetFileInfoW(
            PCWSTR(wide.as_ptr()),
            FILE_FLAGS_AND_ATTRIBUTES::default(),
            Some(&mut shfi),
            crate::win_cast::size_of_u32::<SHFILEINFOW>(),
            SHGFI_ICON | SHGFI_LARGEICON,
        );
        if shfi.hIcon.0.is_null() {
            return None;
        }
        let image = icon_to_color_image(shfi.hIcon, size);
        let _ = DestroyIcon(shfi.hIcon);
        image
    }
}

unsafe fn icon_to_color_image(icon: HICON, size: u32) -> Option<ColorImage> {
    let dim = i32::try_from(size).unwrap_or(0);
    let screen = GetDC(None);
    if screen.0.is_null() {
        return None;
    }

    let mem_dc = CreateCompatibleDC(Some(screen));
    if mem_dc.0.is_null() {
        let _ = ReleaseDC(None, screen);
        return None;
    }

    let bitmap = CreateCompatibleBitmap(screen, dim, dim);
    if bitmap.0.is_null() {
        let _ = DeleteDC(mem_dc);
        let _ = ReleaseDC(None, screen);
        return None;
    }

    let old = SelectObject(mem_dc, HGDIOBJ(bitmap.0));
    let _ = DrawIconEx(mem_dc, 0, 0, icon, dim, dim, 0, None, DI_NORMAL);

    let mut bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: crate::win_cast::size_of_u32::<BITMAPINFOHEADER>(),
            biWidth: dim,
            biHeight: dim.saturating_neg(),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };

    let pixel_count = usize::try_from(dim.saturating_mul(dim).saturating_mul(4)).unwrap_or(0);
    let mut pixels = vec![0u8; pixel_count];
    let _ = GetDIBits(
        mem_dc,
        bitmap,
        0,
        size,
        Some(pixels.as_mut_ptr().cast::<core::ffi::c_void>()),
        &mut bmi,
        DIB_RGB_COLORS,
    );

    let _ = SelectObject(mem_dc, old);
    let _ = DeleteObject(HGDIOBJ(bitmap.0));
    let _ = DeleteDC(mem_dc);
    let _ = ReleaseDC(None, screen);

    for chunk in pixels.chunks_exact_mut(4) {
        let Some([b, g, r, a]) = chunk.first_chunk_mut::<4>() else {
            continue;
        };
        std::mem::swap(b, r);
        if *a == 0 && *b | *g | *r != 0 {
            *a = 255;
        }
    }

    let dim_usize = usize::try_from(size).unwrap_or(0);
    Some(ColorImage::from_rgba_unmultiplied(
        [dim_usize, dim_usize],
        &pixels,
    ))
}
