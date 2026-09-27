//! Animated teaching diagrams.

use egui::{
    Align2, Color32, CornerRadius, FontId, Pos2, Rect, RichText, Sense, Shape, Stroke, StrokeKind,
    Ui, Vec2,
};

use super::{ease_out, App};
use crate::app::map::draw_link;
use crate::app::theme::{self, with_alpha, Palette};
use crate::app::widgets;
use crate::model::Speed;

fn node_box(ui: &Ui, p: &Palette, r: Rect, title: &str, color: Color32) {
    ui.painter()
        .rect_filled(r, CornerRadius::same(10), p.soft(color));
    ui.painter().rect_stroke(
        r,
        CornerRadius::same(10),
        Stroke::new(1.0, with_alpha(color, 0.5)),
        StrokeKind::Inside,
    );
    ui.painter().text(
        r.center(),
        Align2::CENTER_CENTER,
        title,
        theme::semibold(12.0),
        p.text,
    );
}

/// Host -> hubs -> devices, with a polling "token" that visits each device in turn.
pub fn tiered_star(app: &App, ui: &mut Ui) {
    let p = app.p;
    let t = ui.input(|i| i.time);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 250.0), Sense::hover());
    let w = rect.width();
    let host = Rect::from_center_size(
        Pos2::new(rect.left() + w * 0.1, rect.center().y),
        Vec2::new(110.0, 46.0),
    );
    let hub1 = Rect::from_center_size(
        Pos2::new(rect.left() + w * 0.38, rect.top() + 70.0),
        Vec2::new(96.0, 40.0),
    );
    let hub2 = Rect::from_center_size(
        Pos2::new(rect.left() + w * 0.38, rect.bottom() - 60.0),
        Vec2::new(96.0, 40.0),
    );
    let devs = [
        (
            Rect::from_center_size(
                Pos2::new(rect.left() + w * 0.7, rect.top() + 28.0),
                Vec2::new(110.0, 34.0),
            ),
            "Keyboard",
            Speed::Low,
        ),
        (
            Rect::from_center_size(
                Pos2::new(rect.left() + w * 0.7, rect.top() + 80.0),
                Vec2::new(110.0, 34.0),
            ),
            "Webcam",
            Speed::High,
        ),
        (
            Rect::from_center_size(
                Pos2::new(rect.left() + w * 0.7, rect.top() + 132.0),
                Vec2::new(110.0, 34.0),
            ),
            "Flash drive",
            Speed::Super,
        ),
        (
            Rect::from_center_size(
                Pos2::new(rect.left() + w * 0.7, rect.bottom() - 60.0),
                Vec2::new(110.0, 34.0),
            ),
            "Audio DAC",
            Speed::Full,
        ),
    ];
    let hubs_of = [hub1, hub1, hub1, hub2];
    draw_link(
        ui,
        &p,
        host.right_center(),
        hub1.left_center(),
        Some(Speed::Super),
        1.0,
        t,
        true,
        1.0,
        false,
    );
    draw_link(
        ui,
        &p,
        host.right_center(),
        hub2.left_center(),
        Some(Speed::High),
        1.0,
        t,
        true,
        1.0,
        false,
    );
    for ((r, _, s), h) in devs.iter().zip(hubs_of) {
        draw_link(
            ui,
            &p,
            h.right_center(),
            r.left_center(),
            Some(*s),
            1.0,
            t,
            true,
            1.0,
            false,
        );
    }
    node_box(ui, &p, host, "Host (PC)", p.accent);
    node_box(ui, &p, hub1, "Hub", p.speed(Speed::Super));
    node_box(ui, &p, hub2, "Hub", p.speed(Speed::High));
    // Polling: the host asks one device at a time.
    let turn = ((t * 0.8) as usize) % devs.len();
    for (i, (r, name, s)) in devs.iter().enumerate() {
        let active = i == turn;
        let c = p.speed(*s);
        node_box(ui, &p, *r, name, c);
        if active {
            let pulse = ((t * 0.8).fract()) as f32;
            ui.painter().rect_stroke(
                r.expand(2.0 + pulse * 8.0),
                CornerRadius::same(12),
                Stroke::new(2.0, with_alpha(c, 1.0 - pulse)),
                StrokeKind::Outside,
            );
            ui.painter().text(
                r.right_center() + Vec2::new(12.0, 0.0),
                Align2::LEFT_CENTER,
                "← \"anything for me?\"",
                FontId::proportional(11.5),
                p.text_muted,
            );
        }
    }
    ui.painter().text(
        Pos2::new(host.center().x, host.bottom() + 16.0),
        Align2::CENTER_TOP,
        "asks every device\nin turn (polling)",
        FontId::proportional(11.0),
        p.text_faint,
    );
}

