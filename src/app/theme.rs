//! Colors, fonts and widget styling.

use std::sync::Arc;

use egui::{
    Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Stroke, TextStyle,
};
use serde::{Deserialize, Serialize};

use crate::model::Speed;
use crate::tree::Health;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ThemeMode {
    #[default]
    System,
    Dark,
    Light,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Accent {
    #[default]
    Indigo,
    Teal,
    Rose,
    Amber,
    Violet,
}

impl Accent {
    pub const ALL: [Accent; 5] = [
        Accent::Indigo,
        Accent::Teal,
        Accent::Rose,
        Accent::Amber,
        Accent::Violet,
    ];

    pub fn color(self, dark: bool) -> Color32 {
        match (self, dark) {
            (Accent::Indigo, true) => Color32::from_rgb(0x8B, 0x9C, 0xFF),
            (Accent::Indigo, false) => Color32::from_rgb(0x4F, 0x5B, 0xD5),
            (Accent::Teal, true) => Color32::from_rgb(0x4F, 0xD1, 0xC5),
            (Accent::Teal, false) => Color32::from_rgb(0x0F, 0x8A, 0x80),
            (Accent::Rose, true) => Color32::from_rgb(0xFF, 0x7A, 0x9A),
            (Accent::Rose, false) => Color32::from_rgb(0xD0, 0x3A, 0x62),
            (Accent::Amber, true) => Color32::from_rgb(0xFF, 0xC1, 0x5E),
            (Accent::Amber, false) => Color32::from_rgb(0xB8, 0x6E, 0x00),
            (Accent::Violet, true) => Color32::from_rgb(0xC0, 0x8B, 0xFF),
            (Accent::Violet, false) => Color32::from_rgb(0x7C, 0x3A, 0xD8),
        }
    }
}

/// User overrides for connection colors (None = theme default).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct LinkColors {
    /// Low, Full, High, SuperSpeed, SuperSpeed+ 10G, SuperSpeed+ 20G.
    pub speeds: [Option<[u8; 3]>; 6],
    pub idle: Option<[u8; 3]>,
    pub slow_lane: Option<[u8; 3]>,
}

pub const LINK_SPEEDS: [Speed; 6] = [
    Speed::Low,
    Speed::Full,
    Speed::High,
    Speed::Super,
    Speed::SuperPlus,
    Speed::SuperPlus20,
];

pub fn speed_index(s: Speed) -> Option<usize> {
    LINK_SPEEDS.iter().position(|x| *x == s)
}

impl LinkColors {
    pub fn is_default(&self) -> bool {
        *self == LinkColors::default()
    }

    /// Named presets: (name, colors).
    pub fn presets() -> Vec<(&'static str, LinkColors)> {
        let rgb = |h: u32| Some([(h >> 16) as u8, (h >> 8) as u8, h as u8]);
        vec![
            ("Default", LinkColors::default()),
            (
                "Colorblind-safe",
                // Okabe–Ito palette.
                LinkColors {
                    speeds: [
                        rgb(0x999999),
                        rgb(0x56B4E9),
                        rgb(0xE69F00),
                        rgb(0x0072B2),
                        rgb(0x009E73),
                        rgb(0xCC79A7),
                    ],
                    idle: rgb(0x6B7280),
                    slow_lane: rgb(0xF0E442),
                },
            ),
            (
                "Traffic light",
                LinkColors {
                    speeds: [
                        rgb(0xEF4444),
                        rgb(0xF97316),
                        rgb(0xEAB308),
                        rgb(0x84CC16),
                        rgb(0x22C55E),
                        rgb(0x10B981),
                    ],
                    idle: rgb(0x64748B),
                    slow_lane: rgb(0xF43F5E),
                },
            ),
            (
                "Neon",
                LinkColors {
                    speeds: [
                        rgb(0x94A3B8),
                        rgb(0x38BDF8),
                        rgb(0xE879F9),
                        rgb(0x22D3EE),
                        rgb(0xA3E635),
                        rgb(0xFB7185),
                    ],
                    idle: rgb(0x475569),
                    slow_lane: rgb(0xFACC15),
                },
            ),
            (
                "Ocean",
                LinkColors {
                    speeds: [
                        rgb(0x9CA3AF),
                        rgb(0x7DD3FC),
                        rgb(0x60A5FA),
                        rgb(0x3B82F6),
                        rgb(0x2DD4BF),
                        rgb(0x818CF8),
                    ],
                    idle: rgb(0x64748B),
                    slow_lane: rgb(0xFB923C),
                },
            ),
        ]
    }
}

