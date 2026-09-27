//! The topology tree (left panel).

use std::collections::HashSet;

use egui::text::{LayoutJob, TextFormat};
use egui::{
    Align2, Color32, CornerRadius, FontId, Key, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2,
};
use egui_phosphor::regular as ph;

use super::theme::{self, with_alpha};
use super::{icons, widgets, Action, App, QuickFilter};
use crate::insights::Severity;
use crate::tree::{FlatNode, Health, NodeKind, NodePath};

fn terms(search: &str) -> Vec<String> {
    search
        .split_whitespace()
        .map(|s| s.to_lowercase())
        .collect()
}

fn matches(app: &App, n: &FlatNode, terms: &[String]) -> bool {
    let q = match app.quick_filter {
        QuickFilter::All => true,
        QuickFilter::Problems => {
            n.health >= Health::Warning
                || app
                    .insights
                    .iter()
                    .any(|i| i.node_id == n.id && i.severity >= Severity::Warning)
        }
        QuickFilter::SuperSpeed => n.speed.is_super(),
        QuickFilter::Storage => n.classes.contains(&0x08) || has_drive_letter(&n.detail),
        QuickFilter::Input => n.classes.contains(&0x03),
    };
    q && n.kind != NodeKind::Computer && terms.iter().all(|t| n.search_text.contains(t.as_str()))
}

/// Whether a node passes the current search and quick filter.
pub fn node_matches(app: &App, n: &FlatNode) -> bool {
    matches(app, n, &terms(&app.search))
}

fn has_drive_letter(detail: &str) -> bool {
    detail
        .split_whitespace()
        .any(|t| t.len() == 2 && t.ends_with(':') && t.as_bytes()[0].is_ascii_alphabetic())
}

pub fn filter_active(app: &App) -> bool {
    !app.search.trim().is_empty() || app.quick_filter != QuickFilter::All
}

/// Rows to display, in order, as indexes into `app.flat.nodes`.
pub fn visible_rows(app: &App) -> Vec<usize> {
    let flat = &app.flat;
    if flat.nodes.is_empty() {
        return vec![];
    }
    let mut out = Vec::new();
    if filter_active(app) {
        let t = terms(&app.search);
        let mut keep: HashSet<usize> = HashSet::new();
        for (i, n) in flat.nodes.iter().enumerate() {
            if matches(app, n, &t) {
                keep.insert(i);
                keep.extend(flat.ancestors(i));
            }
        }
        fn walk(flat: &crate::tree::Flat, i: usize, keep: &HashSet<usize>, out: &mut Vec<usize>) {
            if keep.contains(&i) {
                out.push(i);
                for &c in &flat.nodes[i].children {
                    walk(flat, c, keep, out);
                }
            }
        }
        walk(flat, 0, &keep, &mut out);
    } else {
        fn walk(app: &App, i: usize, out: &mut Vec<usize>) {
            out.push(i);
            let n = &app.flat.nodes[i];
            if !app.collapsed.contains(&n.id) {
                for &c in &n.children {
                    walk(app, c, out);
                }
            }
        }
        walk(app, 0, &mut out);
    }
    out
}

