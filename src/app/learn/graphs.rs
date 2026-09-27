//! Animated speed graph and file-copy race.

use egui::{Align2, Color32, CornerRadius, FontId, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2};
use egui_phosphor::regular as ph;

use super::{ease_out, fmt_duration, fmt_rate, tier_color, tiers, App};
use crate::app::theme::{self, with_alpha};
use crate::app::widgets;

/// Horizontal bar chart of every USB generation. Log or linear scale, with
/// grow-in animation, flowing particles and a moving shine.
pub fn speed_graph(app: &mut App, ui: &mut Ui) {
    let p = app.p;
    let now = ui.input(|i| i.time);
    let t = (now - app.learn.opened_at) as f32;
    ui.horizontal(|ui| {
        ui.label(RichText::new("Scale").color(p.text_muted));
        let mut lin = app.learn.linear;
        widgets::segmented(
            ui,
            &p,
            "learn-scale",
            &mut lin,
            &[(false, "Logarithmic"), (true, "Linear")],
        );
        if lin != app.learn.linear {
            app.learn.linear = lin;
        }
        ui.label(
            RichText::new(if app.learn.linear {
                "Linear: the true proportions – USB 2 almost vanishes."
            } else {
                "Logarithmic: every bar step is 10× faster."
            })
            .size(12.0)
            .color(p.text_faint),
        );
    });
    ui.add_space(10.0);

    let s = ui
        .ctx()
        .animate_bool_with_time(egui::Id::new("learn-lin"), app.learn.linear, 0.6);
    let s = ease_out(s);
    let tiers = tiers();
    let label_w = 190.0;
    let value_w = 110.0;
    let row_h = 46.0;
    let (rect, resp) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), row_h * tiers.len() as f32 + 34.0),
        Sense::hover(),
    );
    let painter = ui.painter_at(rect.expand(4.0));
    let bar_left = rect.left() + label_w;
    let bar_w = (rect.width() - label_w - value_w).max(100.0);
    let (lmin, lmax) = (0.0_f64, 5.0_f64); // 1 Mbit/s .. 100 Gbit/s
    let log_frac = |m: f64| (((m.log10() - lmin) / (lmax - lmin)) as f32).clamp(0.0, 1.0);
    let lin_frac = |m: f64| (m / 80_000.0) as f32;
    let frac = |m: f64| log_frac(m) * (1.0 - s) + lin_frac(m) * s;

    // Grid lines (cross-fade between scales).
    let grid_top = rect.top();
    let grid_bot = rect.top() + row_h * tiers.len() as f32;
    for (m, label) in [
        (1.0, "1M"),
        (10.0, "10M"),
        (100.0, "100M"),
        (1_000.0, "1G"),
        (10_000.0, "10G"),
        (100_000.0, "100G"),
    ] {
        let x = bar_left + log_frac(m) * bar_w;
        let a = 1.0 - s;
        painter.line_segment(
            [Pos2::new(x, grid_top), Pos2::new(x, grid_bot)],
            Stroke::new(1.0, with_alpha(p.guide, a)),
        );
        painter.text(
            Pos2::new(x, grid_bot + 12.0),
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(11.0),
            with_alpha(p.text_faint, a),
        );
    }
    for (m, label) in [
        (0.0, "0"),
        (20_000.0, "20G"),
        (40_000.0, "40G"),
        (60_000.0, "60G"),
        (80_000.0, "80G"),
    ] {
        let x = bar_left + lin_frac(m) * bar_w;
        painter.line_segment(
            [Pos2::new(x, grid_top), Pos2::new(x, grid_bot)],
            Stroke::new(1.0, with_alpha(p.guide, s)),
        );
        painter.text(
            Pos2::new(x, grid_bot + 12.0),
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(11.0),
            with_alpha(p.text_faint, s),
        );
    }

    let hover = resp.hover_pos();
    let mut hovered = None;
    for (i, tier) in tiers.iter().enumerate() {
        let y = rect.top() + i as f32 * row_h;
        let row = Rect::from_min_size(Pos2::new(rect.left(), y), Vec2::new(rect.width(), row_h));
        let is_hot = hover.is_some_and(|h| row.contains(h));
        if is_hot {
            hovered = Some(i);
            painter.rect_filled(
                row.shrink2(Vec2::new(0.0, 2.0)),
                CornerRadius::same(8),
                p.card_hover,
            );
        }
        let c = tier_color(&p, tier);
        // Labels
        painter.text(
            Pos2::new(rect.left() + 8.0, y + 16.0),
            Align2::LEFT_CENTER,
            tier.title,
            theme::semibold(13.5),
            p.text,
        );
        painter.text(
            Pos2::new(rect.left() + 8.0, y + 32.0),
            Align2::LEFT_CENTER,
            format!("{} · {}", tier.year, tier.short),
            FontId::proportional(11.0),
            p.text_faint,
        );
        // Bar
        let grow = ease_out((t - 0.12 * i as f32) / 1.1);
        let w = (frac(tier.mbps) * bar_w * grow).max(3.0);
        let bar = Rect::from_min_size(Pos2::new(bar_left, y + 12.0), Vec2::new(w, row_h - 24.0));
        let track = Rect::from_min_size(bar.min, Vec2::new(bar_w, bar.height()));
        painter.rect_filled(track, CornerRadius::same(6), with_alpha(p.guide, 0.35));
        painter.rect_filled(bar.expand(3.0), CornerRadius::same(8), with_alpha(c, 0.14));
        painter.rect_filled(bar, CornerRadius::same(6), with_alpha(c, 0.9));
        // Moving shine
        let bar_painter = painter.with_clip_rect(bar);
        let shine_x =
            bar.left() + ((t * 0.35 + i as f32 * 0.13).fract()) * (bar.width() + 80.0) - 40.0;
        for k in 0..6 {
            let a = 0.10 - k as f32 * 0.015;
            bar_painter.rect_filled(
                Rect::from_center_size(
                    Pos2::new(shine_x, bar.center().y),
                    Vec2::new(12.0 + k as f32 * 10.0, bar.height()),
                ),
                CornerRadius::ZERO,
                with_alpha(Color32::WHITE, a.max(0.0)),
            );
        }
        // Particles: flow speed ~ log(speed)
        let flow = 0.15 + (tier.mbps.log10() as f32) * 0.22;
        let n = ((bar.width() / 26.0) as usize).clamp(1, 40);
        for k in 0..n {
            let ph = ((t * flow * 60.0 / bar.width().max(30.0)) + k as f32 / n as f32).fract();
            let x = bar.left() + ph * bar.width();
            let yy = bar.center().y + ((k * 7919) % 5) as f32 - 2.0;
            bar_painter.circle_filled(Pos2::new(x, yy), 1.6, with_alpha(Color32::WHITE, 0.55));
        }
        // Value label (counts up while growing)
        if grow > 0.05 {
            let shown = tier.mbps * (0.4 + 0.6 * grow as f64);
            painter.text(
                Pos2::new(bar.right() + 10.0, bar.center().y),
                Align2::LEFT_CENTER,
                fmt_rate(shown),
                theme::semibold(12.5),
                with_alpha(c, grow),
            );
        }
    }

    if let Some(i) = hovered {
        let tier = &tiers[i];
        resp.on_hover_ui_at_pointer(|ui| {
            ui.set_max_width(340.0);
            ui.label(
                RichText::new(tier.title)
                    .font(theme::semibold(14.0))
                    .color(tier_color(&p, tier)),
            );
            ui.label(RichText::new(tier.names).size(12.5));
            ui.add_space(4.0);
            ui.label(
                RichText::new(format!("Signaling rate: {}", fmt_rate(tier.mbps)))
                    .size(12.0)
                    .color(p.text_muted),
            );
            ui.label(
                RichText::new(format!(
                    "Typical real transfer: ~{} MB/s",
                    fmt_mb(tier.real)
                ))
                .size(12.0)
                .color(p.text_muted),
            );
            ui.label(
                RichText::new(format!("Encoding: {}", tier.encoding))
                    .size(12.0)
                    .color(p.text_muted),
            );
            ui.label(
                RichText::new(format!("Lanes: {}", tier.lanes))
                    .size(12.0)
                    .color(p.text_muted),
            );
            ui.label(
                RichText::new(format!("Logo / marketing name: {}", tier.logo))
                    .size(12.0)
                    .color(p.text_muted),
            );
            ui.label(
                RichText::new(format!("Connectors: {}", tier.connectors))
                    .size(12.0)
                    .color(p.text_muted),
            );
            ui.label(
                RichText::new(format!("Typical port color: {}", tier.port_color))
                    .size(12.0)
                    .color(p.text_muted),
            );
        });
    }
}

