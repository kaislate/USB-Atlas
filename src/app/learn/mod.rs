//! "Learn" view: an illustrated, animated guide to USB.

mod content;
mod diagrams;
mod graphs;

use egui::{Color32, CornerRadius, RichText, Sense, Stroke, Ui, Vec2};
use egui_phosphor::regular as ph;

use super::theme::{self, with_alpha, Palette};
use super::App;
use crate::model::Speed;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Chapter {
    HowItWorks,
    Connectors,
    Power,
    Names,
    Speeds,
    Companion,
    Enumeration,
    Transfers,
    Signaling,
    LinkPower,
    TypeC,
    Troubleshooting,
    YourPc,
}

impl Chapter {
    pub const GROUPS: [(&'static str, &'static [Chapter]); 5] = [
        (
            "Basics",
            &[Chapter::HowItWorks, Chapter::Connectors, Chapter::Power],
        ),
        ("Speeds & names", &[Chapter::Names, Chapter::Speeds]),
        ("Companion ports", &[Chapter::Companion]),
        (
            "Advanced",
            &[
                Chapter::Enumeration,
                Chapter::Transfers,
                Chapter::Signaling,
                Chapter::LinkPower,
                Chapter::TypeC,
                Chapter::Troubleshooting,
            ],
        ),
        ("Personal", &[Chapter::YourPc]),
    ];

    pub fn all() -> Vec<Chapter> {
        Self::GROUPS
            .iter()
            .flat_map(|g| g.1.iter().copied())
            .collect()
    }

    pub fn title(self) -> &'static str {
        match self {
            Chapter::HowItWorks => "How USB works",
            Chapter::Connectors => "Connectors",
            Chapter::Power => "Power",
            Chapter::Names => "Versions & names",
            Chapter::Speeds => "Speed comparison",
            Chapter::Companion => "Companion ports",
            Chapter::Enumeration => "Enumeration",
            Chapter::Transfers => "Transfer types",
            Chapter::Signaling => "Signaling & encoding",
            Chapter::LinkPower => "Link power states",
            Chapter::TypeC => "Type-C, PD & USB4",
            Chapter::Troubleshooting => "Troubleshooting",
            Chapter::YourPc => "Your computer",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Chapter::HowItWorks => ph::TREE_STRUCTURE,
            Chapter::Connectors => ph::PLUG,
            Chapter::Power => ph::BATTERY_FULL,
            Chapter::Names => ph::TEXT_AA,
            Chapter::Speeds => ph::CHART_LINE_UP,
            Chapter::Companion => ph::ARROWS_LEFT_RIGHT,
            Chapter::Enumeration => ph::HANDSHAKE,
            Chapter::Transfers => ph::FLOW_ARROW,
            Chapter::Signaling => ph::WAVE_SINE,
            Chapter::LinkPower => ph::TIMER,
            Chapter::TypeC => ph::ROCKET_LAUNCH,
            Chapter::Troubleshooting => ph::FIRST_AID,
            Chapter::YourPc => ph::DESKTOP_TOWER,
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            Chapter::HowItWorks => "Hosts, hubs, devices and why the computer is always in charge.",
            Chapter::Connectors => "From Type-A to Type-C, and which pins do what.",
            Chapter::Power => "How much current a port can give – from 100 mA to 240 W.",
            Chapter::Names => "USB 3.0 = 3.1 Gen 1 = 3.2 Gen 1 = USB 5Gbps. Untangled.",
            Chapter::Speeds => "Every generation side by side, plus a file-copy race.",
            Chapter::Companion => "Why every blue socket shows up as two ports.",
            Chapter::Enumeration => "What happens in the first 100 ms after you plug in.",
            Chapter::Transfers => {
                "Control, bulk, interrupt and isochronous – and the 125 µs microframe."
            }
            Chapter::Signaling => "Why a 5 Gbit/s link never moves 625 MB/s.",
            Chapter::LinkPower => "Suspend, LPM and the U0–U3 link states.",
            Chapter::TypeC => "Alt modes, Power Delivery, e-marked cables and USB4 tunnels.",
            Chapter::Troubleshooting => {
                "Code 43, over-current, slow drives and random disconnects."
            }
            Chapter::YourPc => "What this guide means for the hardware in front of you.",
        }
    }
}

