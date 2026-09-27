//! Ctrl+K command palette: jump to any device or run any command.

use egui::{Align2, CornerRadius, Key, RichText, Stroke};
use egui_phosphor::regular as ph;

use super::{icons, theme, Action, App};
use crate::tree::NodeKind;

struct Item {
    icon: &'static str,
    title: String,
    hint: String,
    action: Action,
    score: i32,
}

/// Simple fuzzy score: all query chars in order; bonus for contiguous / prefix.
pub fn fuzzy(hay: &str, needle: &str) -> Option<i32> {
    if needle.is_empty() {
        return Some(0);
    }
    let h = hay.to_lowercase();
    let n = needle.to_lowercase();
    if let Some(pos) = h.find(&n) {
        return Some(1000 - pos as i32 * 2 - h.len() as i32 / 8);
    }
    let mut score = 0;
    let mut hi = h.chars();
    let mut last_match = false;
    for c in n.chars() {
        if c == ' ' {
            continue;
        }
        let mut found = false;
        for hc in hi.by_ref() {
            if hc == c {
                score += if last_match { 8 } else { 2 };
                last_match = true;
                found = true;
                break;
            }
            last_match = false;
        }
        if !found {
            return None;
        }
    }
    Some(score)
}

/// Palette titles need 'static strings.
fn learn_title(c: super::learn::Chapter) -> &'static str {
    use super::learn::Chapter::*;
    match c {
        HowItWorks => "Learn: How USB works",
        Connectors => "Learn: Connectors",
        Power => "Learn: Power",
        Names => "Learn: Versions & names (3.2 Gen 2x2 …)",
        Speeds => "Learn: Speed comparison",
        Companion => "Learn: Companion ports",
        Enumeration => "Learn: Enumeration",
        Transfers => "Learn: Transfer types",
        Signaling => "Learn: Signaling & encoding",
        LinkPower => "Learn: Link power states",
        TypeC => "Learn: Type-C, PD & USB4",
        Troubleshooting => "Learn: Troubleshooting",
        YourPc => "Learn: Your computer",
    }
}

fn commands(app: &App) -> Vec<(&'static str, &'static str, &'static str, Action)> {
    let mut v = vec![
        (ph::ARROWS_CLOCKWISE, "Refresh", "F5", Action::Refresh),
        (
            ph::GRAPH,
            "Switch to Map view",
            "Ctrl+2",
            Action::SetView(super::ViewMode::Map),
        ),
        (
            ph::TREE_VIEW,
            "Switch to Tree view",
            "Ctrl+1",
            Action::SetView(super::ViewMode::Tree),
        ),
        (
            ph::GRADUATION_CAP,
            "Open the USB guide",
            "Ctrl+3",
            Action::SetView(super::ViewMode::Learn),
        ),
        (
            ph::LIST_BULLETS,
            "Show / hide map legend",
            "L",
            Action::ToggleMapLegend,
        ),
        (
            ph::FLOPPY_DISK,
            "Save snapshot",
            "Ctrl+S",
            Action::SaveSnapshot,
        ),
        (
            ph::FOLDER_OPEN,
            "Open snapshot",
            "Ctrl+O",
            Action::OpenSnapshot,
        ),
        (ph::GIT_DIFF, "Compare with snapshot", "", Action::Compare),
        (
            ph::EXPORT,
            "Export HTML report",
            "Ctrl+E",
            Action::ExportHtml,
        ),
        (ph::EXPORT, "Export text report", "", Action::ExportText),
        (
            ph::COPY,
            "Copy full report",
            "Ctrl+Shift+C",
            Action::CopyFullReport,
        ),
        (
            ph::MOON,
            "Toggle light / dark theme",
            "Ctrl+Shift+L",
            Action::ToggleTheme,
        ),
        (
            ph::CIRCLE_DASHED,
            "Toggle empty ports",
            "",
            Action::ToggleEmptyPorts,
        ),
        (
            ph::STACK,
            "Toggle Windows child devices",
            "",
            Action::ToggleChildDevices,
        ),
        (
            ph::PLUG,
            "Toggle physical sockets (merge companion ports)",
            "",
            Action::TogglePhysical,
        ),
        (ph::PULSE, "Toggle live updates", "", Action::ToggleLive),
        (ph::ROWS, "Expand all", "", Action::ExpandAll),
        (ph::LIST, "Collapse all", "", Action::CollapseAll),
        (ph::WRENCH, "Open Device Manager", "", Action::DeviceManager),
        (ph::GEAR, "Settings", "Ctrl+,", Action::OpenSettings),
        (ph::INFO, "About", "", Action::About),
    ];
    for c in super::learn::Chapter::all() {
        v.push((c.icon(), learn_title(c), "guide", Action::OpenLesson(c)));
    }
    if !crate::platform::is_admin() {
        v.push((
            ph::SHIELD_CHECK,
            "Restart as administrator",
            "",
            Action::RunAsAdmin,
        ));
    }
    if !app.is_live() {
        v.push((ph::PULSE, "Back to live view", "", Action::BackToLive));
    }
    v
}