/// Front-view outlines of the common connectors.
pub fn connectors(app: &App, ui: &mut Ui) {
    let p = app.p;
    let items: [(&str, &str, &str); 6] = [
        ("Type-A", "USB 1–3 · 4 pins (USB 2) / 9 pins (USB 3)", "a"),
        ("Type-B", "Printers, docks · 4 / 9 pins", "b"),
        ("Mini-B", "Old cameras, MP3 players · 5 pins", "mini"),
        ("Micro-B", "Older phones · 5 pins", "micro"),
        (
            "Micro-B SuperSpeed",
            "USB 3 hard drives · 10 pins",
            "micross",
        ),
        (
            "Type-C",
            "Reversible · 24 pins · all speeds up to 80 Gbit/s",
            "c",
        ),
    ];
    let cols = if ui.available_width() > 760.0 { 3 } else { 2 };
    for chunk in items.chunks(cols) {
        ui.columns(cols, |uis| {
            for (i, (name, desc, kind)) in chunk.iter().enumerate() {
                let ui = &mut uis[i];
                widgets::card(&p).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    let (r, _) = ui
                        .allocate_exact_size(Vec2::new(ui.available_width(), 70.0), Sense::hover());
                    draw_connector(ui, &p, r.center(), kind);
                    ui.label(RichText::new(*name).font(theme::semibold(13.5)));
                    ui.add(
                        egui::Label::new(RichText::new(*desc).size(12.0).color(p.text_muted))
                            .wrap(),
                    );
                });
                ui.add_space(8.0);
            }
        });
    }
}

fn draw_connector(ui: &Ui, p: &Palette, c: Pos2, kind: &str) {
    let pen = Stroke::new(2.0, p.text_muted);
    let pt = ui.painter();
    let blue = p.speed(Speed::Super);
    match kind {
        "a" => {
            let r = Rect::from_center_size(c, Vec2::new(90.0, 34.0));
            pt.rect_stroke(r, CornerRadius::same(3), pen, StrokeKind::Inside);
            let tongue = Rect::from_min_size(r.min + Vec2::new(8.0, 6.0), Vec2::new(74.0, 10.0));
            pt.rect_filled(tongue, CornerRadius::same(2), blue);
            pt.text(
                r.center_bottom() + Vec2::new(0.0, 10.0),
                Align2::CENTER_CENTER,
                "blue tongue = USB 3",
                FontId::proportional(10.0),
                p.text_faint,
            );
        }
        "b" => {
            let pts = vec![
                c + Vec2::new(-24.0, -26.0),
                c + Vec2::new(24.0, -26.0),
                c + Vec2::new(24.0, 26.0),
                c + Vec2::new(-24.0, 26.0),
                c + Vec2::new(-24.0, -14.0),
                c + Vec2::new(-24.0, -26.0),
            ];
            pt.add(Shape::closed_line(pts, pen));
            pt.line_segment(
                [c + Vec2::new(-24.0, -14.0), c + Vec2::new(-12.0, -26.0)],
                pen,
            );
            pt.rect_filled(
                Rect::from_center_size(c, Vec2::new(20.0, 22.0)),
                CornerRadius::same(2),
                p.text_faint,
            );
        }
        "mini" => {
            let pts = vec![
                c + Vec2::new(-30.0, -10.0),
                c + Vec2::new(30.0, -10.0),
                c + Vec2::new(30.0, 4.0),
                c + Vec2::new(22.0, 12.0),
                c + Vec2::new(-22.0, 12.0),
                c + Vec2::new(-30.0, 4.0),
            ];
            pt.add(Shape::closed_line(pts, pen));
        }
        "micro" => {
            let pts = vec![
                c + Vec2::new(-32.0, -7.0),
                c + Vec2::new(32.0, -7.0),
                c + Vec2::new(26.0, 7.0),
                c + Vec2::new(-26.0, 7.0),
            ];
            pt.add(Shape::closed_line(pts, pen));
        }
        "micross" => {
            let pts = vec![
                c + Vec2::new(-46.0, -7.0),
                c + Vec2::new(46.0, -7.0),
                c + Vec2::new(42.0, 7.0),
                c + Vec2::new(-42.0, 7.0),
            ];
            pt.add(Shape::closed_line(pts, pen));
            pt.line_segment([c + Vec2::new(8.0, -7.0), c + Vec2::new(8.0, 7.0)], pen);
            pt.rect_filled(
                Rect::from_min_max(c + Vec2::new(10.0, -5.0), c + Vec2::new(40.0, 5.0)),
                CornerRadius::same(2),
                with_alpha(blue, 0.6),
            );
        }
        _ => {
            let r = Rect::from_center_size(c, Vec2::new(84.0, 26.0));
            pt.rect_stroke(r, CornerRadius::same(13), pen, StrokeKind::Inside);
            pt.rect_filled(
                Rect::from_center_size(c, Vec2::new(56.0, 6.0)),
                CornerRadius::same(3),
                p.accent,
            );
            pt.text(
                r.center_bottom() + Vec2::new(0.0, 12.0),
                Align2::CENTER_CENTER,
                "flips either way",
                FontId::proportional(10.0),
                p.text_faint,
            );
        }
    }
}