/// Semantic color palette used by every view.
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub dark: bool,
    pub bg: Color32,
    pub panel: Color32,
    pub card: Color32,
    pub card_hover: Color32,
    pub border: Color32,
    pub text: Color32,
    pub text_muted: Color32,
    pub text_faint: Color32,
    pub accent: Color32,
    pub accent_soft: Color32,
    pub ok: Color32,
    pub info: Color32,
    pub warn: Color32,
    pub error: Color32,
    pub guide: Color32,
    pub hex_hi: Color32,
    /// Custom per-speed link colors.
    pub link: [Option<Color32>; 6],
    /// Dashed line for lanes without a device.
    pub idle_link: Color32,
    /// Highlight for devices on a slower lane than available.
    pub slow_lane: Color32,
}

fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

pub fn with_alpha(c: Color32, a: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (a.clamp(0.0, 1.0) * 255.0) as u8)
}

impl Palette {
    pub fn new(dark: bool, accent: Accent) -> Self {
        let accent_c = accent.color(dark);
        if dark {
            let bg = Color32::from_rgb(0x0E, 0x10, 0x14);
            Self {
                dark,
                bg,
                panel: Color32::from_rgb(0x13, 0x16, 0x1B),
                card: Color32::from_rgb(0x19, 0x1D, 0x24),
                card_hover: Color32::from_rgb(0x20, 0x25, 0x2E),
                border: Color32::from_rgb(0x27, 0x2D, 0x37),
                text: Color32::from_rgb(0xE8, 0xEB, 0xF1),
                text_muted: Color32::from_rgb(0x96, 0x9E, 0xAD),
                text_faint: Color32::from_rgb(0x5E, 0x66, 0x74),
                accent: accent_c,
                accent_soft: mix(bg, accent_c, 0.18),
                ok: Color32::from_rgb(0x4A, 0xDE, 0x80),
                info: Color32::from_rgb(0x60, 0xA5, 0xFA),
                warn: Color32::from_rgb(0xFB, 0xBF, 0x24),
                error: Color32::from_rgb(0xF8, 0x71, 0x71),
                guide: Color32::from_rgb(0x2A, 0x30, 0x3A),
                hex_hi: mix(bg, accent_c, 0.35),
                link: [None; 6],
                idle_link: Color32::from_rgb(0x3A, 0x41, 0x4D),
                slow_lane: Color32::from_rgb(0xFB, 0xBF, 0x24),
            }
        } else {
            let bg = Color32::from_rgb(0xF4, 0xF5, 0xF8);
            Self {
                dark,
                bg,
                panel: Color32::from_rgb(0xFB, 0xFB, 0xFD),
                card: Color32::WHITE,
                card_hover: Color32::from_rgb(0xF0, 0xF2, 0xF7),
                border: Color32::from_rgb(0xE1, 0xE4, 0xEB),
                text: Color32::from_rgb(0x16, 0x1A, 0x22),
                text_muted: Color32::from_rgb(0x5B, 0x63, 0x73),
                text_faint: Color32::from_rgb(0x9A, 0xA1, 0xAE),
                accent: accent_c,
                accent_soft: mix(Color32::WHITE, accent_c, 0.12),
                ok: Color32::from_rgb(0x16, 0xA3, 0x4A),
                info: Color32::from_rgb(0x25, 0x63, 0xEB),
                warn: Color32::from_rgb(0xD9, 0x77, 0x06),
                error: Color32::from_rgb(0xDC, 0x26, 0x26),
                guide: Color32::from_rgb(0xE3, 0xE6, 0xEC),
                hex_hi: mix(Color32::WHITE, accent_c, 0.25),
                link: [None; 6],
                idle_link: Color32::from_rgb(0xC5, 0xCA, 0xD3),
                slow_lane: Color32::from_rgb(0xD9, 0x77, 0x06),
            }
        }
    }

    /// Applies user link-color overrides.
    pub fn with_links(mut self, lc: &LinkColors) -> Self {
        let c = |v: [u8; 3]| Color32::from_rgb(v[0], v[1], v[2]);
        for (i, v) in lc.speeds.iter().enumerate() {
            self.link[i] = v.map(c);
        }
        if let Some(v) = lc.idle {
            self.idle_link = c(v);
        }
        if let Some(v) = lc.slow_lane {
            self.slow_lane = c(v);
        }
        self
    }

