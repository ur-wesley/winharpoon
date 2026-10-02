//! Build script: icon generation + Windows resources.

use image::{Rgba, RgbaImage};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let icon_path = std::path::Path::new("assets/winharpoon.ico");
    if !icon_path.exists() {
        std::fs::create_dir_all("assets")?;
        write_icon(icon_path)?;
    }

    if cfg!(target_os = "windows") {
        winres::WindowsResource::new()
            .set_icon("assets/winharpoon.ico")
            .compile()?;
    }
    Ok(())
}

/// # Errors
///
/// Returns an error if icon frames cannot be encoded or written.
fn write_icon(path: &std::path::Path) -> Result<(), Box<dyn Error>> {
    use image::codecs::ico::{IcoEncoder, IcoFrame};
    use image::ExtendedColorType;
    let sizes = [256_u32, 48, 32, 16];
    let mut frames = Vec::new();
    for size in sizes {
        let img = draw_harpoon(size)?;
        let frame = IcoFrame::as_png(img.as_raw(), size, size, ExtendedColorType::Rgba8)?;
        frames.push(frame);
    }

    let file = std::fs::File::create(path)?;
    IcoEncoder::new(std::io::BufWriter::new(file)).encode_images(&frames)?;
    Ok(())
}

/// # Errors
///
/// Returns an error if icon dimensions do not fit in `i32`.
fn draw_harpoon(size: u32) -> Result<RgbaImage, Box<dyn Error>> {
    let mut img = RgbaImage::from_pixel(size, size, Rgba([0, 0, 0, 0]));
    let margin = size.saturating_div(8).max(1);
    let bg = Rgba([26, 28, 36, 255]);
    let accent = Rgba([99, 140, 255, 255]);
    let radius = size.saturating_div(5).max(2);

    fill_round_rect(
        &mut img,
        margin,
        margin,
        size.saturating_sub(margin),
        size.saturating_sub(margin),
        radius,
        bg,
    );

    let size_i32 = i32::try_from(size)?;
    let center = size_i32.saturating_div(2);
    let head = size_i32.saturating_div(3).max(4);
    let shaft_w = size_i32.saturating_div(10).max(2);
    let half_head = head.saturating_div(2);

    draw_line(
        &mut img,
        center.saturating_sub(head),
        center.saturating_add(half_head),
        center.saturating_add(half_head),
        center.saturating_sub(head),
        shaft_w,
        accent,
    );

    let tip_x = center.saturating_add(half_head);
    let tip_y = center.saturating_sub(head);
    fill_triangle(
        &mut img,
        &[
            (tip_x, tip_y),
            (
                tip_x.saturating_sub(half_head),
                tip_y.saturating_add(head.saturating_div(3)),
            ),
            (tip_x.saturating_sub(head.saturating_div(4)), tip_y),
        ],
        accent,
    );
    fill_triangle(
        &mut img,
        &[
            (tip_x, tip_y),
            (
                tip_x.saturating_sub(head.saturating_div(3)),
                tip_y.saturating_add(half_head),
            ),
            (tip_x.saturating_sub(head.saturating_div(6)), tip_y),
        ],
        accent,
    );

    Ok(img)
}

fn fill_round_rect(
    img: &mut RgbaImage,
    x0: u32,
    y0: u32,
    x1: u32,
    y1: u32,
    radius: u32,
    color: Rgba<u8>,
) {
    let width = img.width();
    let height = img.height();
    for y in y0..y1.min(height) {
        for x in x0..x1.min(width) {
            if in_round_rect(x, y, x0, y0, x1, y1, radius) {
                img.put_pixel(x, y, color);
            }
        }
    }
}