pub struct LearnState {
    pub chapter: Chapter,
    pub opened_at: f64,
    pub linear: bool,
    pub race_start: f64,
    pub race_file: usize,
    pub scenario: usize,
    pub scenario_at: f64,
    pub needs_reset: bool,
}

impl Default for LearnState {
    fn default() -> Self {
        Self {
            chapter: Chapter::HowItWorks,
            opened_at: 0.0,
            linear: false,
            race_start: 0.0,
            race_file: 2,
            scenario: 0,
            scenario_at: 0.0,
            needs_reset: true,
        }
    }
}

impl LearnState {
    pub fn open(&mut self, c: Chapter) {
        self.chapter = c;
        self.needs_reset = true;
    }
}

pub fn show(app: &mut App, ui: &mut Ui) {
    let p = app.p;
    let now = ui.input(|i| i.time);
    if app.learn.needs_reset {
        app.learn.needs_reset = false;
        app.learn.opened_at = now;
        app.learn.race_start = now + 0.8;
        app.learn.scenario_at = now;
    }
    ui.ctx().request_repaint();

    egui::Panel::left("learn-nav")
        .exact_size(250.0)
        .frame(
            egui::Frame::new()
                .fill(p.panel)
                .inner_margin(egui::Margin {
                    left: 12,
                    right: 12,
                    top: 16,
                    bottom: 12,
                })
                .stroke(Stroke::new(1.0, p.border)),
        )
        .show(ui, |ui| nav(app, ui));

    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(p.bg))
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .id_salt(("learn", app.learn.chapter as u8))
                .show(ui, |ui| {
                    let avail = ui.available_width();
                    let w = avail.min(980.0);
                    let side = ((avail - w) / 2.0).max(0.0);
                    ui.horizontal(|ui| {
                        ui.add_space(side);
                        ui.vertical(|ui| {
                            ui.set_width(w - 48.0);
                            ui.add_space(26.0);
                            content::chapter(app, ui);
                            ui.add_space(20.0);
                            footer_nav(app, ui);
                            ui.add_space(40.0);
                        });
                    });
                });
        });
}

fn nav(app: &mut App, ui: &mut Ui) {
    let p = app.p;
    ui.horizontal(|ui| {
        widgets_tile(ui, &p, ph::GRADUATION_CAP, 30.0, p.accent);
        ui.vertical(|ui| {
            ui.label(RichText::new("USB Guide").font(theme::semibold(15.0)));
            ui.label(
                RichText::new("From basics to bits")
                    .size(11.5)
                    .color(p.text_muted),
            );
        });
    });
    ui.add_space(12.0);
    egui::ScrollArea::vertical()
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            for (group, chapters) in Chapter::GROUPS {
                ui.add_space(6.0);
                ui.label(
                    RichText::new(group.to_uppercase())
                        .size(10.5)
                        .strong()
                        .color(p.text_faint),
                );
                ui.add_space(2.0);
                for c in chapters {
                    let sel = app.learn.chapter == *c;
                    let (r, resp) = ui
                        .allocate_exact_size(Vec2::new(ui.available_width(), 30.0), Sense::click());
                    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                    if sel {
                        ui.painter()
                            .rect_filled(r, CornerRadius::same(8), p.accent_soft);
                        ui.painter().rect_filled(
                            egui::Rect::from_min_size(
                                r.min + Vec2::new(0.0, 6.0),
                                Vec2::new(3.0, r.height() - 12.0),
                            ),
                            CornerRadius::same(2),
                            p.accent,
                        );
                    } else if resp.hovered() {
                        ui.painter()
                            .rect_filled(r, CornerRadius::same(8), p.card_hover);
                    }
                    let c_col = if sel { p.accent } else { p.text_muted };
                    ui.painter().text(
                        r.left_center() + Vec2::new(16.0, 0.0),
                        egui::Align2::CENTER_CENTER,
                        c.icon(),
                        egui::FontId::proportional(15.0),
                        c_col,
                    );
                    ui.painter().text(
                        r.left_center() + Vec2::new(32.0, 0.0),
                        egui::Align2::LEFT_CENTER,
                        c.title(),
                        if sel {
                            theme::semibold(13.0)
                        } else {
                            egui::FontId::proportional(13.0)
                        },
                        if sel { p.text } else { p.text_muted },
                    );
                    if resp.clicked() {
                        app.learn.open(*c);
                    }
                }
            }
        });
}