pub fn show(app: &mut App, ui: &mut Ui) {
    let p = app.p;
    // Header
    ui.horizontal(|ui| {
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new("TOPOLOGY")
                .size(11.0)
                .color(p.text_faint)
                .strong(),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if widgets::icon_button(ui, &p, ph::LIST, "Collapse all").clicked() {
                app.dispatch(Action::CollapseAll);
            }
            if widgets::icon_button(ui, &p, ph::ROWS, "Expand all").clicked() {
                app.dispatch(Action::ExpandAll);
            }
        });
    });
    ui.add_space(2.0);

    if app.snapshot.is_none() {
        skeleton(ui, &p);
        return;
    }

    let rows = visible_rows(app);
    if rows.len() <= 1 && filter_active(app) {
        ui.add_space(40.0);
        ui.vertical_centered(|ui| {
            ui.label(
                egui::RichText::new(ph::MAGNIFYING_GLASS)
                    .size(32.0)
                    .color(p.text_faint),
            );
            ui.label(egui::RichText::new("No matching devices").color(p.text_muted));
            if ui.link("Clear filters").clicked() {
                app.search.clear();
                app.quick_filter = QuickFilter::All;
            }
        });
        return;
    }

    keyboard_nav(app, ui, &rows);

    let selected = app.selected.clone();
    let companions: HashSet<String> = selected
        .as_ref()
        .map(|s| app.companions_of(s).into_iter().collect())
        .unwrap_or_default();
    let t = terms(&app.search);
    let row_h = if app.settings.compact_rows {
        24.0
    } else {
        30.0
    };
    let indent = 18.0;

    egui::ScrollArea::vertical()
        .auto_shrink([false; 2])
        .id_salt("tree-scroll")
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            let width = ui.available_width();
            for &i in &rows {
                let n = app.flat.nodes[i].clone();
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, row_h), Sense::click());
                if !ui.is_rect_visible(rect) {
                    continue;
                }
                let is_sel = selected.as_deref() == Some(n.id.as_str());
                let ghost = app.is_ghost(&n.id);
                let arrival = app.arrivals.get(&n.id).map(|t| t.elapsed().as_secs_f32());
                let painter = ui.painter();

                // Background states
                if is_sel {
                    painter.rect_filled(rect, CornerRadius::same(7), p.accent_soft);
                    painter.rect_filled(
                        Rect::from_min_size(
                            rect.min + Vec2::new(0.0, 5.0),
                            Vec2::new(3.0, rect.height() - 10.0),
                        ),
                        CornerRadius::same(2),
                        p.accent,
                    );
                    if app.scroll_to_selected {
                        ui.scroll_to_rect(rect, Some(egui::Align::Center));
                        app.scroll_to_selected = false;
                    }
                } else if resp.hovered() {
                    painter.rect_filled(rect, CornerRadius::same(7), p.card_hover);
                }
                if let Some(age) = arrival {
                    if age < 6.0 {
                        let a = (1.0 - age / 6.0).powf(1.5) * 0.35;
                        painter.rect_filled(rect, CornerRadius::same(7), with_alpha(p.ok, a));
                    }
                }
                if let Some(age) = ghost {
                    let a = (1.0 - age / 8.0).max(0.0) * 0.25;
                    painter.rect_filled(rect, CornerRadius::same(7), with_alpha(p.error, a));
                }
                if companions.contains(&n.id) {
                    let c = p.speed(n.port_max_speed);
                    painter.rect_stroke(
                        rect.shrink(0.5),
                        CornerRadius::same(7),
                        Stroke::new(1.5, with_alpha(c, 0.9)),
                        StrokeKind::Inside,
                    );
                }

                // Indent guides
                for d in 1..n.depth {
                    let x = rect.left() + 10.0 + (d as f32 - 1.0) * indent + indent / 2.0 + 2.0;
                    painter.line_segment(
                        [
                            Pos2::new(x, rect.top() - 1.0),
                            Pos2::new(x, rect.bottom() + 1.0),
                        ],
                        Stroke::new(1.0, p.guide),
                    );
                }
                let mut x = rect.left() + 10.0 + (n.depth.saturating_sub(1)) as f32 * indent;
                let cy = rect.center().y;

                // Chevron
                let has_children = !n.children.is_empty() && !super::tree_view::filter_active(app);
                let chev_rect = Rect::from_center_size(Pos2::new(x + 7.0, cy), Vec2::splat(16.0));
                if has_children && n.depth > 0 {
                    let open = !app.collapsed.contains(&n.id);
                    let chev_resp =
                        ui.interact(chev_rect, ui.id().with(("chev", &n.id)), Sense::click());
                    let col = if chev_resp.hovered() {
                        p.text
                    } else {
                        p.text_faint
                    };
                    ui.painter().text(
                        chev_rect.center(),
                        Align2::CENTER_CENTER,
                        if open {
                            ph::CARET_DOWN
                        } else {
                            ph::CARET_RIGHT
                        },
                        FontId::proportional(12.0),
                        col,
                    );
                    if chev_resp.clicked() {
                        toggle(app, &n.id);
                    }
                }
                if n.depth > 0 {
                    x += 16.0;
                }

                // Icon tile
                let dev = app.device_at(&n.path);
                let icon = icons::for_node(&n, dev);
                let icon_color = match n.health {
                    Health::Error => p.error,
                    Health::Warning => p.warn,
                    Health::Empty => p.text_faint,
                    _ => match n.kind {
                        NodeKind::Device | NodeKind::Hub => p.speed(n.speed),
                        NodeKind::Controller | NodeKind::Computer | NodeKind::RootHub => p.accent,
                        _ => p.text_muted,
                    },
                };
                let tile = Rect::from_center_size(
                    Pos2::new(x + 11.0, cy),
                    Vec2::splat(if app.settings.compact_rows {
                        18.0
                    } else {
                        22.0
                    }),
                );
                if n.kind == NodeKind::EmptyPort {
                    ui.painter().text(
                        tile.center(),
                        Align2::CENTER_CENTER,
                        icon,
                        FontId::proportional(15.0),
                        with_alpha(icon_color, 0.8),
                    );
                } else {
                    widgets::paint_icon_tile(
                        ui,
                        &p,
                        tile,
                        icon,
                        if ghost.is_some() {
                            p.text_faint
                        } else {
                            icon_color
                        },
                    );
                }
                x += 30.0;

                // Right side: speed pill + health dot
                let mut right = rect.right() - 8.0;
                if n.health == Health::Error
                    || n.health == Health::Warning
                    || n.health == Health::Info
                {
                    let c = p.health(n.health);
                    ui.painter()
                        .circle_filled(Pos2::new(right - 4.0, cy), 3.5, c);
                    right -= 14.0;
                }
                let pill_text = match n.kind {
                    NodeKind::Device | NodeKind::Hub => Some((n.speed.short(), p.speed(n.speed))),
                    NodeKind::EmptyPort => {
                        if n.port_max_speed == crate::model::Speed::Unknown {
                            None
                        } else {
                            Some((
                                n.port_max_speed.short(),
                                with_alpha(p.speed(n.port_max_speed), 0.6),
                            ))
                        }
                    }
                    NodeKind::Controller => Some(("HCI", p.text_faint)),
                    _ => None,
                };
                if let Some((t, c)) = pill_text {
                    let g =
                        ui.painter()
                            .layout_no_wrap(t.to_string(), FontId::proportional(10.5), c);
                    let pr = Rect::from_min_max(
                        Pos2::new(right - g.size().x - 12.0, cy - 8.5),
                        Pos2::new(right, cy + 8.5),
                    );
                    if n.kind == NodeKind::EmptyPort {
                        ui.painter().rect_stroke(
                            pr,
                            CornerRadius::same(255),
                            Stroke::new(1.0, with_alpha(c, 0.5)),
                            StrokeKind::Inside,
                        );
                    } else {
                        ui.painter()
                            .rect_filled(pr, CornerRadius::same(255), p.soft(c));
                    }
                    ui.painter().galley(pr.center() - g.size() / 2.0, g, c);
                    right = pr.left() - 6.0;
                }
                if app.settings.pinned.contains(&n.id) {
                    ui.painter().text(
                        Pos2::new(right - 6.0, cy),
                        Align2::CENTER_CENTER,
                        ph::PUSH_PIN,
                        FontId::proportional(12.0),
                        p.accent,
                    );
                    right -= 16.0;
                }

                // Text
                let label = app.display_label(&n);
                let mut job = LayoutJob::default();
                if app.settings.show_port_numbers && !n.port_label.is_empty() {
                    let num = n.port_label.trim_start_matches("Port ");
                    job.append(
                        &format!("{num}  "),
                        0.0,
                        TextFormat {
                            font_id: FontId::monospace(11.0),
                            color: p.text_faint,
                            valign: egui::Align::Center,
                            ..Default::default()
                        },
                    );
                }
                let label_color = if ghost.is_some()
                    || (n.kind == NodeKind::EmptyPort && n.health != Health::Error)
                {
                    p.text_faint
                } else {
                    p.text
                };
                let font = if matches!(
                    n.kind,
                    NodeKind::Device | NodeKind::Hub | NodeKind::Controller
                ) {
                    theme::semibold(13.0)
                } else {
                    FontId::proportional(13.0)
                };
                append_highlighted(
                    &mut job,
                    &label,
                    &t,
                    font,
                    label_color,
                    p.accent,
                    ghost.is_some(),
                );
                if !n.detail.is_empty() {
                    job.append(
                        &format!("   {}", n.detail),
                        0.0,
                        TextFormat {
                            font_id: FontId::proportional(12.0),
                            color: p.text_muted,
                            valign: egui::Align::Center,
                            ..Default::default()
                        },
                    );
                }
                job.wrap.max_width = (right - x - 4.0).max(10.0);
                job.wrap.max_rows = 1;
                job.wrap.break_anywhere = true;
                job.wrap.overflow_character = Some('…');
                let g = ui.painter().layout_job(job);
                ui.painter()
                    .galley(Pos2::new(x, cy - g.size().y / 2.0), g, p.text);

                // Interaction
                if resp.clicked() || resp.secondary_clicked() {
                    app.selected = Some(n.id.clone());
                    app.hover_bytes = None;
                    app.report_cache = None;
                }
                if resp.double_clicked() && !n.children.is_empty() {
                    toggle(app, &n.id);
                }
                let resp = resp.on_hover_ui_at_pointer(|ui| hover_card(app, ui, &n));
                resp.context_menu(|ui| context_menu(app, ui, &n.id));
            }
            ui.add_space(20.0);
        });
}