/// Scenarios for the companion diagram: (label, device SS-capable, cable SS-capable, is USB 3 hub).
pub const SCENARIOS: [(&str, bool, bool, bool); 4] = [
    ("USB 3 device + USB 3 cable", true, true, false),
    ("USB 3 device + USB 2 cable", true, false, false),
    ("USB 2 device", false, true, false),
    ("USB 3 hub", true, true, true),
];

/// Interactive dual-bus diagram: controller -> two lanes -> one socket -> device.
pub fn companion(app: &mut App, ui: &mut Ui) {
    let p = app.p;
    let now = ui.input(|i| i.time);
    ui.horizontal_wrapped(|ui| {
        let mut sel = app.learn.scenario;
        let opts: Vec<(usize, &str)> = SCENARIOS
            .iter()
            .enumerate()
            .map(|(i, s)| (i, s.0))
            .collect();
        if widgets::segmented(ui, &p, "companion-scn", &mut sel, &opts) {
            app.learn.scenario = sel;
            app.learn.scenario_at = now;
        }
    });
    ui.add_space(8.0);
    let (_, dev_ss, cable_ss, is_hub) = SCENARIOS[app.learn.scenario];
    let appear = ease_out(((now - app.learn.scenario_at) / 0.6) as f32);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 270.0), Sense::hover());
    let pt = ui.painter();
    let w = rect.width();
    let usb2_c = p.speed(Speed::High);
    let ss_c = p.speed(Speed::Super);

    // Controller with two internal buses.
    let ctrl = Rect::from_min_size(rect.min + Vec2::new(0.0, 20.0), Vec2::new(w * 0.26, 210.0));
    pt.rect_filled(ctrl, CornerRadius::same(14), p.soft(p.accent));
    pt.rect_stroke(
        ctrl,
        CornerRadius::same(14),
        Stroke::new(1.0, with_alpha(p.accent, 0.5)),
        StrokeKind::Inside,
    );
    pt.text(
        ctrl.center_top() + Vec2::new(0.0, 18.0),
        Align2::CENTER_CENTER,
        "xHCI host controller",
        theme::semibold(12.5),
        p.text,
    );
    let bus2 = Rect::from_center_size(
        Pos2::new(ctrl.center().x, ctrl.top() + 78.0),
        Vec2::new(ctrl.width() - 30.0, 40.0),
    );
    let bus3 = Rect::from_center_size(
        Pos2::new(ctrl.center().x, ctrl.top() + 150.0),
        Vec2::new(ctrl.width() - 30.0, 40.0),
    );
    for (b, c, name, sub) in [
        (bus2, usb2_c, "USB 2 root port 3", "D+ / D−  ·  480M"),
        (bus3, ss_c, "SuperSpeed root port 9", "TX± / RX±  ·  5G+"),
    ] {
        pt.rect_filled(b, CornerRadius::same(8), p.card);
        pt.rect_stroke(
            b,
            CornerRadius::same(8),
            Stroke::new(1.0, with_alpha(c, 0.7)),
            StrokeKind::Inside,
        );
        pt.text(
            b.center() - Vec2::new(0.0, 7.0),
            Align2::CENTER_CENTER,
            name,
            theme::semibold(11.5),
            c,
        );
        pt.text(
            b.center() + Vec2::new(0.0, 9.0),
            Align2::CENTER_CENTER,
            sub,
            FontId::proportional(10.5),
            p.text_muted,
        );
    }
    pt.text(
        Pos2::new(ctrl.center().x, ctrl.bottom() + 14.0),
        Align2::CENTER_CENTER,
        "Windows lists these as two ports",
        FontId::proportional(11.0),
        p.text_faint,
    );

    // Socket with pin rows: 4 USB 2 pins (front), 5 SuperSpeed pins (back).
    let sock = Rect::from_center_size(
        Pos2::new(rect.left() + w * 0.52, rect.center().y),
        Vec2::new(130.0, 150.0),
    );
    pt.rect_filled(sock, CornerRadius::same(12), p.card);
    pt.rect_stroke(
        sock,
        CornerRadius::same(12),
        Stroke::new(1.0, p.border),
        StrokeKind::Inside,
    );
    pt.text(
        sock.center_top() + Vec2::new(0.0, 14.0),
        Align2::CENTER_CENTER,
        "one physical socket",
        FontId::proportional(11.0),
        p.text_muted,
    );
    let row2_y = sock.top() + 62.0;
    let row3_y = sock.top() + 110.0;
    let use2 = !(dev_ss && cable_ss) || is_hub;
    let use3 = dev_ss && cable_ss;
    for (k, lbl) in ["VBUS", "D−", "D+", "GND"].iter().enumerate() {
        let x = sock.left() + 22.0 + k as f32 * 28.0;
        let r = Rect::from_center_size(Pos2::new(x, row2_y), Vec2::new(16.0, 18.0));
        let on = use2 && (k == 1 || k == 2);
        pt.rect_filled(
            r,
            CornerRadius::same(3),
            if on { usb2_c } else { with_alpha(usb2_c, 0.3) },
        );
        pt.text(
            r.center_bottom() + Vec2::new(0.0, 8.0),
            Align2::CENTER_CENTER,
            *lbl,
            FontId::proportional(8.5),
            p.text_faint,
        );
    }
    for (k, lbl) in ["RX−", "RX+", "GND", "TX−", "TX+"].iter().enumerate() {
        let x = sock.left() + 17.0 + k as f32 * 24.0;
        let r = Rect::from_center_size(Pos2::new(x, row3_y), Vec2::new(14.0, 18.0));
        let on = use3 && k != 2;
        pt.rect_filled(
            r,
            CornerRadius::same(3),
            if on { ss_c } else { with_alpha(ss_c, 0.25) },
        );
        pt.text(
            r.center_bottom() + Vec2::new(0.0, 8.0),
            Align2::CENTER_CENTER,
            *lbl,
            FontId::proportional(8.5),
            p.text_faint,
        );
    }

    // Controller -> socket lanes.
    let lane2 = |spd: Option<Speed>| {
        draw_link(
            ui,
            &p,
            bus2.right_center(),
            Pos2::new(sock.left(), row2_y),
            spd,
            1.0,
            now,
            true,
            1.0,
            false,
        )
    };
    let lane3 = |spd: Option<Speed>| {
        draw_link(
            ui,
            &p,
            bus3.right_center(),
            Pos2::new(sock.left(), row3_y),
            spd,
            1.0,
            now,
            true,
            1.0,
            false,
        )
    };
    lane2(if use2 { Some(Speed::High) } else { None });
    lane3(if use3 { Some(Speed::Super) } else { None });

    // Cable + device.
    let dev = Rect::from_center_size(
        Pos2::new(rect.right() - w * 0.12, rect.center().y),
        Vec2::new(w * 0.2, 110.0),
    );
    let slide = (1.0 - appear) * 40.0;
    let dev = dev.translate(Vec2::new(slide, 0.0));
    let dev_c = if is_hub {
        p.accent
    } else if dev_ss {
        ss_c
    } else {
        usb2_c
    };
    pt.rect_filled(
        dev,
        CornerRadius::same(14),
        with_alpha(p.soft(dev_c), appear),
    );
    pt.rect_stroke(
        dev,
        CornerRadius::same(14),
        Stroke::new(1.0, with_alpha(dev_c, 0.6 * appear)),
        StrokeKind::Inside,
    );
    let dev_name = if is_hub {
        "USB 3 hub"
    } else if dev_ss {
        "USB 3 device"
    } else {
        "USB 2 device"
    };
    pt.text(
        dev.center() - Vec2::new(0.0, 12.0),
        Align2::CENTER_CENTER,
        dev_name,
        theme::semibold(13.0),
        with_alpha(p.text, appear),
    );
    let sub = if is_hub {
        "a USB 2 hub + a SuperSpeed hub"
    } else if dev_ss {
        "speaks both buses"
    } else {
        "D+/D− only"
    };
    pt.text(
        dev.center() + Vec2::new(0.0, 8.0),
        Align2::CENTER_CENTER,
        sub,
        FontId::proportional(11.0),
        with_alpha(p.text_muted, appear),
    );
    // Cable wires from socket to device.
    let cable_c = if cable_ss { p.text_muted } else { p.warn };
    pt.text(
        Pos2::new((sock.right() + dev.left()) / 2.0, sock.top() + 12.0),
        Align2::CENTER_CENTER,
        if cable_ss {
            "USB 3 cable (9 wires)"
        } else {
            "USB 2 cable (4 wires)"
        },
        FontId::proportional(11.0),
        cable_c,
    );
    if use2 {
        draw_link(
            ui,
            &p,
            Pos2::new(sock.right(), row2_y),
            Pos2::new(dev.left(), dev.center().y - 14.0),
            Some(Speed::High),
            1.0,
            now,
            true,
            1.0,
            !cable_ss && dev_ss,
        );
    }
    if use3 {
        draw_link(
            ui,
            &p,
            Pos2::new(sock.right(), row3_y),
            Pos2::new(dev.left(), dev.center().y + 14.0),
            Some(Speed::Super),
            1.0,
            now,
            true,
            1.0,
            false,
        );
    }
    if !cable_ss {
        // Missing SuperSpeed wires: a broken dashed line.
        for s in Shape::dashed_line(
            &[
                Pos2::new(sock.right(), row3_y),
                Pos2::new(dev.left(), dev.center().y + 14.0),
            ],
            Stroke::new(1.5, with_alpha(p.error, 0.6)),
            5.0,
            6.0,
        ) {
            pt.add(s);
        }
        pt.text(
            Pos2::new((sock.right() + dev.left()) / 2.0, row3_y + 14.0),
            Align2::CENTER_CENTER,
            "no SuperSpeed wires",
            FontId::proportional(10.5),
            p.error,
        );
    }
}