fn footer_nav(app: &mut App, ui: &mut Ui) {
    let p = app.p;
    let all = Chapter::all();
    let i = all
        .iter()
        .position(|c| *c == app.learn.chapter)
        .unwrap_or(0);
    ui.separator();
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if i > 0 {
            let prev = all[i - 1];
            if ui
                .add(
                    egui::Button::new(
                        RichText::new(format!("{}  {}", ph::ARROW_LEFT, prev.title()))
                            .color(p.text_muted),
                    )
                    .frame(false),
                )
                .clicked()
            {
                app.learn.open(prev);
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if let Some(next) = all.get(i + 1).copied() {
                if ui
                    .add(
                        egui::Button::new(
                            RichText::new(format!("Next: {}  {}", next.title(), ph::ARROW_RIGHT))
                                .color(p.accent),
                        )
                        .corner_radius(CornerRadius::same(8)),
                    )
                    .clicked()
                {
                    app.learn.open(next);
                }
            }
        });
    });
}

// ---------- shared building blocks ----------

fn widgets_tile(ui: &mut Ui, p: &Palette, icon: &str, size: f32, color: Color32) {
    super::widgets::icon_tile(ui, p, icon, size, color);
}

pub(super) fn hero(ui: &mut Ui, p: &Palette, c: Chapter) {
    ui.horizontal(|ui| {
        widgets_tile(ui, p, c.icon(), 52.0, p.accent);
        ui.add_space(6.0);
        ui.vertical(|ui| {
            ui.label(RichText::new(c.title()).font(theme::semibold(26.0)));
            ui.label(RichText::new(c.blurb()).size(14.5).color(p.text_muted));
        });
    });
    ui.add_space(18.0);
}

pub(super) fn h2(ui: &mut Ui, p: &Palette, text: &str) {
    ui.add_space(14.0);
    ui.label(
        RichText::new(text)
            .font(theme::semibold(18.0))
            .color(p.text),
    );
    ui.add_space(4.0);
}

pub(super) fn para(ui: &mut Ui, p: &Palette, text: &str) {
    ui.add(egui::Label::new(RichText::new(text).size(14.0).color(p.text_muted)).wrap());
    ui.add_space(6.0);
}

pub(super) fn card<R>(ui: &mut Ui, p: &Palette, add: impl FnOnce(&mut Ui) -> R) -> R {
    let r = super::widgets::card(p)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner;
    ui.add_space(12.0);
    r
}

pub(super) fn callout(
    ui: &mut Ui,
    p: &Palette,
    icon: &str,
    color: Color32,
    title: &str,
    body: &str,
) {
    egui::Frame::new()
        .fill(p.soft(color))
        .stroke(Stroke::new(1.0, with_alpha(color, 0.35)))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(egui::Margin::symmetric(14, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(icon).size(18.0).color(color));
                ui.vertical(|ui| {
                    ui.label(RichText::new(title).strong().color(p.text));
                    ui.add(egui::Label::new(RichText::new(body).color(p.text_muted)).wrap());
                });
            });
        });
    ui.add_space(10.0);
}

