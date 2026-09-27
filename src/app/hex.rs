//! Decoded descriptor tree + hex view with two-way hover highlighting.

use egui::{Align2, CornerRadius, FontId, Pos2, Rect, RichText, Sense, Ui, Vec2};

use super::theme::with_alpha;
use super::App;
use crate::descriptors::{DescNode, Field};

fn is_hot(app: &App, section: &str, offset: usize, size: usize) -> bool {
    matches!(&app.hover_view, Some((s, o, l)) if s == section && *o == offset && *l == size)
}

fn overlaps(app: &App, section: &str, offset: usize, size: usize) -> bool {
    matches!(&app.hover_view, Some((s, o, l)) if s == section && offset < o + l && *o < offset + size)
}

pub fn desc_tree(app: &mut App, ui: &mut Ui, section: &str, node: &DescNode) {
    node_ui(app, ui, section, node, 0);
}

fn node_ui(app: &mut App, ui: &mut Ui, section: &str, node: &DescNode, depth: usize) {
    let p = app.p;
    let id = ui.make_persistent_id(("desc", section, node.offset, &node.title));
    let state = egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, depth < 3);
    let header = state.show_header(ui, |ui| {
        let hot = overlaps(app, section, node.offset, node.len) && depth > 0;
        let color = if hot { p.accent } else { p.text };
        let r = ui.add(egui::Label::new(RichText::new(&node.title).strong().color(color)).sense(Sense::hover()));
        ui.label(RichText::new(format!("@{:#06X} · {} B", node.offset, node.len)).monospace().size(11.0).color(p.text_faint));
        if !node.warnings.is_empty() {
            ui.label(RichText::new(egui_phosphor::regular::WARNING).color(p.warn));
        }
        r
    });
    let (_, hdr, _) = header.body(|ui| {
        for w in &node.warnings {
            ui.label(RichText::new(format!("{}  {w}", egui_phosphor::regular::WARNING)).color(p.warn));
        }
        egui::Grid::new(("fields", section, node.offset, depth)).num_columns(2).spacing([12.0, 3.0]).show(ui, |ui| {
            for f in &node.fields {
                field_row(app, ui, section, f);
                ui.end_row();
            }
        });
        for c in &node.children {
            node_ui(app, ui, section, c, depth + 1);
        }
    });
    if hdr.inner.hovered() || hdr.response.hovered() {
        app.hover_bytes = Some((section.to_string(), node.offset, node.len));
    }
}

fn field_row(app: &mut App, ui: &mut Ui, section: &str, f: &Field) {
    let p = app.p;
    let hot = is_hot(app, section, f.offset, f.size);
    let name_color = if hot { p.accent } else { p.text_muted };
    let a = ui.add(egui::Label::new(RichText::new(&f.name).font(FontId::monospace(12.0)).color(name_color)).sense(Sense::hover()));
    let b = ui.add(egui::Label::new(RichText::new(&f.display).color(p.text)).wrap().sense(Sense::hover()));
    if hot {
        let r = a.rect.union(b.rect).expand2(Vec2::new(4.0, 1.0));
        ui.painter().rect_filled(r, CornerRadius::same(4), with_alpha(p.accent, 0.10));
    }
    if a.hovered() || b.hovered() {
        app.hover_bytes = Some((section.to_string(), f.offset, f.size));
    }
}

/// Deepest field containing byte `i`.
fn field_at(desc: &DescNode, i: usize) -> Option<&Field> {
    desc.walk()
        .into_iter()
        .flat_map(|n| n.fields.iter())
        .filter(|f| f.offset <= i && i < f.offset + f.size)
        .min_by_key(|f| f.size)
}