pub fn companion_caption(app: &App) -> (&'static str, &'static str) {
    match app.learn.scenario {
        0 => (
            "The device takes the SuperSpeed lane",
            "When the SuperSpeed receivers detect each other, the link trains on the TX/RX pairs and the device is enumerated on the SuperSpeed port (here: port 9). The USB 2 port of the same socket stays empty, so tools that list ports show one busy and one \"empty\" port for the same physical hole.",
        ),
        1 => (
            "Fallback: the device lands on the USB 2 lane",
            "A USB 2 cable only has four wires, so SuperSpeed training never succeeds. After a timeout the device connects over D+/D− and appears on the USB 2 port (port 3) at 480 Mbit/s. The socket and device are both fine – the cable is the bottleneck. The same happens with half-inserted plugs, cheap extensions and some adapters.",
        ),
        2 => (
            "USB 2 devices always use the USB 2 lane",
            "A mouse, keyboard or older drive has no SuperSpeed circuitry, so it appears on the USB 2 port. That's normal – nothing is being wasted, because the device can't go faster.",
        ),
        _ => (
            "A USB 3 hub uses both lanes at once",
            "Inside every USB 3 hub are two hubs: a USB 2 hub and a SuperSpeed hub. The USB 2 hub connects through the D+/D− lane, the SuperSpeed hub through the TX/RX lane – both through the same cable. That's why Windows shows a USB 3 hub twice (e.g. \"USB2.0 Hub\" and \"USB3.0 Hub\" with the same vendor). USB 2 devices plugged into it go through the USB 2 half, USB 3 devices through the SuperSpeed half.",
        ),
    }
}

