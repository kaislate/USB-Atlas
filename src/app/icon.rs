//! Procedurally drawn window icon (rounded tile with a USB-tree glyph).

pub fn app_icon() -> egui::IconData {
    const N: usize = 64;
    let mut rgba = vec![0u8; N * N * 4];
    let top = [0x8B_u8, 0x9C, 0xFF];
    let bottom = [0x5B_u8, 0x4B, 0xD8];
    // Tree glyph: root at top, trunk, bar, three children.
    let nodes = [(32.0, 17.0, 6.0), (17.0, 46.0, 5.5), (32.0, 46.0, 5.5), (47.0, 46.0, 5.5)];
    let segs = [((32.0, 17.0), (32.0, 32.0)), ((17.0, 32.0), (47.0, 32.0)), ((17.0, 32.0), (17.0, 46.0)), ((32.0, 32.0), (32.0, 46.0)), ((47.0, 32.0), (47.0, 46.0))];
    for y in 0..N {
        for x in 0..N {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            // Rounded square with radius 14.
            let r = 14.0;
            let dx = (fx - 32.0).abs() - (32.0 - r);
            let dy = (fy - 32.0).abs() - (32.0 - r);
            let d = (dx.max(0.0).powi(2) + dy.max(0.0).powi(2)).sqrt() - r + dx.max(dy).min(0.0);
            let cover = (0.5 - d).clamp(0.0, 1.0);
            if cover <= 0.0 {
                continue;
            }
            let t = fy / N as f32;
            let mut c = [0f32; 3];
            for i in 0..3 {
                c[i] = top[i] as f32 * (1.0 - t) + bottom[i] as f32 * t;
            }
            // Glyph coverage.
            let mut g: f32 = 0.0;
            for (cx, cy, rad) in nodes {
                let dd = ((fx - cx).powi(2) + (fy - cy).powi(2)).sqrt() - rad;
                g = g.max((0.5 - dd).clamp(0.0, 1.0));
            }
            for ((ax, ay), (bx, by)) in segs {
                let (vx, vy) = (bx - ax, by - ay);
                let len2: f32 = vx * vx + vy * vy;
                let h = (((fx - ax) * vx + (fy - ay) * vy) / len2).clamp(0.0, 1.0);
                let dd = ((fx - ax - vx * h).powi(2) + (fy - ay - vy * h).powi(2)).sqrt() - 2.2;
                g = g.max((0.5 - dd).clamp(0.0, 1.0));
            }
            for (i, ch) in c.iter_mut().enumerate() {
                let _ = i;
                *ch = *ch * (1.0 - g) + 255.0 * g;
            }
            let o = (y * N + x) * 4;
            rgba[o] = c[0] as u8;
            rgba[o + 1] = c[1] as u8;
            rgba[o + 2] = c[2] as u8;
            rgba[o + 3] = (cover * 255.0) as u8;
        }
    }
    egui::IconData { rgba, width: N as u32, height: N as u32 }
}