pub fn show(app: &mut App, ctx: &egui::Context) {
    if !app.palette_open {
        return;
    }
    let p = app.p;
    let q = app.palette_query.trim().to_string();
    let mut items: Vec<Item> = Vec::new();
    for n in &app.flat.nodes {
        if matches!(n.kind, NodeKind::EmptyPort | NodeKind::Computer)
            && !q.is_empty()
            && !n.search_text.contains(&q.to_lowercase())
        {
            continue;
        }
        if n.kind == NodeKind::EmptyPort {
            continue;
        }
        let label = app.display_label(n);
        // Fuzzy on the visible name; plain substring on everything else.
        let score =
            fuzzy(&label, &q).or_else(|| n.search_text.contains(&q.to_lowercase()).then_some(300));
        if let Some(s) = score {
            let bonus = if matches!(n.kind, NodeKind::Device | NodeKind::Hub) {
                40
            } else {
                0
            };
            items.push(Item {
                icon: icons::for_node(n, app.device_at(&n.path)),
                title: label,
                hint: if n.detail.is_empty() {
                    n.port_label.clone()
                } else {
                    n.detail.clone()
                },
                action: Action::Select(n.id.clone()),
                score: s + bonus,
            });
        }
    }
    for (icon, title, sc, action) in commands(app) {
        if let Some(s) = fuzzy(title, &q) {
            items.push(Item {
                icon,
                title: title.to_string(),
                hint: sc.to_string(),
                action,
                score: s + if q.is_empty() { -100 } else { 20 },
            });
        }
    }
    items.sort_by(|a, b| b.score.cmp(&a.score));
    items.truncate(40);
    app.palette_sel = app.palette_sel.min(items.len().saturating_sub(1));

    let (up, down, enter, esc) = ctx.input(|i| {
        (
            i.key_pressed(Key::ArrowUp),
            i.key_pressed(Key::ArrowDown),
            i.key_pressed(Key::Enter),
            i.key_pressed(Key::Escape),
        )
    });
    if up {
        app.palette_sel = app.palette_sel.saturating_sub(1);
    }
    if down && app.palette_sel + 1 < items.len() {
        app.palette_sel += 1;
    }
    let mut chosen: Option<Action> = None;
    if enter {
        chosen = items.get(app.palette_sel).map(|i| i.action.clone());
    }

    // Backdrop
    let screen = ctx.content_rect();
    ctx.layer_painter(egui::LayerId::new(
        egui::Order::Middle,
        egui::Id::new("palette-bg"),
    ))
    .rect_filled(
        screen,
        CornerRadius::ZERO,
        egui::Color32::from_black_alpha(if p.dark { 140 } else { 70 }),
    );
    let width = (screen.width() - 40.0).min(620.0);
    let area = egui::Area::new(egui::Id::new("palette"))
        .anchor(Align2::CENTER_TOP, egui::vec2(0.0, 80.0))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(p.card)
                .stroke(Stroke::new(1.0, p.border))
                .corner_radius(CornerRadius::same(14))
                .shadow(egui::Shadow {
                    offset: [0, 16],
                    blur: 48,
                    spread: 0,
                    color: egui::Color32::from_black_alpha(if p.dark { 160 } else { 60 }),
                })
                .inner_margin(egui::Margin::same(10))
                .show(ui, |ui| {
                    ui.set_width(width);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(ph::MAGNIFYING_GLASS)
                                .size(18.0)
                                .color(p.text_muted),
                        );
                        let r = ui.add(
                            egui::TextEdit::singleline(&mut app.palette_query)
                                .hint_text("Jump to a device or run a command…")
                                .frame(egui::Frame::NONE)
                                .font(egui::FontId::proportional(16.0))
                                .desired_width(f32::INFINITY),
                        );
                        r.request_focus();
                        if r.changed() {
                            app.palette_sel = 0;
                        }
                    });
                    ui.separator();
                    egui::ScrollArea::vertical()
                        .max_height(400.0)
                        .show(ui, |ui| {
                            if items.is_empty() {
                                ui.label(RichText::new("No results").color(p.text_muted));
                            }
                            for (i, it) in items.iter().enumerate() {
                                let sel = i == app.palette_sel;
                                let (rect, resp) = ui.allocate_exact_size(
                                    egui::vec2(ui.available_width(), 34.0),
                                    egui::Sense::click(),
                                );
                                if sel || resp.hovered() {
                                    ui.painter().rect_filled(
                                        rect,
                                        CornerRadius::same(8),
                                        if sel { p.accent_soft } else { p.card_hover },
                                    );
                                }
                                if sel && (up || down) {
                                    ui.scroll_to_rect(rect, None);
                                }
                                let c = rect.left_center();
                                ui.painter().text(
                                    c + egui::vec2(16.0, 0.0),
                                    Align2::CENTER_CENTER,
                                    it.icon,
                                    egui::FontId::proportional(16.0),
                                    if sel { p.accent } else { p.text_muted },
                                );
                                ui.painter().text(
                                    c + egui::vec2(34.0, 0.0),
                                    Align2::LEFT_CENTER,
                                    &it.title,
                                    theme::semibold(13.5),
                                    p.text,
                                );
                                ui.painter().text(
                                    rect.right_center() - egui::vec2(10.0, 0.0),
                                    Align2::RIGHT_CENTER,
                                    &it.hint,
                                    egui::FontId::proportional(12.0),
                                    p.text_faint,
                                );
                                if resp.clicked() {
                                    chosen = Some(it.action.clone());
                                }
                            }
                        });
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("↑↓ navigate    Enter select    Esc close")
                            .size(11.0)
                            .color(p.text_faint),
                    );
                });
        });
    let clicked_outside = ctx.input(|i| {
        i.pointer.any_pressed()
            && i.pointer
                .interact_pos()
                .is_some_and(|pos| !area.response.rect.contains(pos))
    });
    if esc || clicked_outside {
        app.palette_open = false;
    }
    if let Some(a) = chosen {
        app.palette_open = false;
        app.dispatch(a);
    }
}

#[cfg(test)]
mod tests {
    use super::fuzzy;

    #[test]
    fn fuzzy_matching() {
        assert!(fuzzy("SanDisk Ultra Fit", "ultra").is_some());
        assert!(fuzzy("SanDisk Ultra Fit", "sdfit").is_some());
        assert!(fuzzy("SanDisk Ultra Fit", "xyz").is_none());
        assert!(fuzzy("Ultra", "ultra").unwrap() > fuzzy("SanDisk Ultra Fit", "ultra").unwrap());
    }
}