/// Enumeration steps: (title, detail, direction: true = host->device).
pub const ENUM_STEPS: [(&str, &str, bool); 7] = [
    ("Attach", "The device pulls D+ (or D− for Low Speed) high through a 1.5 kΩ resistor. SuperSpeed devices announce themselves with receiver terminations and low-frequency pulses (LFPS). The hub notices and reports a port change.", false),
    ("Reset & speed", "The host resets the port for at least 10 ms. A Hi-Speed device answers with a \"chirp\" and the hub chirps back – now both know they can do 480 Mbit/s. SuperSpeed links train their receivers instead.", true),
    ("First descriptor", "At the default address 0, the host asks for the device descriptor. The first 8 bytes reveal bMaxPacketSize0 – how big control packets may be. If this fails you get Code 43 \"Device Descriptor Request Failed\".", true),
    ("Set address", "The host hands out a unique address (1–127 on each bus). From now on the device only answers to that address.", true),
    ("Read everything", "Full device descriptor, configuration descriptor with all interfaces and endpoints, string descriptors (names, serial) and the BOS descriptor.", false),
    ("Find a driver", "Windows builds Hardware IDs (USB\\VID_xxxx&PID_xxxx&REV_xxxx) and Compatible IDs (USB\\Class_08&SubClass_06…) and picks the best driver: usbstor for drives, HidUsb for input devices, usbccgp for composite devices …", false),
    ("Set configuration", "The host activates a configuration. Endpoints come alive, the device may now draw its full configured current, and it's ready to use.", true),
];

pub fn enumeration(app: &App, ui: &mut Ui) {
    let p = app.p;
    let now = ui.input(|i| i.time);
    let t = (now - app.learn.opened_at) as f32;
    let step_len = 2.2;
    let cur = ((t / step_len) as usize) % ENUM_STEPS.len();
    let within = (t % step_len) / step_len;
    // Host <-> device lane with a travelling packet.
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 90.0), Sense::hover());
    let host = Rect::from_center_size(
        Pos2::new(rect.left() + 70.0, rect.center().y),
        Vec2::new(120.0, 50.0),
    );
    let dev = Rect::from_center_size(
        Pos2::new(rect.right() - 70.0, rect.center().y),
        Vec2::new(120.0, 50.0),
    );
    node_box(ui, &p, host, "Host", p.accent);
    node_box(ui, &p, dev, "New device", p.speed(Speed::High));
    let a = host.right_center();
    let b = dev.left_center();
    ui.painter()
        .line_segment([a, b], Stroke::new(2.0, with_alpha(p.guide, 1.0)));
    let (_, _, to_dev) = ENUM_STEPS[cur];
    let k = ease_out(within * 1.4);
    let pos = if to_dev {
        a + (b - a) * k
    } else {
        b + (a - b) * k
    };
    let c = p.speed(Speed::High);
    ui.painter().circle_filled(pos, 12.0, with_alpha(c, 0.25));
    ui.painter().circle_filled(pos, 6.0, c);
    ui.painter().text(
        Pos2::new(rect.center().x, rect.top() + 10.0),
        Align2::CENTER_CENTER,
        format!(
            "Step {} of {} – {}",
            cur + 1,
            ENUM_STEPS.len(),
            ENUM_STEPS[cur].0
        ),
        theme::semibold(12.5),
        p.text,
    );
    ui.add_space(10.0);
    for (i, (title, detail, _)) in ENUM_STEPS.iter().enumerate() {
        let active = i == cur;
        let done = i < cur;
        let fill = if active { p.accent_soft } else { p.card };
        let frame = egui::Frame::new()
            .fill(fill)
            .stroke(Stroke::new(1.0, if active { p.accent } else { p.border }))
            .corner_radius(CornerRadius::same(10))
            .inner_margin(egui::Margin::symmetric(12, 8));
        let r = frame.show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let (nr, _) = ui.allocate_exact_size(Vec2::splat(26.0), Sense::hover());
                let nc = if active || done {
                    p.accent
                } else {
                    p.text_faint
                };
                ui.painter()
                    .circle_filled(nr.center(), 12.0, if active { nc } else { p.soft(nc) });
                ui.painter().text(
                    nr.center(),
                    Align2::CENTER_CENTER,
                    format!("{}", i + 1),
                    theme::semibold(12.0),
                    if active { Color32::WHITE } else { nc },
                );
                ui.vertical(|ui| {
                    ui.label(RichText::new(*title).strong());
                    ui.add(
                        egui::Label::new(RichText::new(*detail).size(12.5).color(p.text_muted))
                            .wrap(),
                    );
                });
            });
        });
        if active {
            // Progress along the bottom edge of the active step.
            let rr = r.response.rect;
            let bar = Rect::from_min_size(
                Pos2::new(rr.left() + 10.0, rr.bottom() - 3.0),
                Vec2::new((rr.width() - 20.0) * within, 2.0),
            );
            ui.painter()
                .rect_filled(bar, CornerRadius::same(1), p.accent);
        }
        ui.add_space(6.0);
    }
}

