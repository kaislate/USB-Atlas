//! Small custom-painted widgets shared by the views.

use egui::{Align2, Color32, CornerRadius, FontId, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2};

use super::theme::{self, Palette};
use crate::model::Speed;

/// Rounded pill with text, e.g. a speed badge.
pub fn pill(ui: &mut Ui, p: &Palette, text: &str, color: Color32) -> Response {
    let font = FontId::proportional(11.5);
    let galley = ui.painter().layout_no_wrap(text.to_string(), font, color);
    let size = galley.size() + Vec2::new(14.0, 6.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(255), p.soft(color));
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, color);
    resp
}

/// Chip with an icon glyph and text.
pub fn chip(ui: &mut Ui, p: &Palette, icon: &str, text: &str, color: Color32) -> Response {
    let font = FontId::proportional(12.0);
    let label = if icon.is_empty() { text.to_string() } else { format!("{icon}  {text}") };
    let galley = ui.painter().layout_no_wrap(label, font, color);
    let size = galley.size() + Vec2::new(18.0, 9.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(8), p.soft(color));
    ui.painter().rect_stroke(rect, CornerRadius::same(8), Stroke::new(1.0, theme::with_alpha(color, 0.25)), StrokeKind::Inside);
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, color);
    resp
}

/// Square tile with a large icon.
pub fn icon_tile(ui: &mut Ui, p: &Palette, icon: &str, size: f32, color: Color32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    paint_icon_tile(ui, p, rect, icon, color);
    resp
}

pub fn paint_icon_tile(ui: &Ui, p: &Palette, rect: Rect, icon: &str, color: Color32) {
    let r = (rect.width() * 0.26) as u8;
    ui.painter().rect_filled(rect, CornerRadius::same(r), p.soft(color));
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, icon, theme::icon_fill(rect.width() * 0.56), color);
}

/// Card frame used for detail sections.
pub fn card(p: &Palette) -> egui::Frame {
    egui::Frame::new()
        .fill(p.card)
        .stroke(Stroke::new(1.0, p.border))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(egui::Margin::symmetric(16, 14))
}

pub fn card_title(ui: &mut Ui, p: &Palette, icon: &str, title: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(icon).color(p.accent).size(15.0));
        ui.label(egui::RichText::new(title).font(theme::semibold(14.5)).color(p.text));
    });
    ui.add_space(4.0);
}

/// Segmented control. Returns true when the selection changed.
pub fn segmented<T: PartialEq + Copy>(ui: &mut Ui, p: &Palette, salt: &str, value: &mut T, options: &[(T, &str)]) -> bool {
    let mut changed = false;
    let font = FontId::proportional(13.0);
    let pad = Vec2::new(14.0, 6.0);
    let galleys: Vec<_> = options
        .iter()
        .map(|(_, t)| ui.painter().layout_no_wrap(t.to_string(), font.clone(), p.text))
        .collect();
    let w: f32 = galleys.iter().map(|g| g.size().x + pad.x * 2.0).sum::<f32>() + 6.0;
    let h = galleys.iter().map(|g| g.size().y).fold(0.0, f32::max) + pad.y * 2.0 + 6.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(10), if p.dark { p.bg } else { p.card_hover });
    let mut x = rect.left() + 3.0;
    let sel_idx = options.iter().position(|(v, _)| v == value).unwrap_or(0);
    // Animated selection highlight.
    let target_x: f32 = rect.left() + 3.0 + galleys[..sel_idx].iter().map(|g| g.size().x + pad.x * 2.0).sum::<f32>();
    let id = egui::Id::new(("seg", salt));
    let anim_x = ui.ctx().animate_value_with_time(id, target_x, 0.16);
    let sel_w = galleys[sel_idx].size().x + pad.x * 2.0;
    let sel_rect = Rect::from_min_size(egui::pos2(anim_x, rect.top() + 3.0), Vec2::new(sel_w, h - 6.0));
    ui.painter().rect_filled(sel_rect, CornerRadius::same(8), p.card);
    ui.painter().rect_stroke(sel_rect, CornerRadius::same(8), Stroke::new(1.0, p.border), StrokeKind::Inside);
    for (i, ((v, _), g)) in options.iter().zip(galleys).enumerate() {
        let bw = g.size().x + pad.x * 2.0;
        let r = Rect::from_min_size(egui::pos2(x, rect.top() + 3.0), Vec2::new(bw, h - 6.0));
        let resp = ui.interact(r, id.with(i), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
        let color = if i == sel_idx || resp.hovered() { p.text } else { p.text_muted };
        ui.painter().galley_with_override_text_color(r.center() - g.size() / 2.0, g, color);
        if resp.clicked() && *value != *v {
            *value = *v;
            changed = true;
        }
        x += bw;
    }
    changed
}