    pub fn speed(&self, s: Speed) -> Color32 {
        if let Some(c) = speed_index(s).and_then(|i| self.link[i]) {
            return c;
        }
        let d = self.dark;
        match s {
            Speed::Unknown => self.text_faint,
            Speed::Low | Speed::Full => {
                if d {
                    Color32::from_rgb(0x9C, 0xA8, 0xBA)
                } else {
                    Color32::from_rgb(0x64, 0x74, 0x8B)
                }
            }
            Speed::High => {
                if d {
                    Color32::from_rgb(0xC4, 0xA1, 0xFF)
                } else {
                    Color32::from_rgb(0x7C, 0x3A, 0xED)
                }
            }
            Speed::Super => {
                if d {
                    Color32::from_rgb(0x38, 0xBD, 0xF8)
                } else {
                    Color32::from_rgb(0x02, 0x84, 0xC7)
                }
            }
            Speed::SuperPlus => {
                if d {
                    Color32::from_rgb(0x2D, 0xD4, 0xBF)
                } else {
                    Color32::from_rgb(0x0D, 0x94, 0x88)
                }
            }
            Speed::SuperPlus20 => {
                if d {
                    Color32::from_rgb(0xF4, 0x72, 0xB6)
                } else {
                    Color32::from_rgb(0xDB, 0x27, 0x77)
                }
            }
        }
    }

    pub fn health(&self, h: Health) -> Color32 {
        match h {
            Health::Empty => self.text_faint,
            Health::Ok => self.ok,
            Health::Info => self.info,
            Health::Warning => self.warn,
            Health::Error => self.error,
        }
    }

    pub fn soft(&self, c: Color32) -> Color32 {
        mix(self.card, c, if self.dark { 0.16 } else { 0.12 })
    }
}

pub const SEMIBOLD: &str = "semibold";
pub const ICON_FILL: &str = "icon-fill";

pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(SEMIBOLD.into()))
}

pub fn icon_fill(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(ICON_FILL.into()))
}

fn load_system_font(name: &str) -> Option<Arc<FontData>> {
    let dir = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into());
    let bytes = std::fs::read(format!(r"{dir}\Fonts\{name}")).ok()?;
    Some(Arc::new(FontData::from_owned(bytes)))
}

pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let default_prop = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let default_mono = fonts
        .families
        .get(&FontFamily::Monospace)
        .cloned()
        .unwrap_or_default();

    fonts.font_data.insert(
        "phosphor".into(),
        egui_phosphor::Variant::Regular.font_data().into(),
    );
    fonts.font_data.insert(
        "phosphor-fill".into(),
        egui_phosphor::Variant::Fill.font_data().into(),
    );

    let mut prop = Vec::new();
    let mut semi = Vec::new();
    if let Some(f) = load_system_font("segoeui.ttf") {
        fonts.font_data.insert("segoe".into(), f);
        prop.push("segoe".to_string());
    }
    if let Some(f) = load_system_font("seguisb.ttf") {
        fonts.font_data.insert("segoe-sb".into(), f);
        semi.push("segoe-sb".to_string());
    }
    prop.push("phosphor".into());
    prop.extend(default_prop.iter().cloned());
    semi.extend(prop.iter().cloned());

    let mut mono = Vec::new();
    if let Some(f) =
        load_system_font("CascadiaMono.ttf").or_else(|| load_system_font("consola.ttf"))
    {
        fonts.font_data.insert("mono".into(), f);
        mono.push("mono".to_string());
    }
    mono.extend(default_mono);
    mono.push("phosphor".into());

    let mut fill = vec!["phosphor-fill".to_string()];
    fill.extend(prop.iter().cloned());

    fonts.families.insert(FontFamily::Proportional, prop);
    fonts.families.insert(FontFamily::Monospace, mono);
    fonts
        .families
        .insert(FontFamily::Name(SEMIBOLD.into()), semi);
    fonts
        .families
        .insert(FontFamily::Name(ICON_FILL.into()), fill);
    ctx.set_fonts(fonts);
}