/// A 125 µs Hi-Speed microframe: periodic traffic reserved first, bulk fills the rest.
pub fn microframe(app: &App, ui: &mut Ui) {
    let p = app.p;
    let t = ui.input(|i| i.time) as f32;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 110.0), Sense::hover());
    let bar = Rect::from_min_size(
        rect.min + Vec2::new(0.0, 26.0),
        Vec2::new(rect.width(), 38.0),
    );
    let segs = [
        ("SOF", 0.02, p.text_faint),
        (
            "Isochronous (audio/video)",
            0.38,
            p.speed(Speed::SuperPlus20),
        ),
        ("Interrupt (HID)", 0.08, p.speed(Speed::High)),
        ("Control", 0.07, p.accent),
        (
            "Bulk (storage) – whatever is left",
            0.45,
            p.speed(Speed::Super),
        ),
    ];
    let mut x = bar.left();
    for (name, frac, c) in segs {
        let w = bar.width() * frac;
        let r = Rect::from_min_size(Pos2::new(x, bar.top()), Vec2::new(w - 2.0, bar.height()));
        ui.painter()
            .rect_filled(r, CornerRadius::same(5), with_alpha(c, 0.8));
        if w > 70.0 {
            ui.painter().text(
                r.center(),
                Align2::CENTER_CENTER,
                name,
                FontId::proportional(11.0),
                Color32::WHITE,
            );
        }
        x += w;
    }
    // 80 % periodic limit marker
    let lim = bar.left() + bar.width() * 0.8;
    ui.painter().line_segment(
        [
            Pos2::new(lim, bar.top() - 8.0),
            Pos2::new(lim, bar.bottom() + 8.0),
        ],
        Stroke::new(2.0, p.warn),
    );
    ui.painter().text(
        Pos2::new(lim, bar.top() - 14.0),
        Align2::CENTER_CENTER,
        "periodic may reserve up to 80 %",
        FontId::proportional(10.5),
        p.warn,
    );
    // Sweep line = bus time passing
    let sweep = bar.left() + ((t * 0.35).fract()) * bar.width();
    ui.painter().line_segment(
        [
            Pos2::new(sweep, bar.top() - 2.0),
            Pos2::new(sweep, bar.bottom() + 2.0),
        ],
        Stroke::new(2.0, Color32::WHITE),
    );
    ui.painter().text(
        Pos2::new(bar.left(), bar.bottom() + 16.0),
        Align2::LEFT_CENTER,
        "0 µs",
        FontId::proportional(10.5),
        p.text_faint,
    );
    ui.painter().text(
        Pos2::new(bar.right(), bar.bottom() + 16.0),
        Align2::RIGHT_CENTER,
        "125 µs (one microframe, 8 per millisecond)",
        FontId::proportional(10.5),
        p.text_faint,
    );
}

