use std::path::Path;

use egui::ColorImage;
use windows::core::PCWSTR;
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits,
    ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HGDIOBJ,
};
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
            Default::default(),
            Some(&mut shfi),
            std::mem::size_of::<SHFILEINFOW>() as u32,
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
    let dim = size as i32;
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
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: dim,
            biHeight: -dim,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };

    let mut pixels = vec![0u8; (dim * dim * 4) as usize];
    let _ = GetDIBits(
        mem_dc,
        bitmap,
        0,
        size,
        Some(pixels.as_mut_ptr() as *mut _),
        &mut bmi,
        DIB_RGB_COLORS,
    );

    let _ = SelectObject(mem_dc, old);
    let _ = DeleteObject(HGDIOBJ(bitmap.0));
    let _ = DeleteDC(mem_dc);
    let _ = ReleaseDC(None, screen);

    for chunk in pixels.chunks_exact_mut(4) {
        chunk.swap(0, 2);
        let alpha = chunk[3];
        if alpha == 0 && chunk[0] | chunk[1] | chunk[2] != 0 {
            chunk[3] = 255;
        }
    }

    Some(ColorImage::from_rgba_unmultiplied(
        [size as usize, size as usize],
        &pixels,
    ))
}