fn append_highlighted(
    job: &mut LayoutJob,
    text: &str,
    terms: &[String],
    font: FontId,
    color: Color32,
    hi: Color32,
    strike: bool,
) {
    let fmt = |c: Color32, bg: Color32| TextFormat {
        font_id: font.clone(),
        color: c,
        background: bg,
        valign: egui::Align::Center,
        strikethrough: if strike {
            Stroke::new(1.0, c)
        } else {
            Stroke::NONE
        },
        ..Default::default()
    };
    let lower = text.to_lowercase();
    // Highlight the first occurrence of the longest term (ASCII-safe offsets only).
    let found = terms
        .iter()
        .filter(|t| !t.is_empty())
        .filter_map(|t| lower.find(t.as_str()).map(|i| (i, t.len())))
        .filter(|(i, l)| {
            text.is_char_boundary(*i) && text.is_char_boundary(i + l) && lower.len() == text.len()
        })
        .max_by_key(|(_, l)| *l);
    match found {
        Some((i, l)) => {
            job.append(&text[..i], 0.0, fmt(color, Color32::TRANSPARENT));
            job.append(&text[i..i + l], 0.0, fmt(hi, with_alpha(hi, 0.18)));
            job.append(&text[i + l..], 0.0, fmt(color, Color32::TRANSPARENT));
        }
        None => job.append(text, 0.0, fmt(color, Color32::TRANSPARENT)),
    }
}