/// Animated 8b/10b vs 128b/132b efficiency comparison.
pub fn encoding(app: &App, ui: &mut Ui) {
    let p = app.p;
    let t = ui.input(|i| i.time) as f32;
    for (name, data, total, c) in [
        ("Gen 1 · 8b/10b", 8usize, 10usize, p.speed(Speed::Super)),
        ("Gen 2 · 128b/132b", 128, 132, p.speed(Speed::SuperPlus)),
    ] {
        ui.label(
            RichText::new(format!(
                "{name} – {:.0} % of the bits carry data",
                data as f32 / total as f32 * 100.0
            ))
            .strong(),
        );
        let (rect, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 26.0), Sense::hover());
        let cells = total.min(66);
        let shown_data = if total > 66 {
            cells * data / total
        } else {
            data
        };
        let cw = rect.width() / cells as f32;
        let offset = ((t * 6.0) as usize) % cells;
        for k in 0..cells {
            let r = Rect::from_min_size(
                Pos2::new(rect.left() + k as f32 * cw, rect.top()),
                Vec2::new(cw - 2.0, rect.height()),
            );
            let is_data = k < shown_data;
            let lit = k == offset;
            let col = if is_data { c } else { p.error };
            ui.painter().rect_filled(
                r,
                CornerRadius::same(3),
                with_alpha(
                    col,
                    if lit {
                        1.0
                    } else if is_data {
                        0.55
                    } else {
                        0.8
                    },
                ),
            );
        }
        ui.add_space(8.0);
    }
    ui.label(RichText::new("Colored = payload bits · red = encoding overhead (for 128b/132b the row is scaled down to fit).").size(11.5).color(p.text_faint));
}

/// Conventional socket insert colors: (name, insert color, meaning, note).
const PORT_COLORS: [(&str, [u8; 3], &str, &str); 7] = [
    (
        "White",
        [0xF2, 0xF2, 0xF2],
        "USB 1.x (Low / Full Speed)",
        "Older PCs and hubs; some USB 2 ports are white too.",
    ),
    (
        "Black",
        [0x22, 0x22, 0x22],
        "USB 2.0 Hi-Speed (480 Mbit/s)",
        "The most common USB 2 color.",
    ),
    (
        "Blue",
        [0x00, 0x5E, 0xB8],
        "USB 5Gbps (USB 3.0 / 3.2 Gen 1)",
        "Recommended by the USB 3.0 specification (Pantone 300C).",
    ),
    (
        "Teal",
        [0x00, 0xA3, 0xA6],
        "USB 10Gbps (USB 3.1 / 3.2 Gen 2)",
        "Used by several PC and motherboard makers.",
    ),
    (
        "Red",
        [0xD1, 0x1A, 0x2A],
        "10 Gbit/s – or always-on",
        "Some boards mark 10 Gbit/s red; others use red for charging while off.",
    ),
    (
        "Yellow / orange",
        [0xF5, 0xB8, 0x00],
        "Always-on / sleep-and-charge",
        "Supplies power while the PC sleeps or is off; speed varies.",
    ),
    (
        "Type-C",
        [0x00, 0x00, 0x00],
        "Any speed – read the logo",
        "No insert color: look for SS 5/10/20, 40/80, a Thunderbolt ⚡ or DP logo.",
    ),
];

/// Swatches drawn as Type-A sockets with a colored insert.
pub fn port_colors(app: &App, ui: &mut Ui) {
    let p = app.p;
    let t = ui.input(|i| i.time) as f32;
    let cols = if ui.available_width() > 760.0 { 4 } else { 2 };
    for (row, chunk) in PORT_COLORS.chunks(cols).enumerate() {
        ui.columns(cols, |uis| {
            for (i, (name, rgb, meaning, note)) in chunk.iter().enumerate() {
                // Columns justify wrapped text; switch to a plain left-aligned layout.
                uis[i].with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                    let (r, _) = ui
                        .allocate_exact_size(Vec2::new(ui.available_width(), 58.0), Sense::hover());
                    let k = row * cols + i;
                    // Gentle staggered pop-in on open.
                    let appear =
                        ease_out(((t - app.learn.opened_at as f32) - k as f32 * 0.07) / 0.5);
                    let c = r.center() + Vec2::new(0.0, (1.0 - appear) * 8.0);
                    let pt = ui.painter();
                    if *name == "Type-C" {
                        let body = Rect::from_center_size(c, Vec2::new(84.0, 26.0));
                        pt.rect_stroke(
                            body,
                            CornerRadius::same(13),
                            Stroke::new(2.0, with_alpha(p.text_muted, appear)),
                            StrokeKind::Inside,
                        );
                        pt.rect_filled(
                            Rect::from_center_size(c, Vec2::new(56.0, 6.0)),
                            CornerRadius::same(3),
                            with_alpha(p.text_faint, appear),
                        );
                        pt.text(
                            body.right_center() + Vec2::new(14.0, 0.0),
                            Align2::LEFT_CENTER,
                            "⚡ SS",
                            theme::semibold(11.0),
                            with_alpha(p.accent, appear),
                        );
                    } else {
                        let shell = Rect::from_center_size(c, Vec2::new(90.0, 34.0));
                        pt.rect_filled(
                            shell,
                            CornerRadius::same(3),
                            with_alpha(
                                if p.dark {
                                    Color32::from_rgb(0x3A, 0x3F, 0x48)
                                } else {
                                    Color32::from_rgb(0xC9, 0xCD, 0xD4)
                                },
                                appear,
                            ),
                        );
                        pt.rect_stroke(
                            shell,
                            CornerRadius::same(3),
                            Stroke::new(1.5, with_alpha(p.text_muted, appear)),
                            StrokeKind::Inside,
                        );
                        let insert = Rect::from_min_size(
                            shell.min + Vec2::new(8.0, 6.0),
                            Vec2::new(74.0, 11.0),
                        );
                        let ic = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
                        pt.rect_filled(insert, CornerRadius::same(2), with_alpha(ic, appear));
                        pt.rect_stroke(
                            insert,
                            CornerRadius::same(2),
                            Stroke::new(1.0, with_alpha(p.border, appear)),
                            StrokeKind::Inside,
                        );
                        // Contacts on the insert
                        for pin in 0..4 {
                            // Four 9 px contacts on a 17 px pitch, centered on the insert.
                            let span = 3.0 * 17.0 + 9.0;
                            let x =
                                insert.left() + (insert.width() - span) / 2.0 + pin as f32 * 17.0;
                            pt.rect_filled(
                                Rect::from_min_size(
                                    Pos2::new(x, insert.bottom()),
                                    Vec2::new(9.0, 3.0),
                                ),
                                CornerRadius::ZERO,
                                with_alpha(Color32::from_rgb(0xC8, 0xA2, 0x4A), appear),
                            );
                        }
                    }
                    ui.label(RichText::new(*name).font(theme::semibold(13.0)));
                    ui.label(RichText::new(*meaning).size(12.0).color(p.text));
                    ui.add(
                        egui::Label::new(RichText::new(*note).size(11.5).color(p.text_muted))
                            .wrap(),
                    );
                    ui.add_space(8.0);
                });
            }
        });
    }
    ui.add(
        egui::Label::new(
            RichText::new("Port colors are a convention, not a rule – manufacturers mix them freely. The negotiated speed shown by this app is what actually counts.")
                .size(12.0)
                .color(p.text_faint),
        )
        .wrap(),
    );
}