/// Grid of "term – explanation" rows.
pub(super) fn facts(ui: &mut Ui, p: &Palette, id: &str, rows: &[(&str, &str)]) {
    egui::Grid::new(id)
        .num_columns(2)
        .spacing([18.0, 8.0])
        .min_col_width(150.0)
        .show(ui, |ui| {
            for (k, v) in rows {
                ui.label(RichText::new(*k).strong().color(p.text));
                ui.add(egui::Label::new(RichText::new(*v).color(p.text_muted)).wrap());
                ui.end_row();
            }
        });
}

/// Simple table with a header row.
pub(super) fn table(ui: &mut Ui, p: &Palette, id: &str, header: &[&str], rows: &[Vec<String>]) {
    let _ = id;
    // Column widths proportional to the longest text in each column.
    let n = header.len();
    let weights: Vec<f32> = (0..n)
        .map(|c| {
            let longest = rows
                .iter()
                .filter_map(|r| r.get(c))
                .map(|s| s.chars().count())
                .chain([header[c].chars().count()])
                .max()
                .unwrap_or(4);
            (longest as f32).clamp(6.0, 60.0)
        })
        .collect();
    let widths = column_widths(ui.available_width(), &weights, 0.0);
    table_row(ui, p, None, |ui| {
        for (i, h) in header.iter().enumerate() {
            table_cell(
                ui,
                widths[i],
                RichText::new(*h).size(12.0).strong().color(p.text_faint),
            );
        }
    });
    for (ri, r) in rows.iter().enumerate() {
        table_row(ui, p, Some(ri), |ui| {
            for (i, text) in r.iter().enumerate() {
                let rt = if i == 0 {
                    RichText::new(text).strong().color(p.text)
                } else {
                    RichText::new(text).color(p.text_muted)
                };
                table_cell(ui, widths[i.min(n - 1)], rt);
            }
        });
    }
}

pub(super) const TABLE_GAP: f32 = 14.0;

/// Splits `avail` into columns by weight after reserving `fixed` pixels.
pub(super) fn column_widths(avail: f32, weights: &[f32], fixed: f32) -> Vec<f32> {
    let n = weights.len() as f32 + if fixed > 0.0 { 1.0 } else { 0.0 };
    let free = avail - fixed - TABLE_GAP * (n - 1.0) - 24.0;
    let total: f32 = weights.iter().sum();
    weights
        .iter()
        .map(|w| (free * w / total).max(56.0))
        .collect()
}

/// One table row that grows to its tallest cell; odd rows get a stripe.
pub(super) fn table_row(ui: &mut Ui, p: &Palette, index: Option<usize>, add: impl FnOnce(&mut Ui)) {
    let bg = ui.painter().add(egui::Shape::Noop);
    let r = ui
        .horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = TABLE_GAP;
            ui.add_space(4.0);
            add(ui);
        })
        .response
        .rect;
    let row = r.expand2(Vec2::new(4.0, 5.0));
    if index.is_some_and(|i| i % 2 == 0) {
        ui.painter().set(
            bg,
            egui::Shape::rect_filled(row, CornerRadius::same(6), with_alpha(p.text_faint, 0.07)),
        );
    }
    if index.is_none() {
        ui.painter().set(
            bg,
            egui::Shape::line_segment(
                [row.left_bottom(), row.right_bottom()],
                Stroke::new(1.0, p.border),
            ),
        );
    }
    ui.add_space(10.0);
}

/// Fixed-width, top-aligned, wrapping cell.
pub(super) fn table_cell(ui: &mut Ui, w: f32, text: RichText) {
    ui.allocate_ui_with_layout(
        Vec2::new(w, 0.0),
        egui::Layout::top_down(egui::Align::Min),
        |ui| {
            ui.set_width(w);
            ui.add(egui::Label::new(text).wrap());
        },
    );
}

pub(super) fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// One USB speed generation, used by the graphs and the naming table.
pub(super) struct Tier {
    pub short: &'static str,
    pub title: &'static str,
    pub names: &'static str,
    /// Name of the signaling mode (Hi-Speed, SuperSpeed, SuperSpeed+ …).
    pub speed_name: &'static str,
    /// Name in the current specification.
    pub current_spec: &'static str,
    /// Earlier names for the same mode.
    pub former: &'static str,
    pub logo: &'static str,
    pub year: u16,
    pub mbps: f64,
    /// Typical real-world throughput in MB/s.
    pub real: f64,
    pub encoding: &'static str,
    pub lanes: &'static str,
    pub connectors: &'static str,
    /// Conventional color of the socket insert (not mandated).
    pub port_color: &'static str,
    pub speed: Option<Speed>,
}