/// Horizontal usage bar (e.g. disk space, power budget).
pub fn usage_bar(ui: &mut Ui, p: &Palette, fraction: f32, color: Color32, width: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 6.0), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(3), if p.dark { p.bg } else { p.card_hover });
    let mut fill = rect;
    fill.set_width(rect.width() * fraction.clamp(0.0, 1.0));
    ui.painter().rect_filled(fill, CornerRadius::same(3), color);
}

/// A 6-step ladder showing actual speed, device capability and port maximum.
pub fn speed_ladder(ui: &mut Ui, p: &Palette, actual: Speed, device_max: Speed, port_max: Speed) {
    let steps = [
        (Speed::Low, "1.5M"),
        (Speed::Full, "12M"),
        (Speed::High, "480M"),
        (Speed::Super, "5G"),
        (Speed::SuperPlus, "10G"),
        (Speed::SuperPlus20, "20G"),
    ];
    let seg_w = 52.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(seg_w * steps.len() as f32, 40.0), Sense::hover());
    let painter = ui.painter();
    for (i, (s, label)) in steps.iter().enumerate() {
        let r = Rect::from_min_size(
            egui::pos2(rect.left() + i as f32 * seg_w + 2.0, rect.top() + 4.0),
            Vec2::new(seg_w - 4.0, 16.0),
        );
        let reached = *s <= actual;
        let capable = *s <= device_max && *s > actual;
        let color = p.speed(actual);
        if reached {
            painter.rect_filled(r, CornerRadius::same(4), color);
        } else if capable {
            painter.rect_filled(r, CornerRadius::same(4), theme::with_alpha(p.warn, 0.18));
            painter.rect_stroke(r, CornerRadius::same(4), Stroke::new(1.0, p.warn), StrokeKind::Inside);
        } else {
            painter.rect_filled(r, CornerRadius::same(4), if p.dark { p.bg } else { p.card_hover });
        }
        if *s == port_max {
            let x = r.right() + 2.0;
            painter.line_segment([egui::pos2(x, r.top() - 3.0), egui::pos2(x, r.bottom() + 3.0)], Stroke::new(2.0, p.text_muted));
        }
        painter.text(
            egui::pos2(r.center().x, rect.bottom() - 7.0),
            Align2::CENTER_CENTER,
            *label,
            FontId::proportional(10.5),
            if *s == actual { p.text } else { p.text_faint },
        );
    }
}

/// Small legend row for the speed ladder.
pub fn legend_item(ui: &mut Ui, p: &Palette, color: Color32, outline: bool, text: &str) {
    let (r, _) = ui.allocate_exact_size(Vec2::new(10.0, 10.0), Sense::hover());
    if outline {
        ui.painter().rect_stroke(r, CornerRadius::same(2), Stroke::new(1.0, color), StrokeKind::Inside);
    } else {
        ui.painter().rect_filled(r, CornerRadius::same(2), color);
    }
    ui.label(egui::RichText::new(text).size(11.5).color(p.text_muted));
}

/// A borderless icon button with hover background.
pub fn icon_button(ui: &mut Ui, p: &Palette, icon: &str, tooltip: &str) -> Response {
    let size = Vec2::splat(30.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let resp = resp.on_hover_text(tooltip).on_hover_cursor(egui::CursorIcon::PointingHand);
    if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(8), p.card_hover);
    }
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        icon,
        FontId::proportional(17.0),
        if resp.hovered() { p.text } else { p.text_muted },
    );
    resp
}

/// Primary-ish text button with an icon.
pub fn action_button(ui: &mut Ui, p: &Palette, icon: &str, text: &str, danger: bool) -> Response {
    let color = if danger { p.error } else { p.text };
    let label = egui::RichText::new(format!("{icon}  {text}")).color(color).size(13.0);
    ui.add(egui::Button::new(label).corner_radius(CornerRadius::same(8)).min_size(Vec2::new(0.0, 30.0)))
}