/// Simplified recreation of the USB "trident" port marking at `c` (height ~20·s).
pub fn trident(painter: &egui::Painter, c: Pos2, s: f32, color: Color32) {
    let w = 34.0 * s;
    let x0 = c.x - w / 2.0;
    let x1 = c.x + w / 2.0;
    let y = c.y;
    let st = Stroke::new(2.0 * s, color);
    painter.circle_filled(Pos2::new(x0 + 3.0 * s, y), 3.4 * s, color);
    painter.line_segment([Pos2::new(x0 + 3.0 * s, y), Pos2::new(x1 - 6.0 * s, y)], st);
    painter.add(Shape::convex_polygon(
        vec![
            Pos2::new(x1, y),
            Pos2::new(x1 - 8.0 * s, y - 4.5 * s),
            Pos2::new(x1 - 8.0 * s, y + 4.5 * s),
        ],
        color,
        Stroke::NONE,
    ));
    // Upper branch ending in a circle.
    let up = [
        Pos2::new(x0 + 0.34 * w, y),
        Pos2::new(x0 + 0.50 * w, y - 6.5 * s),
        Pos2::new(x0 + 0.66 * w, y - 6.5 * s),
    ];
    painter.add(Shape::line(up.to_vec(), st));
    painter.circle_filled(up[2], 2.6 * s, color);
    // Lower branch ending in a square.
    let dn = [
        Pos2::new(x0 + 0.20 * w, y),
        Pos2::new(x0 + 0.36 * w, y + 6.5 * s),
        Pos2::new(x0 + 0.52 * w, y + 6.5 * s),
    ];
    painter.add(Shape::line(dn.to_vec(), st));
    painter.rect_filled(
        Rect::from_center_size(dn[2], Vec2::splat(5.0 * s)),
        CornerRadius::ZERO,
        color,
    );
}

/// Port marking for a speed tier: trident, optional "SS" prefix and speed digits.
pub fn port_marking(ui: &mut Ui, short: &str, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(84.0, 30.0), Sense::hover());
    let pt = ui.painter();
    let (ss, digits) = match short {
        "5G" => (true, ""),
        "10G" => (true, "10"),
        "20G" => (true, "20"),
        "40G" => (false, "40"),
        "80G" => (false, "80"),
        _ => (false, ""),
    };
    let mut x = rect.left() + 2.0;
    if ss {
        let r = pt.text(
            Pos2::new(x, rect.center().y),
            Align2::LEFT_CENTER,
            "SS",
            theme::semibold(14.0),
            color,
        );
        x = r.right() + 2.0;
    }
    trident(pt, Pos2::new(x + 17.0, rect.center().y), 1.0, color);
    x += 36.0;
    if !digits.is_empty() {
        pt.text(
            Pos2::new(x, rect.center().y - 6.0),
            Align2::LEFT_CENTER,
            digits,
            theme::semibold(11.5),
            color,
        );
    }
}