pub(super) fn tiers() -> Vec<Tier> {
    vec![
        Tier {
            short: "1.5M",
            title: "Low Speed",
            names: "USB 1.0 / 1.1 Low Speed",
            speed_name: "Low Speed",
            current_spec: "USB 2.0 Low Speed",
            former: "USB 1.0, USB 1.1",
            logo: "–",
            year: 1996,
            mbps: 1.5,
            real: 0.15,
            encoding: "NRZI + bit stuffing",
            lanes: "1 half-duplex pair",
            connectors: "A, B",
            port_color: "White (USB 1.x era)",
            speed: Some(Speed::Low),
        },
        Tier {
            short: "12M",
            title: "Full Speed",
            names: "USB 1.1 Full Speed (also \"USB 2.0 Full Speed\")",
            speed_name: "Full Speed",
            current_spec: "USB 2.0 Full Speed",
            former: "USB 1.1",
            logo: "USB",
            year: 1998,
            mbps: 12.0,
            real: 1.0,
            encoding: "NRZI + bit stuffing",
            lanes: "1 half-duplex pair",
            connectors: "A, B, Mini, Micro, C",
            port_color: "White (USB 1.x era)",
            speed: Some(Speed::Full),
        },
        Tier {
            short: "480M",
            title: "Hi-Speed",
            names: "USB 2.0 Hi-Speed",
            speed_name: "Hi-Speed",
            current_spec: "USB 2.0 Hi-Speed",
            former: "–",
            logo: "Hi-Speed USB",
            year: 2000,
            mbps: 480.0,
            real: 40.0,
            encoding: "NRZI + bit stuffing",
            lanes: "1 half-duplex pair",
            connectors: "A, B, Mini, Micro, C",
            port_color: "Black (sometimes white)",
            speed: Some(Speed::High),
        },
        Tier {
            short: "5G",
            title: "USB 5Gbps",
            names: "USB 3.0 = USB 3.1 Gen 1 = USB 3.2 Gen 1 (Gen 1x1) · \"SuperSpeed\"",
            speed_name: "SuperSpeed",
            current_spec: "USB 3.2 Gen 1 (Gen 1x1)",
            former: "USB 3.0 · USB 3.1 Gen 1",
            logo: "USB 5Gbps",
            year: 2008,
            mbps: 5_000.0,
            real: 450.0,
            encoding: "8b/10b (80 % efficient)",
            lanes: "1 lane (TX + RX pairs)",
            connectors: "A, B, Micro-B SS, C",
            port_color: "Blue",
            speed: Some(Speed::Super),
        },
        Tier {
            short: "10G",
            title: "USB 10Gbps",
            names: "USB 3.1 Gen 2 = USB 3.2 Gen 2 (Gen 2x1) · \"SuperSpeed+\"",
            speed_name: "SuperSpeed+",
            current_spec: "USB 3.2 Gen 2 (Gen 2x1)",
            former: "USB 3.1 Gen 2",
            logo: "USB 10Gbps",
            year: 2013,
            mbps: 10_000.0,
            real: 1_050.0,
            encoding: "128b/132b (97 % efficient)",
            lanes: "1 lane",
            connectors: "A, C",
            port_color: "Teal (some brands: red)",
            speed: Some(Speed::SuperPlus),
        },
        Tier {
            short: "20G",
            title: "USB 20Gbps",
            names: "USB 3.2 Gen 2x2 · \"SuperSpeed+ 20\"",
            speed_name: "SuperSpeed+ 20Gbps",
            current_spec: "USB 3.2 Gen 2x2",
            former: "–",
            logo: "USB 20Gbps",
            year: 2017,
            mbps: 20_000.0,
            real: 2_000.0,
            encoding: "128b/132b",
            lanes: "2 lanes",
            connectors: "C only",
            port_color: "Type-C – no color; \"SS 20\" logo",
            speed: Some(Speed::SuperPlus20),
        },
        Tier {
            short: "40G",
            title: "USB 40Gbps",
            names: "USB4 (Gen 3x2) · compatible with Thunderbolt 3 / 4",
            speed_name: "USB4 40Gbps",
            current_spec: "USB4 Gen 3x2",
            former: "USB4 v1.0 (basis of Thunderbolt 4)",
            logo: "USB 40Gbps",
            year: 2019,
            mbps: 40_000.0,
            real: 3_800.0,
            encoding: "128b/132b",
            lanes: "2 lanes, tunneled",
            connectors: "C only",
            port_color: "Type-C – \"40\" or Thunderbolt ⚡ logo",
            speed: None,
        },
        Tier {
            short: "80G",
            title: "USB 80Gbps",
            names: "USB4 Version 2.0 (Gen 4, PAM3) · Thunderbolt 5 (up to 120 Gbit/s one way)",
            speed_name: "USB4 80Gbps",
            current_spec: "USB4 Version 2.0 (Gen 4)",
            former: "– (basis of Thunderbolt 5)",
            logo: "USB 80Gbps",
            year: 2022,
            mbps: 80_000.0,
            real: 7_500.0,
            encoding: "PAM3, 11b/7t",
            lanes: "2 lanes, tunneled",
            connectors: "C only",
            port_color: "Type-C – \"80\" or Thunderbolt ⚡ logo",
            speed: None,
        },
    ]
}

