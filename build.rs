use image::{Rgba, RgbaImage};

fn main() {
    let icon_path = std::path::Path::new("assets/winharpoon.ico");
    if !icon_path.exists() {
        std::fs::create_dir_all("assets").expect("create assets dir");
        write_icon(icon_path);
    }

    if cfg!(target_os = "windows") {
        winres::WindowsResource::new()
            .set_icon("assets/winharpoon.ico")
            .compile()
            .expect("embed application icon");
    }
}

fn write_icon(path: &std::path::Path) {
    use image::codecs::ico::{IcoEncoder, IcoFrame};
    use image::ExtendedColorType;
    let sizes = [256u32, 48, 32, 16];
    let mut frames = Vec::new();
    for &size in &sizes {
        let img = draw_harpoon(size);
        let frame = IcoFrame::as_png(
            img.as_raw(),
            size,
            size,
            ExtendedColorType::Rgba8,
        )
        .expect("encode ico frame");
        frames.push(frame);
    }

    let file = std::fs::File::create(path).expect("create ico");
    IcoEncoder::new(std::io::BufWriter::new(file))
        .encode_images(&frames)
        .expect("encode ico");
}

fn draw_harpoon(size: u32) -> RgbaImage {
    let mut img = RgbaImage::from_pixel(size, size, Rgba([0, 0, 0, 0]));
    let margin = (size / 8).max(1);
    let bg = Rgba([26, 28, 36, 255]);
    let accent = Rgba([99, 140, 255, 255]);
    let radius = (size / 5).max(2);

    fill_round_rect(&mut img, margin, margin, size - margin, size - margin, radius, bg);

    let cx = size as i32 / 2;
    let cy = size as i32 / 2;
    let head = (size as i32 / 3).max(4);
    let shaft_w = (size as i32 / 10).max(2);

    draw_line(
        &mut img,
        cx - head,
        cy + head / 2,
        cx + head / 2,
        cy - head,
        shaft_w,
        accent,
    );

    let tip_x = cx + head / 2;
    let tip_y = cy - head;
    fill_triangle(
        &mut img,
        &[
            (tip_x, tip_y),
            (tip_x - head / 2, tip_y + head / 3),
            (tip_x - head / 4, tip_y),
        ],
        accent,
    );
    fill_triangle(
        &mut img,
        &[
            (tip_x, tip_y),
            (tip_x - head / 3, tip_y + head / 2),
            (tip_x - head / 6, tip_y),
        ],
        accent,
    );

    img
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
    let w = img.width();
    let h = img.height();
    for y in y0..y1.min(h) {
        for x in x0..x1.min(w) {
            if in_round_rect(x, y, x0, y0, x1, y1, radius) {
                img.put_pixel(x, y, color);
            }
        }
    }
}

fn in_round_rect(x: u32, y: u32, x0: u32, y0: u32, x1: u32, y1: u32, r: u32) -> bool {
    if x < x0 || y < y0 || x >= x1 || y >= y1 {
        return false;
    }
    let r = r as i32;
    let corners = [
        (x0 as i32 + r, y0 as i32 + r),
        (x1 as i32 - r - 1, y0 as i32 + r),
        (x0 as i32 + r, y1 as i32 - r - 1),
        (x1 as i32 - r - 1, y1 as i32 - r - 1),
    ];
    for &(cx, cy) in &corners {
        let dx = x as i32 - cx;
        let dy = y as i32 - cy;
        let in_corner = (x < x0 + r as u32 || x >= x1 - r as u32)
            && (y < y0 + r as u32 || y >= y1 - r as u32);
        if in_corner && dx * dx + dy * dy > r * r {
            return false;
        }
    }
    true
}

fn draw_line(
    img: &mut RgbaImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    width: i32,
    color: Rgba<u8>,
) {
    let steps = ((x1 - x0).abs() + (y1 - y0).abs()).max(1);
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let x = (x0 as f32 + (x1 - x0) as f32 * t).round() as i32;
        let y = (y0 as f32 + (y1 - y0) as f32 * t).round() as i32;
        fill_disk(img, x, y, width / 2, color);
    }
}

fn fill_disk(img: &mut RgbaImage, cx: i32, cy: i32, r: i32, color: Rgba<u8>) {
    let w = img.width() as i32;
    let h = img.height() as i32;
    for dy in -r..=r {
        for dx in -r..=r {
            if dx * dx + dy * dy <= r * r {
                let x = cx + dx;
                let y = cy + dy;
                if x >= 0 && y >= 0 && x < w && y < h {
                    img.put_pixel(x as u32, y as u32, color);
                }
            }
        }
    }
}

fn fill_triangle(img: &mut RgbaImage, pts: &[(i32, i32)], color: Rgba<u8>) {
    let min_y = pts.iter().map(|p| p.1).min().unwrap();
    let max_y = pts.iter().map(|p| p.1).max().unwrap();
    let w = img.width() as i32;
    let h = img.height() as i32;
    for y in min_y..=max_y {
        if y < 0 || y >= h {
            continue;
        }
        for x in 0..w {
            if point_in_triangle(x, y, pts[0], pts[1], pts[2]) {
                img.put_pixel(x as u32, y as u32, color);
            }
        }
    }
}

fn point_in_triangle(px: i32, py: i32, a: (i32, i32), b: (i32, i32), c: (i32, i32)) -> bool {
    fn sign(px: i32, py: i32, ax: i32, ay: i32, bx: i32, by: i32) -> i32 {
        (px - bx) * (ay - by) - (ax - bx) * (py - by)
    }
    let d1 = sign(px, py, a.0, a.1, b.0, b.1);
    let d2 = sign(px, py, b.0, b.1, c.0, c.1);
    let d3 = sign(px, py, c.0, c.1, a.0, a.1);
    let has_neg = (d1 < 0) || (d2 < 0) || (d3 < 0);
    let has_pos = (d1 > 0) || (d2 > 0) || (d3 > 0);
    !(has_neg && has_pos)
}