fn fmt_mb(v: f64) -> String {
    if v >= 1.0 {
        format!("{:.0}", v)
    } else {
        format!("{:.2}", v)
    }
}

const FILES: [(&str, f64, &str); 5] = [
    ("Photo", 6.0, "a 6 MB photo"),
    ("Album", 120.0, "a 120 MB music album"),
    ("4K movie", 25_000.0, "a 25 GB 4K movie"),
    ("Game", 120_000.0, "a 120 GB game"),
    ("Backup", 1_000_000.0, "a 1 TB backup"),
];

/// Animated race: how long a real file takes at each generation's typical speed.
pub fn race(app: &mut App, ui: &mut Ui) {
    let p = app.p;
    let now = ui.input(|i| i.time);
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("Copy").color(p.text_muted));
        let mut sel = app.learn.race_file;
        let opts: Vec<(usize, &str)> = FILES.iter().enumerate().map(|(i, f)| (i, f.0)).collect();
        if widgets::segmented(ui, &p, "race-file", &mut sel, &opts) {
            app.learn.race_file = sel;
            app.learn.race_start = now;
        }
        if ui
            .add(egui::Button::new(RichText::new(format!(
                "{}  Restart",
                ph::ARROW_CLOCKWISE
            ))))
            .clicked()
        {
            app.learn.race_start = now;
        }
    });
    let (fname, size_mb, fdesc) = FILES[app.learn.race_file];
    let _ = fname;
    let tiers: Vec<_> = tiers().into_iter().filter(|t| t.mbps >= 12.0).collect();
    let fastest = size_mb / tiers.last().unwrap().real;
    // The fastest link finishes in 2 s of animation; everything else keeps real proportions.
    let k = fastest / 2.0;
    let elapsed = ((now - app.learn.race_start).max(0.0)) * k;
    ui.add_space(4.0);
    ui.label(
        RichText::new(format!(
            "Simulated time: {}  ·  copying {fdesc}",
            fmt_duration(elapsed)
        ))
        .size(12.5)
        .color(p.text_faint),
    );
    ui.add_space(8.0);
    let label_w = 150.0;
    let time_w = 130.0;
    for tier in &tiers {
        let total = size_mb / tier.real;
        let prog = (elapsed / total).min(1.0) as f32;
        let c = tier_color(&p, tier);
        let (rect, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 30.0), Sense::hover());
        ui.painter().text(
            Pos2::new(rect.left(), rect.center().y),
            Align2::LEFT_CENTER,
            tier.title,
            theme::semibold(13.0),
            p.text,
        );
        let track = Rect::from_min_max(
            Pos2::new(rect.left() + label_w, rect.center().y - 7.0),
            Pos2::new(rect.right() - time_w, rect.center().y + 7.0),
        );
        ui.painter()
            .rect_filled(track, CornerRadius::same(7), with_alpha(p.guide, 0.35));
        let mut fill = track;
        fill.set_width((track.width() * prog).max(2.0));
        ui.painter().rect_filled(fill, CornerRadius::same(7), c);
        if prog < 1.0 && prog > 0.0 {
            // Leading glow on the moving edge.
            ui.painter().circle_filled(
                Pos2::new(fill.right(), fill.center().y),
                9.0,
                with_alpha(c, 0.25),
            );
            ui.painter().circle_filled(
                Pos2::new(fill.right(), fill.center().y),
                4.0,
                Color32::WHITE,
            );
        }
        let done = prog >= 1.0;
        let txt = if done {
            format!("{} {}", ph::CHECK, fmt_duration(total))
        } else {
            fmt_duration(total)
        };
        ui.painter().text(
            Pos2::new(rect.right(), rect.center().y),
            Align2::RIGHT_CENTER,
            txt,
            if done {
                theme::semibold(12.5)
            } else {
                FontId::proportional(12.5)
            },
            if done { c } else { p.text_muted },
        );
    }
    ui.add_space(6.0);
    ui.label(RichText::new("Times use typical real-world throughput, not the headline rate – see Signaling & encoding for why.").size(11.5).color(p.text_faint));
}