pub(super) fn tier_color(p: &Palette, t: &Tier) -> Color32 {
    match t.speed {
        Some(s) => p.speed(s),
        None if t.mbps < 60_000.0 => {
            if p.dark {
                Color32::from_rgb(0xA7, 0x8B, 0xFA)
            } else {
                Color32::from_rgb(0x6D, 0x28, 0xD9)
            }
        }
        None => {
            if p.dark {
                Color32::from_rgb(0xFB, 0xBF, 0x24)
            } else {
                Color32::from_rgb(0xB4, 0x53, 0x09)
            }
        }
    }
}

pub(super) fn fmt_rate(mbps: f64) -> String {
    if mbps >= 1000.0 {
        format!("{} Gbit/s", trim(mbps / 1000.0))
    } else {
        format!("{} Mbit/s", trim(mbps))
    }
}

fn trim(v: f64) -> String {
    if (v - v.round()).abs() < 0.05 {
        format!("{:.0}", v)
    } else {
        format!("{:.1}", v)
    }
}

pub(super) fn fmt_duration(secs: f64) -> String {
    if secs < 1.0 {
        format!("{:.1} s", secs)
    } else if secs < 60.0 {
        format!("{:.0} s", secs)
    } else if secs < 3600.0 {
        format!("{} min {:02} s", (secs / 60.0) as u64, (secs % 60.0) as u64)
    } else if secs < 86_400.0 {
        format!(
            "{} h {:02} min",
            (secs / 3600.0) as u64,
            ((secs % 3600.0) / 60.0) as u64
        )
    } else {
        format!("{:.1} days", secs / 86_400.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiers_are_ordered_and_consistent() {
        let t = tiers();
        assert!(t.windows(2).all(|w| w[0].mbps < w[1].mbps));
        // Real throughput never exceeds the signaling rate.
        assert!(t.iter().all(|x| x.real * 8.0 < x.mbps));
    }

    #[test]
    fn formatting() {
        assert_eq!(fmt_rate(480.0), "480 Mbit/s");
        assert_eq!(fmt_rate(5000.0), "5 Gbit/s");
        assert_eq!(fmt_rate(1.5), "1.5 Mbit/s");
        assert_eq!(fmt_duration(625.0), "10 min 25 s");
        assert_eq!(fmt_duration(7200.0), "2 h 00 min");
    }
}