pub fn apply(ctx: &egui::Context, p: &Palette) {
    let mut visuals = if p.dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    visuals.panel_fill = p.panel;
    visuals.window_fill = p.card;
    visuals.extreme_bg_color = if p.dark {
        Color32::from_rgb(0x0B, 0x0D, 0x10)
    } else {
        Color32::from_rgb(0xF7, 0xF8, 0xFA)
    };
    visuals.faint_bg_color = p.card_hover;
    visuals.window_stroke = Stroke::new(1.0, p.border);
    visuals.window_corner_radius = CornerRadius::same(12);
    visuals.menu_corner_radius = CornerRadius::same(10);
    visuals.selection.bg_fill = with_alpha(p.accent, 0.35);
    visuals.selection.stroke = Stroke::new(1.0, p.accent);
    visuals.hyperlink_color = p.accent;
    visuals.warn_fg_color = p.warn;
    visuals.error_fg_color = p.error;
    visuals.override_text_color = Some(p.text);
    visuals.window_shadow = egui::Shadow {
        offset: [0, 8],
        blur: 28,
        spread: 0,
        color: Color32::from_black_alpha(if p.dark { 110 } else { 40 }),
    };
    visuals.popup_shadow = egui::Shadow {
        offset: [0, 6],
        blur: 18,
        spread: 0,
        color: Color32::from_black_alpha(if p.dark { 90 } else { 30 }),
    };

    let r = CornerRadius::same(7);
    let w = &mut visuals.widgets;
    w.noninteractive.bg_fill = p.card;
    w.noninteractive.weak_bg_fill = p.card;
    w.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
    w.noninteractive.fg_stroke = Stroke::new(1.0, p.text_muted);
    w.noninteractive.corner_radius = r;
    w.inactive.bg_fill = p.card;
    w.inactive.weak_bg_fill = p.card;
    w.inactive.bg_stroke = Stroke::new(1.0, p.border);
    w.inactive.fg_stroke = Stroke::new(1.0, p.text);
    w.inactive.corner_radius = r;
    w.hovered.bg_fill = p.card_hover;
    w.hovered.weak_bg_fill = p.card_hover;
    w.hovered.bg_stroke = Stroke::new(1.0, with_alpha(p.accent, 0.6));
    w.hovered.fg_stroke = Stroke::new(1.0, p.text);
    w.hovered.corner_radius = r;
    w.hovered.expansion = 0.0;
    w.active.bg_fill = p.accent_soft;
    w.active.weak_bg_fill = p.accent_soft;
    w.active.bg_stroke = Stroke::new(1.0, p.accent);
    w.active.fg_stroke = Stroke::new(1.0, p.text);
    w.active.corner_radius = r;
    w.open.bg_fill = p.card_hover;
    w.open.weak_bg_fill = p.card_hover;
    w.open.corner_radius = r;

    ctx.all_styles_mut(|style| {
        style.visuals = visuals.clone();
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(10.0, 5.0);
        style.spacing.interact_size.y = 26.0;
        style.spacing.menu_margin = egui::Margin::same(6);
        style.spacing.scroll.bar_width = 8.0;
        style.spacing.scroll.floating = true;
        style.text_styles = [
            (TextStyle::Small, FontId::proportional(11.5)),
            (TextStyle::Body, FontId::proportional(13.5)),
            (TextStyle::Button, FontId::proportional(13.5)),
            (TextStyle::Heading, semibold(19.0)),
            (TextStyle::Monospace, FontId::monospace(12.5)),
        ]
        .into();
        style.animation_time = 0.14;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_color_overrides_apply_and_reset() {
        let base = Palette::new(true, Accent::Indigo);
        let mut lc = LinkColors::default();
        lc.speeds[speed_index(Speed::High).unwrap()] = Some([1, 2, 3]);
        lc.slow_lane = Some([9, 9, 9]);
        let p = base.with_links(&lc);
        assert_eq!(p.speed(Speed::High), Color32::from_rgb(1, 2, 3));
        assert_eq!(
            p.speed(Speed::Super),
            base.speed(Speed::Super),
            "untouched speeds keep theme colors"
        );
        assert_eq!(p.slow_lane, Color32::from_rgb(9, 9, 9));
        assert_eq!(
            base.with_links(&LinkColors::default()).speed(Speed::High),
            base.speed(Speed::High)
        );
    }

    #[test]
    fn presets_are_complete_and_distinct() {
        for (name, lc) in LinkColors::presets().into_iter().skip(1) {
            let set: std::collections::HashSet<_> =
                lc.speeds.iter().map(|c| c.expect(name)).collect();
            assert_eq!(set.len(), 6, "{name} must give every speed its own color");
        }
    }
}