pub fn hex_view(app: &mut App, ui: &mut Ui, section: &str, raw: &[u8], desc: &DescNode) {
    let p = app.p;
    let font = FontId::monospace(12.0);
    let cw = ui.painter().layout_no_wrap("0".into(), font.clone(), p.text).size().x;
    let row_h = 18.0;
    let off_w = cw * 5.0;
    let cell = cw * 2.0 + cw * 0.9;
    let ascii_gap = cw * 1.5;
    let full = |n: usize| off_w + cell * n as f32 + cw * 0.6 + ascii_gap + cw * n as f32;
    let per_row = if ui.available_width() >= full(16) { 16usize } else { 8 };
    let width = full(per_row);
    let rows = raw.len().div_ceil(per_row);

    // Descriptor boundaries (alternating tint per descriptor).
    let mut starts: Vec<usize> = desc.walk().iter().filter(|n| n.fields.first().is_some_and(|f| f.name == "bLength")).map(|n| n.offset).collect();
    starts.sort_unstable();
    starts.dedup();
    let chunk_of = |i: usize| starts.iter().filter(|&&s| s <= i).count();

    egui::ScrollArea::both().id_salt(("hex", section)).max_height(440.0).auto_shrink([true, true]).show(ui, |ui| {
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, rows as f32 * row_h + 4.0), Sense::hover());
        // Follow hovers coming from the decoded tree.
        if !app.hover_from_hex_prev {
            if let Some((s, o, _)) = &app.hover_view {
                if s == section {
                    let y = rect.top() + (*o / per_row) as f32 * row_h;
                    ui.scroll_to_rect(Rect::from_min_size(Pos2::new(rect.left(), y), Vec2::new(10.0, row_h)), None);
                }
            }
        }
        let painter = ui.painter_at(rect.expand(2.0));
        let hover_pos = resp.hover_pos();
        let mut hovered_byte: Option<usize> = None;
        for r in 0..rows {
            let y = rect.top() + r as f32 * row_h;
            painter.text(Pos2::new(rect.left(), y + row_h / 2.0), Align2::LEFT_CENTER, format!("{:04X}", r * per_row), font.clone(), p.text_faint);
            for c in 0..per_row {
                let i = r * per_row + c;
                if i >= raw.len() {
                    break;
                }
                let x = rect.left() + off_w + c as f32 * cell + if c >= 8 { cw * 0.6 } else { 0.0 };
                let cell_rect = Rect::from_min_size(Pos2::new(x - cw * 0.3, y + 1.0), Vec2::new(cw * 2.6, row_h - 2.0));
                let ax = rect.left() + off_w + cell * per_row as f32 + cw * 0.6 + ascii_gap + c as f32 * cw;
                let ascii_rect = Rect::from_min_size(Pos2::new(ax, y + 1.0), Vec2::new(cw, row_h - 2.0));
                if let Some(hp) = hover_pos {
                    if cell_rect.contains(hp) || ascii_rect.contains(hp) {
                        hovered_byte = Some(i);
                    }
                }
                let chunk = chunk_of(i);
                if chunk % 2 == 0 {
                    painter.rect_filled(cell_rect, CornerRadius::same(3), with_alpha(p.text_faint, 0.07));
                }
                let hot = overlaps(app, section, i, 1);
                if hot {
                    painter.rect_filled(cell_rect, CornerRadius::same(3), p.hex_hi);
                    painter.rect_filled(ascii_rect, CornerRadius::same(2), p.hex_hi);
                }
                let color = if hot { p.text } else if raw[i] == 0 { p.text_faint } else { p.text_muted };
                painter.text(Pos2::new(x, y + row_h / 2.0), Align2::LEFT_CENTER, format!("{:02X}", raw[i]), font.clone(), color);
                let ch = raw[i];
                let a = if (0x20..0x7F).contains(&ch) { ch as char } else { '·' };
                painter.text(Pos2::new(ax, y + row_h / 2.0), Align2::LEFT_CENTER, a.to_string(), font.clone(), if hot { p.text } else { p.text_faint });
            }
        }
        if let Some(i) = hovered_byte {
            if let Some(f) = field_at(desc, i) {
                app.hover_bytes = Some((section.to_string(), f.offset, f.size));
                app.hover_from_hex = true;
                let (name, disp) = (f.name.clone(), f.display.clone());
                resp.on_hover_ui_at_pointer(|ui| {
                    ui.label(RichText::new(format!("{name}  @{i:#06X}")).monospace().strong());
                    ui.label(disp);
                });
            }
        }
    });
}