fn toggle(app: &mut App, id: &str) {
    if !app.collapsed.remove(id) {
        app.collapsed.insert(id.to_string());
    }
}

fn keyboard_nav(app: &mut App, ui: &Ui, rows: &[usize]) {
    let text_focused = ui.ctx().memory(|m| m.focused().is_some());
    if text_focused || app.palette_open || app.rename.is_some() || app.confirm.is_some() {
        return;
    }
    let Some(sel) = app.selected.clone() else {
        return;
    };
    let Some(&idx) = app.flat.by_id.get(&sel) else {
        return;
    };
    let pos = rows.iter().position(|&r| r == idx);
    let (up, down, left, right, home, end, f2, copy, pgup, pgdn) = ui.input(|i| {
        (
            i.key_pressed(Key::ArrowUp),
            i.key_pressed(Key::ArrowDown),
            i.key_pressed(Key::ArrowLeft),
            i.key_pressed(Key::ArrowRight),
            i.key_pressed(Key::Home),
            i.key_pressed(Key::End),
            i.key_pressed(Key::F2),
            i.modifiers.command && i.key_pressed(Key::C),
            i.key_pressed(Key::PageUp),
            i.key_pressed(Key::PageDown),
        )
    });
    let go = |app: &mut App, r: usize| {
        app.selected = Some(app.flat.nodes[r].id.clone());
        app.scroll_to_selected = true;
        app.hover_bytes = None;
        app.report_cache = None;
    };
    if let Some(p) = pos {
        if up && p > 0 {
            go(app, rows[p - 1]);
        }
        if down && p + 1 < rows.len() {
            go(app, rows[p + 1]);
        }
        if pgup {
            go(app, rows[p.saturating_sub(15)]);
        }
        if pgdn {
            go(app, rows[(p + 15).min(rows.len() - 1)]);
        }
    }
    if home {
        go(app, rows[0]);
    }
    if end {
        go(app, *rows.last().unwrap());
    }
    let n = app.flat.nodes[idx].clone();
    if right && !n.children.is_empty() {
        if app.collapsed.contains(&n.id) {
            app.collapsed.remove(&n.id.clone());
        } else if let Some(&c) = n.children.first() {
            go(app, c);
        }
    }
    if left {
        if !n.children.is_empty() && !app.collapsed.contains(&n.id) && n.depth > 0 {
            app.collapsed.insert(n.id.clone());
        } else if let Some(pa) = n.parent {
            go(app, pa);
        }
    }
    if f2 {
        app.dispatch(Action::Rename(sel.clone()));
    }
    if copy {
        app.dispatch(Action::CopyNodeReport(sel));
    }
}

