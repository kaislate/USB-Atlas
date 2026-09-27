//! Animated notification toasts (bottom-right).

use std::time::{Duration, Instant};

use egui::{Align2, CornerRadius, Id, RichText, Stroke};
use egui_phosphor::regular as ph;

use super::theme::with_alpha;
use super::{Action, App};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Error,
    Arrived,
    Removed,
}

pub struct Toast {
    pub kind: ToastKind,
    pub title: String,
    pub body: String,
    pub born: Instant,
    pub target: Option<String>,
    pub open: Option<String>,
    id: u64,
}

impl Toast {
    pub fn new(kind: ToastKind, title: String, body: String) -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        Self {
            kind,
            title,
            body,
            born: Instant::now(),
            target: None,
            open: None,
            id: NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        }
    }

    pub fn with_target(mut self, id: String) -> Self {
        self.target = Some(id);
        self
    }

    pub fn with_open(mut self, path: String) -> Self {
        self.open = Some(path);
        self
    }

    fn lifetime(&self) -> Duration {
        match self.kind {
            ToastKind::Error => Duration::from_secs(8),
            _ => Duration::from_millis(4500),
        }
    }
}

pub fn show(app: &mut App, ctx: &egui::Context) {
    let p = app.p;
    app.toasts.retain(|t| t.born.elapsed() < t.lifetime());
    if app.toasts.len() > 5 {
        let n = app.toasts.len() - 5;
        app.toasts.drain(..n);
    }
    let mut y = -40.0;
    let mut clicked: Option<Action> = None;
    let mut dismiss: Option<u64> = None;
    for t in app.toasts.iter().rev() {
        let age = t.born.elapsed().as_secs_f32();
        let life = t.lifetime().as_secs_f32();
        let appear = (age / 0.18).min(1.0);
        let fade = ((life - age) / 0.35).clamp(0.0, 1.0);
        let alpha = appear.min(fade);
        let slide = (1.0 - appear) * 40.0;
        let (color, icon) = match t.kind {
            ToastKind::Info => (p.info, ph::INFO),
            ToastKind::Success => (p.ok, ph::CHECK_CIRCLE),
            ToastKind::Error => (p.error, ph::X_CIRCLE),
            ToastKind::Arrived => (p.ok, ph::PLUGS_CONNECTED),
            ToastKind::Removed => (p.text_muted, ph::PLUG),
        };
        let area = egui::Area::new(Id::new(("toast", t.id)))
            .anchor(Align2::RIGHT_BOTTOM, egui::vec2(-16.0 + slide, y))
            .order(egui::Order::Foreground)
            .interactable(true);
        let resp = area.show(ctx, |ui| {
            ui.set_opacity(alpha);
            egui::Frame::new()
                .fill(p.card)
                .stroke(Stroke::new(1.0, p.border))
                .corner_radius(CornerRadius::same(12))
                .shadow(egui::Shadow { offset: [0, 6], blur: 20, spread: 0, color: egui::Color32::from_black_alpha(if p.dark { 120 } else { 40 }) })
                .inner_margin(egui::Margin::symmetric(14, 10))
                .show(ui, |ui| {
                    ui.set_width(300.0);
                    ui.horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(egui::vec2(30.0, 30.0), egui::Sense::hover());
                        ui.painter().rect_filled(r, CornerRadius::same(8), with_alpha(color, 0.16));
                        ui.painter().text(r.center(), Align2::CENTER_CENTER, icon, egui::FontId::proportional(17.0), color);
                        ui.vertical(|ui| {
                            ui.label(RichText::new(&t.title).strong().color(p.text));
                            if !t.body.is_empty() {
                                ui.add(egui::Label::new(RichText::new(&t.body).size(12.0).color(p.text_muted)).wrap());
                            }
                            if t.target.is_some() || t.open.is_some() {
                                ui.label(RichText::new(if t.open.is_some() { "Click to open" } else { "Click to show" }).size(11.0).color(p.accent));
                            }
                        });
                    });
                    // Progress line
                    let r = ui.min_rect();
                    let frac = 1.0 - age / life;
                    let line = egui::Rect::from_min_size(egui::pos2(r.left(), r.bottom() + 6.0), egui::vec2(r.width() * frac, 2.0));
                    ui.painter().rect_filled(line, CornerRadius::same(1), with_alpha(color, 0.5));
                })
        });
        let r = resp.response.interact(egui::Sense::click());
        if r.clicked() {
            if let Some(tg) = &t.target {
                clicked = Some(Action::Select(tg.clone()));
            } else if let Some(o) = &t.open {
                clicked = Some(Action::OpenPath(o.clone()));
            }
            dismiss = Some(t.id);
        }
        y -= resp.response.rect.height() + 10.0;
    }
    if let Some(a) = clicked {
        app.dispatch(a);
    }
    if let Some(id) = dismiss {
        app.toasts.retain(|t| t.id != id);
    }
}
