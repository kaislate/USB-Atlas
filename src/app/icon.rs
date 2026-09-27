//! The app logo: a USB trident on a fixed indigo tile.
//!
//! The geometry is defined once (in 64-unit logo space) and rasterized with
//! anti-aliasing, so the window icon and the in-app brand tile are pixel-for-
//! pixel the same drawing at any size.

use egui::{Color32, ColorImage, Rect, TextureHandle, TextureOptions, Ui, Vec2};

// Geometry, rasterizer and .ico writer, shared with build.rs.
include!("../logo_raster.rs");

pub fn app_icon() -> egui::IconData {
    const N: usize = 256;
    egui::IconData {
        rgba: rasterize(N),
        width: N as u32,
        height: N as u32,
    }
}

/// Paints the logo into `rect`, rasterized at the exact physical pixel size.
pub fn paint(ui: &Ui, rect: Rect) {
    let ppp = ui.ctx().pixels_per_point();
    let n = (rect.width() * ppp).round().max(8.0) as usize;
    let id = egui::Id::new(("app-logo", n));
    let tex = ui
        .ctx()
        .data_mut(|d| d.get_temp::<TextureHandle>(id))
        .unwrap_or_else(|| {
            let img = ColorImage::from_rgba_unmultiplied([n, n], &rasterize(n));
            let t = ui
                .ctx()
                .load_texture("app-logo", img, TextureOptions::LINEAR);
            ui.ctx().data_mut(|d| d.insert_temp(id, t.clone()));
            t
        });
    ui.painter().image(
        tex.id(),
        rect,
        Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
        Color32::WHITE,
    );
}

/// Allocates space and paints the logo.
pub fn logo(ui: &mut Ui, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::hover());
    paint(ui, rect);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ico_has_all_sizes() {
        let sizes = [16, 32, 48, 256];
        let ico = ico_bytes(&sizes);
        assert_eq!(&ico[..6], &[0, 0, 1, 0, 4, 0]);
        // Last entry's offset + size ends exactly at the file end.
        let e = 6 + 16 * 3;
        let size = u32::from_le_bytes(ico[e + 8..e + 12].try_into().unwrap()) as usize;
        let off = u32::from_le_bytes(ico[e + 12..e + 16].try_into().unwrap()) as usize;
        assert_eq!(off + size, ico.len());
        assert_eq!(ico[e], 0, "256 px is stored as 0");
    }

    #[test]
    fn logo_has_transparent_corners_and_white_glyph() {
        let n = 64;
        let img = rasterize(n);
        let px = |x: usize, y: usize| &img[(y * n + x) * 4..(y * n + x) * 4 + 4];
        assert_eq!(px(0, 0)[3], 0, "rounded corner is transparent");
        // Middle of the shaft is white.
        assert_eq!(px(30, 32)[..3], [255, 255, 255]);
        // Background inside the tile is indigo, fully opaque.
        let bg = px(32, 56);
        assert_eq!(bg[3], 255);
        assert!(bg[2] > bg[0] && bg[2] > bg[1]);
    }
}
