// USB Atlas logo: geometry + anti-aliased rasterizer + .ico writer.
// Plain std only: `include!`d by src/app/icon.rs (window icon, header tile)
// and by build.rs (the .exe icon), so every icon comes from one drawing.

const TOP: [f32; 3] = [139.0, 156.0, 255.0]; // #8B9CFF
const BOTTOM: [f32; 3] = [91.0, 75.0, 216.0]; // #5B4BD8

/// Stroke segments of the trident: shaft and the two branches (width 4, round caps).
const SEGMENTS: [((f32, f32), (f32, f32)); 5] = [
    ((13.0, 32.0), (45.0, 32.0)),
    ((25.0, 32.0), (32.0, 21.0)),
    ((32.0, 21.0), (40.0, 21.0)),
    ((19.0, 32.0), (26.0, 43.0)),
    ((26.0, 43.0), (33.0, 43.0)),
];
const STROKE: f32 = 4.0;
/// Filled circles: shaft base and the upper branch tip.
const CIRCLES: [(f32, f32, f32); 2] = [(13.0, 32.0, 5.0), (40.0, 21.0, 4.2)];
/// Arrow head at the end of the shaft.
const ARROW: [(f32, f32); 3] = [(54.0, 32.0), (44.0, 25.5), (44.0, 38.5)];
/// Square at the lower branch tip (center, half size).
const SQUARE: (f32, f32, f32) = (33.5, 43.0, 3.5);

fn seg_dist(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (vx, vy) = (b.0 - a.0, b.1 - a.1);
    let h = (((p.0 - a.0) * vx + (p.1 - a.1) * vy) / (vx * vx + vy * vy)).clamp(0.0, 1.0);
    ((p.0 - a.0 - vx * h).powi(2) + (p.1 - a.1 - vy * h).powi(2)).sqrt()
}

/// Signed distance to a convex polygon (negative inside), clockwise or not.
fn poly_dist(p: (f32, f32), pts: &[(f32, f32)]) -> f32 {
    let n = pts.len();
    // Orientation so that normals point outward.
    let area: f32 = (0..n)
        .map(|i| pts[i].0 * pts[(i + 1) % n].1 - pts[(i + 1) % n].0 * pts[i].1)
        .sum();
    let sign = if area > 0.0 { 1.0 } else { -1.0 };
    let mut d = f32::MIN;
    for i in 0..n {
        let a = pts[i];
        let b = pts[(i + 1) % n];
        let (ex, ey) = (b.0 - a.0, b.1 - a.1);
        let len = (ex * ex + ey * ey).sqrt();
        let (nx, ny) = (sign * ey / len, -sign * ex / len);
        d = d.max((p.0 - a.0) * nx + (p.1 - a.1) * ny);
    }
    d
}

/// Coverage of the white glyph at logo-space point `p`, with `px` logo units per pixel.
fn glyph_coverage(p: (f32, f32), px: f32) -> f32 {
    let aa = |d: f32| (0.5 - d / px).clamp(0.0, 1.0);
    let mut c: f32 = 0.0;
    for (a, b) in SEGMENTS {
        c = c.max(aa(seg_dist(p, a, b) - STROKE / 2.0));
    }
    for (x, y, r) in CIRCLES {
        c = c.max(aa(((p.0 - x).powi(2) + (p.1 - y).powi(2)).sqrt() - r));
    }
    c = c.max(aa(poly_dist(p, &ARROW)));
    let (sx, sy, h) = SQUARE;
    let d = ((p.0 - sx).abs() - h).max((p.1 - sy).abs() - h);
    c.max(aa(d))
}

/// Coverage of the rounded tile (2..62, radius 15).
fn tile_coverage(p: (f32, f32), px: f32) -> f32 {
    let r = 15.0;
    let dx = (p.0 - 32.0).abs() - (30.0 - r);
    let dy = (p.1 - 32.0).abs() - (30.0 - r);
    let d = (dx.max(0.0).powi(2) + dy.max(0.0).powi(2)).sqrt() - r + dx.max(dy).min(0.0);
    (0.5 - d / px).clamp(0.0, 1.0)
}

/// Rasterizes the logo to an `n`×`n` RGBA (unmultiplied) image.
pub fn rasterize(n: usize) -> Vec<u8> {
    let px = 64.0 / n as f32;
    let mut rgba = vec![0u8; n * n * 4];
    for y in 0..n {
        for x in 0..n {
            let p = ((x as f32 + 0.5) * px, (y as f32 + 0.5) * px);
            let tile = tile_coverage(p, px);
            if tile <= 0.0 {
                continue;
            }
            let t = p.1 / 64.0;
            let g = glyph_coverage(p, px);
            let o = (y * n + x) * 4;
            for i in 0..3 {
                let base = TOP[i] * (1.0 - t) + BOTTOM[i] * t;
                rgba[o + i] = (base * (1.0 - g) + 255.0 * g).round() as u8;
            }
            rgba[o + 3] = (tile * 255.0).round() as u8;
        }
    }
    rgba
}

/// Encodes the logo as a multi-size Windows .ico (32-bit DIB entries).
#[allow(dead_code)]
pub fn ico_bytes(sizes: &[usize]) -> Vec<u8> {
    let mut images: Vec<Vec<u8>> = Vec::new();
    for &n in sizes {
        let rgba = rasterize(n);
        let mask_row = n.div_ceil(32) * 4;
        let mut img = Vec::with_capacity(40 + n * n * 4 + mask_row * n);
        // BITMAPINFOHEADER; height is doubled to include the AND mask.
        for v in [40u32, n as u32, (2 * n) as u32] {
            img.extend_from_slice(&v.to_le_bytes());
        }
        img.extend_from_slice(&1u16.to_le_bytes());
        img.extend_from_slice(&32u16.to_le_bytes());
        img.extend_from_slice(&0u32.to_le_bytes());
        img.extend_from_slice(&((n * n * 4 + mask_row * n) as u32).to_le_bytes());
        img.extend_from_slice(&[0u8; 16]);
        // Pixels bottom-up, BGRA.
        for y in (0..n).rev() {
            for x in 0..n {
                let o = (y * n + x) * 4;
                img.extend_from_slice(&[rgba[o + 2], rgba[o + 1], rgba[o], rgba[o + 3]]);
            }
        }
        // AND mask unused with 32-bit alpha.
        img.extend(std::iter::repeat_n(0u8, mask_row * n));
        images.push(img);
    }
    let mut out = Vec::new();
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&(sizes.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * sizes.len();
    for (&n, img) in sizes.iter().zip(&images) {
        let dim = if n >= 256 { 0 } else { n as u8 };
        out.extend_from_slice(&[dim, dim, 0, 0]);
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&32u16.to_le_bytes());
        out.extend_from_slice(&(img.len() as u32).to_le_bytes());
        out.extend_from_slice(&(offset as u32).to_le_bytes());
        offset += img.len();
    }
    for img in images {
        out.extend(img);
    }
    out
}