fn hover_card(app: &App, ui: &mut Ui, n: &FlatNode) {
    let p = app.p;
    ui.set_max_width(320.0);
    ui.label(egui::RichText::new(app.display_label(n)).font(theme::semibold(13.5)));
    if app.nickname_for(n).is_some() {
        ui.label(egui::RichText::new(&n.label).color(p.text_muted).size(12.0));
    }
    if let Some(d) = app.device_at(&n.path) {
        if let Some((v, pi)) = d.vid_pid() {
            let db = crate::usbids::db();
            ui.label(
                egui::RichText::new(format!("{v:04X}:{pi:04X}  {}", db.vendor(v).unwrap_or("")))
                    .monospace()
                    .size(12.0)
                    .color(p.text_muted),
            );
        }
        ui.label(
            egui::RichText::new(d.speed.label())
                .color(p.speed(d.speed))
                .size(12.0),
        );
    }
    if let NodePath::Port(..) = n.path {
        if let Some(port) = app.port_at(&n.path) {
            ui.label(
                egui::RichText::new(format!(
                    "{} · max {}",
                    n.port_label,
                    port.max_speed().label()
                ))
                .size(12.0)
                .color(p.text_muted),
            );
        }
    }
    for i in app.insights.iter().filter(|i| i.node_id == n.id).take(3) {
        let c = match i.severity {
            Severity::Error => p.error,
            Severity::Warning => p.warn,
            Severity::Info => p.info,
        };
        ui.label(
            egui::RichText::new(format!("{}  {}", ph::WARNING_CIRCLE, i.title))
                .color(c)
                .size(12.0),
        );
    }
}