fn in_round_rect(x: u32, y: u32, x0: u32, y0: u32, x1: u32, y1: u32, radius: u32) -> bool {
    if x < x0 || y < y0 || x >= x1 || y >= y1 {
        return false;
    }
    let (Ok(x0i), Ok(y0i), Ok(x1i), Ok(y1i), Ok(r)) = (
        i32::try_from(x0),
        i32::try_from(y0),
        i32::try_from(x1),
        i32::try_from(y1),
        i32::try_from(radius),
    ) else {
        return true;
    };
    let (Ok(xi), Ok(yi)) = (i32::try_from(x), i32::try_from(y)) else {
        return false;
    };
    let corners = [
        (x0i.saturating_add(r), y0i.saturating_add(r)),
        (
            x1i.saturating_sub(r).saturating_sub(1),
            y0i.saturating_add(r),
        ),
        (
            x0i.saturating_add(r),
            y1i.saturating_sub(r).saturating_sub(1),
        ),
        (
            x1i.saturating_sub(r).saturating_sub(1),
            y1i.saturating_sub(r).saturating_sub(1),
        ),
    ];
    for (cx, cy) in corners {
        let dx = xi.saturating_sub(cx);
        let dy = yi.saturating_sub(cy);
        let in_corner = (x < x0.saturating_add(radius) || x >= x1.saturating_sub(radius))
            && (y < y0.saturating_add(radius) || y >= y1.saturating_sub(radius));
        if in_corner
            && dx.saturating_mul(dx).saturating_add(dy.saturating_mul(dy)) > r.saturating_mul(r)
        {
            return false;
        }
    }
    true
}

/// Integer Bresenham line; no float math needed for icon pixels.
fn draw_line(img: &mut RgbaImage, x0: i32, y0: i32, x1: i32, y1: i32, width: i32, color: Rgba<u8>) {
    let dx = x1.saturating_sub(x0).abs();
    let dy = y0.saturating_sub(y1).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx.saturating_sub(dy);
    let (mut x, mut y) = (x0, y0);
    let radius = width.saturating_div(2);
    loop {
        fill_disk(img, x, y, radius, color);
        if x == x1 && y == y1 {
            break;
        }
        let err2 = err.saturating_mul(2);
        if err2.saturating_add(dy) > 0 {
            err = err.saturating_sub(dx);
            x = x.saturating_add(sx);
        }
        if err2 < dx {
            err = err.saturating_add(dy);
            y = y.saturating_add(sy);
        }
    }
}

fn fill_disk(img: &mut RgbaImage, cx: i32, cy: i32, radius: i32, color: Rgba<u8>) {
    let (Ok(width), Ok(height)) = (i32::try_from(img.width()), i32::try_from(img.height())) else {
        return;
    };
    let radius_sq = radius.saturating_mul(radius);
    for dy in radius.saturating_neg()..=radius {
        for dx in radius.saturating_neg()..=radius {
            if dx.saturating_mul(dx).saturating_add(dy.saturating_mul(dy)) <= radius_sq {
                let x = cx.saturating_add(dx);
                let y = cy.saturating_add(dy);
                if x >= 0 && y >= 0 && x < width && y < height {
                    let (Ok(ux), Ok(uy)) = (u32::try_from(x), u32::try_from(y)) else {
                        continue;
                    };
                    img.put_pixel(ux, uy, color);
                }
            }
        }
    }
}

fn fill_triangle(img: &mut RgbaImage, points: &[(i32, i32); 3], color: Rgba<u8>) {
    let [a, b, c] = *points;
    let min_y = a.1.min(b.1).min(c.1);
    let max_y = a.1.max(b.1).max(c.1);
    let (Ok(width), Ok(height)) = (i32::try_from(img.width()), i32::try_from(img.height())) else {
        return;
    };
    for y in min_y..=max_y {
        if y < 0 || y >= height {
            continue;
        }
        for x in 0..width {
            if point_in_triangle(x, y, a, b, c) {
                let (Ok(ux), Ok(uy)) = (u32::try_from(x), u32::try_from(y)) else {
                    continue;
                };
                img.put_pixel(ux, uy, color);
            }
        }
    }
}

fn point_in_triangle(px: i32, py: i32, a: (i32, i32), b: (i32, i32), c: (i32, i32)) -> bool {
    fn sign(px: i32, py: i32, ax: i32, ay: i32, bx: i32, by: i32) -> i32 {
        px.saturating_sub(bx)
            .saturating_mul(ay.saturating_sub(by))
            .saturating_sub(ax.saturating_sub(bx).saturating_mul(py.saturating_sub(by)))
    }
    let d1 = sign(px, py, a.0, a.1, b.0, b.1);
    let d2 = sign(px, py, b.0, b.1, c.0, c.1);
    let d3 = sign(px, py, c.0, c.1, a.0, a.1);
    let has_neg = (d1 < 0) || (d2 < 0) || (d3 < 0);
    let has_pos = (d1 > 0) || (d2 > 0) || (d3 > 0);
    !(has_neg && has_pos)
}
