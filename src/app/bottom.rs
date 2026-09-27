//! Collapsible bottom panel: activity timeline and insights list.

use egui::{CornerRadius, RichText, Stroke, Ui};
use egui_phosphor::regular as ph;

use super::{widgets, Action, App, BottomTab};
use crate::insights::Severity;
use crate::tree::ChangeKind;

pub fn show(app: &mut App, ui: &mut Ui) {
    let p = app.p;
    let mut open = app.bottom_open;
    let mut close_clicked = false;
    egui::Panel::bottom("bottom")
        .resizable(true)
        .default_size(220.0)
        .size_range(120.0..=600.0)
        .frame(
            egui::Frame::new()
                .fill(p.panel)
                .inner_margin(egui::Margin::symmetric(14, 8))
                .stroke(Stroke::new(1.0, p.border)),
        )
        .show_collapsible(ui, &mut open, |ui| {
            ui.horizontal(|ui| {
                let mut tab = app.bottom_tab;
                let ev = format!(
                    "{}  Activity  {}",
                    ph::CLOCK_COUNTER_CLOCKWISE,
                    app.events.len()
                );
                let ins = format!("{}  Insights  {}", ph::LIGHTBULB, app.insights.len());
                widgets::segmented(
                    ui,
                    &p,
                    "bottom-tabs",
                    &mut tab,
                    &[
                        (BottomTab::Activity, ev.as_str()),
                        (BottomTab::Insights, ins.as_str()),
                    ],
                );
                app.bottom_tab = tab;
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::icon_button(ui, &p, ph::X, "Close (Ctrl+J)").clicked() {
                        close_clicked = true;
                    }
                    if app.bottom_tab == BottomTab::Activity && !app.events.is_empty() {
                        if widgets::icon_button(ui, &p, ph::TRASH, "Clear activity").clicked() {
                            app.events.clear();
                        }
                        if widgets::icon_button(ui, &p, ph::COPY, "Copy activity log").clicked() {
                            let log = app
                                .events
                                .iter()
                                .map(|e| {
                                    format!("{}  {:?}  {}  {}", e.time, e.kind, e.name, e.node_id)
                                })
                                .collect::<Vec<_>>()
                                .join("\n");
                            app.dispatch(Action::CopyText(log));
                        }
                    }
                });
            });
            ui.add_space(6.0);
            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .stick_to_bottom(app.bottom_tab == BottomTab::Activity)
                .show(ui, |ui| match app.bottom_tab {
                    BottomTab::Activity => activity(app, ui),
                    BottomTab::Insights => insights(app, ui),
                });
        });
    // `open` also reflects drag-to-collapse on the panel edge.
    app.bottom_open = open && !close_clicked;
}

fn activity(app: &mut App, ui: &mut Ui) {
    let p = app.p;
    if app.events.is_empty() {
        ui.label(RichText::new("No device changes yet. Plug something in – arrivals and removals are logged here with timestamps.").color(p.text_muted));
        return;
    }
    let mut jump = None;
    for e in app.events.iter() {
        let (icon, color, verb) = match e.kind {
            ChangeKind::Arrived => (ph::PLUGS_CONNECTED, p.ok, "connected"),
            ChangeKind::Removed => (ph::PLUG, p.text_muted, "disconnected"),
            ChangeKind::ProblemAppeared(_) => (ph::WARNING, p.error, "reported a problem"),
            ChangeKind::ProblemCleared => (ph::CHECK_CIRCLE, p.ok, "problem cleared"),
        };
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(&e.time[e.time.len().saturating_sub(8)..])
                    .monospace()
                    .size(12.0)
                    .color(p.text_faint),
            );
            ui.label(RichText::new(icon).color(color));
            let exists = app.flat.by_id.contains_key(&e.node_id);
            let name = RichText::new(&e.name).strong();
            if exists && e.kind != ChangeKind::Removed {
                if ui.link(name).clicked() {
                    jump = Some(e.node_id.clone());
                }
            } else {
                ui.label(name);
            }
            ui.label(RichText::new(verb).color(p.text_muted));
            if let Some((v, pi)) = e.vid_pid {
                ui.label(
                    RichText::new(format!("{v:04X}:{pi:04X}"))
                        .monospace()
                        .size(11.5)
                        .color(p.text_faint),
                );
            }
        });
    }
    if let Some(j) = jump {
        app.dispatch(Action::Select(j));
    }
}

fn insights(app: &mut App, ui: &mut Ui) {
    let p = app.p;
    if app.insights.is_empty() {
        ui.horizontal(|ui| {
            ui.label(RichText::new(ph::CHECK_CIRCLE).color(p.ok).size(18.0));
            ui.label(RichText::new("Everything looks healthy – no problems, speed bottlenecks or power issues found.").color(p.text_muted));
        });
        return;
    }
    let mut jump = None;
    for i in &app.insights {
        let (c, icon) = match i.severity {
            Severity::Error => (p.error, ph::X_CIRCLE),
            Severity::Warning => (p.warn, ph::WARNING),
            Severity::Info => (p.info, ph::LIGHTBULB),
        };
        let r = egui::Frame::new()
            .corner_radius(CornerRadius::same(8))
            .inner_margin(egui::Margin::symmetric(8, 5))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(RichText::new(icon).color(c));
                    ui.label(RichText::new(&i.title).strong());
                    let first = i.detail.lines().next().unwrap_or("");
                    ui.add(egui::Label::new(RichText::new(first).color(p.text_muted)).truncate());
                });
            })
            .response
            .interact(egui::Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(&i.detail);
        if r.hovered() {
            ui.painter().rect_filled(
                r.rect,
                CornerRadius::same(8),
                super::theme::with_alpha(p.accent, 0.06),
            );
        }
        if r.clicked() {
            jump = Some(i.node_id.clone());
        }
    }
    if let Some(j) = jump {
        app.dispatch(Action::Select(j));
    }
}