/// Context menu for a node; also used by the detail header's "more" button.
pub fn context_menu(app: &mut App, ui: &mut Ui, id: &str) {
    let Some(n) = app.flat.get(id).cloned() else {
        return;
    };
    ui.set_min_width(230.0);
    let dev = app.device_at(&n.path).cloned();
    let info = match &n.path {
        NodePath::Controller(ci) => app
            .snapshot
            .as_ref()
            .map(|s| s.controllers[*ci].info.clone()),
        NodePath::RootHub(ci) => app
            .snapshot
            .as_ref()
            .and_then(|s| s.controllers[*ci].root_hub.as_ref())
            .and_then(|h| h.info.clone()),
        NodePath::Child(ci, ch, cc) => app
            .snapshot
            .as_ref()
            .and_then(|s| crate::tree::port(s, *ci, ch))
            .and_then(|p| p.device.as_ref())
            .and_then(|d| d.info.as_ref())
            .and_then(|i| crate::tree::child(i, cc))
            .cloned(),
        _ => dev.as_ref().and_then(|d| d.info.clone()),
    };
    let name = app.display_label(&n);
    let btn = |ui: &mut Ui, icon: &str, t: &str| ui.button(format!("{icon}   {t}")).clicked();

    if btn(ui, ph::COPY, "Copy name") {
        app.dispatch(Action::CopyText(name.clone()));
    }
    if let Some((v, p)) = dev.as_ref().and_then(|d| d.vid_pid()) {
        if btn(ui, ph::HASH, &format!("Copy VID:PID ({v:04X}:{p:04X})")) {
            app.dispatch(Action::CopyText(format!("{v:04X}:{p:04X}")));
        }
    }
    if let Some(i) = &info {
        if !i.instance_id.is_empty() && btn(ui, ph::FINGERPRINT, "Copy device instance ID") {
            app.dispatch(Action::CopyText(i.instance_id.clone()));
        }
    }
    if btn(ui, ph::CODE, "Copy report") {
        app.dispatch(Action::CopyNodeReport(n.id.clone()));
    }
    ui.separator();
    if dev.is_some() && btn(ui, ph::PENCIL_SIMPLE, "Rename…  (F2)") {
        app.dispatch(Action::Rename(n.id.clone()));
    }
    let pinned = app.settings.pinned.contains(&n.id);
    if btn(
        ui,
        ph::PUSH_PIN,
        if pinned { "Unpin" } else { "Pin to favorites" },
    ) {
        app.dispatch(Action::TogglePin(n.id.clone()));
    }
    if app.is_live() {
        if let Some(i) = &info {
            let id = i.instance_id.clone();
            if !id.is_empty() {
                ui.separator();
                if let Some(d) = &dev {
                    if let Some(di) = &d.info {
                        for v in di.all_volumes() {
                            for m in &v.mount_points {
                                if btn(
                                    ui,
                                    ph::FOLDER_OPEN,
                                    &format!("Open {}", m.trim_end_matches('\\')),
                                ) {
                                    app.dispatch(Action::OpenPath(m.clone()));
                                }
                            }
                        }
                    }
                    if btn(ui, ph::EJECT, "Safely remove") {
                        app.dispatch(Action::SafelyRemove {
                            id: id.clone(),
                            name: name.clone(),
                        });
                    }
                }
                if btn(ui, ph::ARROW_COUNTER_CLOCKWISE, "Restart device") {
                    app.dispatch(Action::Restart {
                        id: id.clone(),
                        name: name.clone(),
                    });
                }
                if i.is_disabled() {
                    if btn(ui, ph::POWER, "Enable device") {
                        app.dispatch(Action::SetEnabled {
                            id: id.clone(),
                            name: name.clone(),
                            enable: true,
                        });
                    }
                } else if dev.is_some() && btn(ui, ph::POWER, "Disable device") {
                    app.dispatch(Action::SetEnabled {
                        id: id.clone(),
                        name: name.clone(),
                        enable: false,
                    });
                }
                if btn(ui, ph::ARROW_SQUARE_OUT, "Device properties") {
                    app.dispatch(Action::Properties(id.clone()));
                }
            }
        }
        if let Some((hub, port)) = app.port_map.get(&n.id).cloned() {
            if n.kind != NodeKind::EmptyPort && btn(ui, ph::PLUGS, "Cycle port (re-plug)") {
                app.dispatch(Action::CyclePort {
                    hub,
                    port,
                    name: name.clone(),
                });
            }
        }
    }
    if let Some((v, p)) = dev.as_ref().and_then(|d| d.vid_pid()) {
        ui.separator();
        if btn(ui, ph::MAGNIFYING_GLASS, "Look up VID:PID online") {
            app.dispatch(Action::OpenPath(format!(
                "https://devicehunt.com/view/type/usb/vendor/{v:04X}/device/{p:04X}"
            )));
        }
    }
}

fn skeleton(ui: &mut Ui, p: &super::theme::Palette) {
    let t = ui.input(|i| i.time) as f32;
    for i in 0..10 {
        let (rect, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 30.0), Sense::hover());
        let depth = [0, 1, 2, 3, 3, 3, 4, 3, 1, 2][i] as f32;
        let x = rect.left() + 10.0 + depth * 18.0;
        let shimmer = ((t * 2.5 - i as f32 * 0.25).sin() * 0.5 + 0.5) * 0.5 + 0.3;
        let c = with_alpha(p.text_faint, shimmer * 0.35);
        ui.painter().rect_filled(
            Rect::from_min_size(Pos2::new(x, rect.center().y - 9.0), Vec2::splat(18.0)),
            CornerRadius::same(5),
            c,
        );
        let w = 90.0 + ((i * 37) % 120) as f32;
        ui.painter().rect_filled(
            Rect::from_min_size(
                Pos2::new(x + 28.0, rect.center().y - 5.0),
                Vec2::new(w, 10.0),
            ),
            CornerRadius::same(4),
            c,
        );
    }
    ui.ctx().request_repaint();
}
