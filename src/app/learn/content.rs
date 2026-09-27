//! Chapter text and layout.

use egui::{RichText, Ui};
use egui_phosphor::regular as ph;

use super::{
    callout, card, diagrams, facts, fmt_rate, graphs, h2, hero, para, table, tier_color, tiers,
    App, Chapter,
};
use crate::app::{widgets, Action, ViewMode};
use crate::model::Speed;
use crate::physical::{self, Physical};
use crate::tree;

pub fn chapter(app: &mut App, ui: &mut Ui) {
    let p = app.p;
    let c = app.learn.chapter;
    hero(ui, &p, c);
    match c {
        Chapter::HowItWorks => how_it_works(app, ui),
        Chapter::Connectors => connectors(app, ui),
        Chapter::Power => power(app, ui),
        Chapter::Names => names(app, ui),
        Chapter::Speeds => speeds(app, ui),
        Chapter::Companion => companion(app, ui),
        Chapter::Enumeration => enumeration(app, ui),
        Chapter::Transfers => transfers(app, ui),
        Chapter::Signaling => signaling(app, ui),
        Chapter::LinkPower => link_power(app, ui),
        Chapter::TypeC => type_c(app, ui),
        Chapter::Troubleshooting => troubleshooting(app, ui),
        Chapter::YourPc => your_pc(app, ui),
    }
}

fn how_it_works(app: &mut App, ui: &mut Ui) {
    let p = app.p;
    para(ui, &p, "USB is a host-controlled bus. Your computer's host controller is always in charge: devices never talk on their own, they only answer when the host asks. Hubs fan the bus out into a tree – officially a \"tiered star\".");
    card(ui, &p, |ui| diagrams::tiered_star(app, ui));
    h2(ui, &p, "The key ideas");
    card(ui, &p, |ui| {
        facts(ui, &p, "hiw", &[
            ("Host controller", "The chip in your PC that runs the bus (xHCI on every modern PC; older machines had EHCI for USB 2 and OHCI/UHCI for USB 1). Each controller has a root hub with the physical ports."),
            ("Hub", "Repeats and routes traffic to more ports. Up to 5 hubs may sit between the host and a device (7 tiers counting the root hub and the device)."),
            ("Device", "Anything you plug in. One device can contain several functions (a webcam is often camera + microphone) – a \"composite device\"."),
            ("Address", "Every device gets a number from 1 to 127 when it is connected, so at most 127 devices per bus."),
            ("Endpoints", "Numbered mailboxes inside a device. Endpoint 0 is always there for control; others carry data IN (to the host) or OUT (to the device)."),
            ("Descriptors", "Every device describes itself: who made it, what it is, how much power it needs and which endpoints it has. This app shows them in the Descriptors tab."),
            ("Hot-plug", "Devices can be connected and removed at any time; the host detects the change and loads a driver automatically."),
        ]);
    });
    callout(ui, &p, ph::LIGHTBULB, p.info, "Why polling matters", "Because the host schedules everything, a slow or badly-behaved device can't hog the bus, and the host can guarantee time slots to audio or video streams. The downside: a USB 2 bus is shared – all devices behind one USB 2 hub split its 480 Mbit/s.");
}

fn connectors(app: &mut App, ui: &mut Ui) {
    let p = app.p;
    para(ui, &p, "The connector says nothing definite about speed – a Type-C port can be anything from USB 2 to 80 Gbit/s. Colors and logos give hints: blue (sometimes teal or red) Type-A tongues usually mean USB 3, an \"SS\" logo means SuperSpeed, and a lightning bolt means Thunderbolt.");
    diagrams::connectors(app, ui);
    h2(ui, &p, "Common port colors");
    card(ui, &p, |ui| diagrams::port_colors(app, ui));
    h2(ui, &p, "Pins at a glance");
    card(ui, &p, |ui| {
        table(
            ui,
            &p,
            "pins",
            &["Connector", "USB 2 pins", "Extra SuperSpeed pins", "Notes"],
            &[
                vec![
                    "Type-A / Type-B".into(),
                    "VBUS, D−, D+, GND".into(),
                    "SSTX±, SSRX±, GND_DRAIN (5)".into(),
                    "USB 3 plugs fit USB 2 sockets and vice versa".into(),
                ],
                vec![
                    "Micro-B".into(),
                    "VBUS, D−, D+, ID, GND".into(),
                    "Second section with 5 pins".into(),
                    "The wide \"double\" plug on USB 3 drives".into(),
                ],
                vec![
                    "Type-C".into(),
                    "D+/D− (twice, for flipping)".into(),
                    "2 TX + 2 RX pairs, CC1/2, SBU1/2".into(),
                    "24 pins; CC pins negotiate orientation and power".into(),
                ],
            ],
        );
    });
    callout(ui, &p, ph::INFO, p.info, "USB-C is a shape, not a speed", "A Type-C cable can be USB 2 only (common for charging cables), 5/10/20 Gbit/s, or USB4/Thunderbolt at 40–80 Gbit/s. Look for the speed printed on the cable or its packaging.");
}

fn power(app: &mut App, ui: &mut Ui) {
    let p = app.p;
    para(ui, &p, "Every USB port supplies 5 V. How much current a device may take depends on the port, the negotiation and – for high power – the cable.");
    card(ui, &p, |ui| {
        table(
            ui,
            &p,
            "power",
            &["Source", "Voltage", "Current", "Power"],
            &[
                vec![
                    "USB 2 (unconfigured)".into(),
                    "5 V".into(),
                    "100 mA".into(),
                    "0.5 W".into(),
                ],
                vec![
                    "USB 2 (configured)".into(),
                    "5 V".into(),
                    "500 mA".into(),
                    "2.5 W".into(),
                ],
                vec![
                    "USB 3.x (configured)".into(),
                    "5 V".into(),
                    "900 mA".into(),
                    "4.5 W".into(),
                ],
                vec![
                    "Battery Charging 1.2".into(),
                    "5 V".into(),
                    "1.5 A".into(),
                    "7.5 W".into(),
                ],
                vec![
                    "Type-C current".into(),
                    "5 V".into(),
                    "1.5 A / 3 A".into(),
                    "7.5 / 15 W".into(),
                ],
                vec![
                    "USB Power Delivery (SPR)".into(),
                    "5–20 V".into(),
                    "up to 5 A*".into(),
                    "up to 100 W".into(),
                ],
                vec![
                    "USB PD 3.1 EPR".into(),
                    "28 / 36 / 48 V".into(),
                    "up to 5 A*".into(),
                    "up to 240 W".into(),
                ],
            ],
        );
        ui.label(
            RichText::new("* more than 3 A requires an e-marked 5 A cable")
                .size(11.5)
                .color(p.text_faint),
        );
    });
    callout(ui, &p, ph::BATTERY_CHARGING, p.warn, "Bus-powered hubs", "A hub without its own power supply shares the upstream 500–900 mA among all its ports – often only 100 mA per port. Hard drives and phones may then fail to start or disconnect. This app flags devices that demand more current than their port provides.");
    para(ui, &p, "The \"MaxPower\" field in a device's configuration descriptor states what it needs (in 2 mA units for USB 2 and 8 mA units for SuperSpeed). You can see it as \"Demanded Current\" in the device summary.");
}

fn names(app: &mut App, ui: &mut Ui) {
    let p = app.p;
    para(ui, &p, "USB naming has been renamed twice after the fact, so the same 5 Gbit/s link has had three official names. Since 2022 the USB-IF recommends just stating the speed: \"USB 5Gbps\", \"USB 10Gbps\" and so on. Here is the decoder ring:");
    card(ui, &p, |ui| names_table(ui, &p));
    ui.label(
        RichText::new("Port markings are simplified recreations for identification; the USB logos are trademarks of the USB Implementers Forum.")
            .size(11.5)
            .color(p.text_faint),
    );
    ui.add_space(8.0);
    h2(ui, &p, "Common port colors");
    card(ui, &p, |ui| diagrams::port_colors(app, ui));
    h2(ui, &p, "How the renaming happened");
    card(ui, &p, |ui| {
        facts(ui, &p, "rename", &[
            ("2008 – USB 3.0", "Introduced SuperSpeed at 5 Gbit/s."),
            ("2013 – USB 3.1", "Renamed 5 Gbit/s to \"USB 3.1 Gen 1\" and added \"Gen 2\" at 10 Gbit/s (SuperSpeed+)."),
            ("2017 – USB 3.2", "Renamed again: Gen 1 → \"USB 3.2 Gen 1x1\", Gen 2 → \"Gen 2x1\", and added two-lane modes over Type-C: Gen 1x2 (10 Gbit/s, rare) and Gen 2x2 (20 Gbit/s)."),
            ("2019 – USB4", "Built on Thunderbolt 3: 20 or 40 Gbit/s, Type-C only, and it can tunnel USB 3, DisplayPort and PCIe through one link."),
            ("2022 – USB4 v2", "80 Gbit/s symmetric, or 120 Gbit/s one way for displays, using PAM3 signaling. Thunderbolt 5 is based on it."),
        ]);
    });
    callout(ui, &p, ph::LIGHTBULB, p.info, "Reading \"Gen X x Y\"", "The Gen number is the per-lane speed (Gen 1 = 5, Gen 2 = 10, Gen 3 = 20 Gbit/s per lane). The ×number is how many lanes are used together. So Gen 2x2 = 2 lanes × 10 Gbit/s = 20 Gbit/s. Two lanes need Type-C, which has two sets of high-speed pairs.");
    callout(ui, &p, ph::WARNING, p.warn, "What bcdUSB tells you (and doesn't)", "A device's descriptor reports a USB version such as 0x0320. That only means it follows the 3.2 specification – a \"USB 3.2\" device may still be 5 Gbit/s only. The negotiated speed shown in this app is what actually counts.");
}

fn speeds(app: &mut App, ui: &mut Ui) {
    let p = app.p;
    para(ui, &p, "Every generation at a glance. Hover a bar for all its names and real-world throughput. Switch to the linear scale to see how dramatic the jumps really are.");
    card(ui, &p, |ui| graphs::speed_graph(app, ui));
    h2(ui, &p, "The file-copy race");
    para(ui, &p, "The same file, copied at each generation's typical real-world speed. The fastest link finishes in two seconds of animation; everything else keeps its true proportion.");
    card(ui, &p, |ui| graphs::race(app, ui));
}

fn companion(app: &mut App, ui: &mut Ui) {
    let p = app.p;
    para(ui, &p, "USB 3 did not replace USB 2 – it was added next to it. Every USB 3 socket contains the four USB 2 wires plus five new SuperSpeed wires, and the controller runs two separate buses. That is why Windows (and tools like USBTreeView) list each blue socket twice: once as a USB 2 port and once as a SuperSpeed port. The two are called companion ports.");
    card(ui, &p, |ui| {
        diagrams::companion(app, ui);
        let (title, body) = diagrams::companion_caption(app);
        ui.add_space(4.0);
        ui.label(RichText::new(title).font(crate::app::theme::semibold(15.0)));
        ui.add(egui::Label::new(RichText::new(body).color(p.text_muted)).wrap());
    });
    h2(ui, &p, "Why it's confusing – and how this app helps");
    card(ui, &p, |ui| {
        facts(ui, &p, "comp", &[
            ("Two port numbers", "One socket might be \"port 3\" on the USB 2 side and \"port 9\" on the SuperSpeed side. Windows reads the pairing from the firmware (ACPI) and exposes it as the companion port number."),
            ("Empty-looking ports", "A connected USB 3 device makes its SuperSpeed port busy, while the USB 2 port of the same socket looks empty (and vice versa). Neither is broken."),
            ("\"Port max\" confusion", "A device on the USB 2 lane of a USB 3 socket reports a 480 Mbit/s port – but the socket itself could do 5 Gbit/s. This app always shows the socket maximum across both lanes."),
            ("Two hubs per USB 3 hub", "A USB 3 hub appears as a USB 2 hub plus a SuperSpeed hub. The Map view and the \"physical sockets\" tree mode merge them back together."),
            ("Type-C", "A Type-C socket also pairs a USB 2 port with one SuperSpeed port; the plug orientation decides which of its two SuperSpeed pairs is used."),
        ]);
    });
    ui.horizontal(|ui| {
        if widgets::action_button(ui, &p, ph::GRAPH, "See your sockets in the Map", false).clicked()
        {
            app.dispatch(Action::SetView(ViewMode::Map));
        }
    });
    ui.add_space(8.0);
    callout(ui, &p, ph::FIRST_AID, p.warn, "When a USB 3 device lands on the USB 2 lane", "Try a different cable (many cables are USB 2 only), plug in firmly, avoid extensions and front-panel ports with poor wiring, and check that the port really is USB 3. The link ladder and Map highlight these cases in amber.");
}

fn enumeration(app: &mut App, ui: &mut Ui) {
    let p = app.p;
    para(ui, &p, "Plugging a device in starts a fixed handshake called enumeration. It usually finishes in well under a second. The animation walks through each step.");
    card(ui, &p, |ui| diagrams::enumeration(app, ui));
    callout(ui, &p, ph::STETHOSCOPE, p.info, "Where enumeration fails", "\"Unknown USB Device (Device Descriptor Request Failed)\" with VID_0000&PID_0002 means step 3 failed – usually a bad cable, a marginal port, not enough power or faulty firmware. \"Port Reset Failed\" and \"Set Address Failed\" point at steps 2 and 4.");
}

fn transfers(app: &mut App, ui: &mut Ui) {
    let p = app.p;
    para(ui, &p, "USB offers four kinds of transfer, each suited to a different job. The host packs them into fixed time slices: 1 ms frames at Full Speed, 125 µs microframes at Hi-Speed.");
    card(ui, &p, |ui| {
        table(
            ui,
            &p,
            "xfer",
            &[
                "Type",
                "Guaranteed timing",
                "Retries on error",
                "Typical use",
                "Max packet FS / HS / SS",
            ],
            &[
                vec![
                    "Control".into(),
                    "Some bandwidth reserved".into(),
                    "Yes".into(),
                    "Setup, descriptors, commands (endpoint 0)".into(),
                    "64 / 64 / 512".into(),
                ],
                vec![
                    "Interrupt".into(),
                    "Polled at a fixed interval".into(),
                    "Yes".into(),
                    "Keyboards, mice, game controllers".into(),
                    "64 / 1024 / 1024".into(),
                ],
                vec![
                    "Isochronous".into(),
                    "Yes – fixed bandwidth".into(),
                    "No (late data is useless)".into(),
                    "Audio, webcams, streaming".into(),
                    "1023 / 1024 / 1024".into(),
                ],
                vec![
                    "Bulk".into(),
                    "No – uses leftover time".into(),
                    "Yes".into(),
                    "Storage, printers, network".into(),
                    "64 / 512 / 1024".into(),
                ],
            ],
        );
    });
    h2(ui, &p, "Inside one Hi-Speed microframe");
    card(ui, &p, |ui| diagrams::microframe(app, ui));
    callout(ui, &p, ph::LIGHTBULB, p.info, "Why a webcam can make your drive slower", "Isochronous and interrupt traffic is reserved first. On a busy USB 2 bus, a webcam and an audio interface can take most of each microframe, and a bulk-transfer drive gets only what remains. On SuperSpeed, each device has its own dedicated link, so this sharing matters much less.");
    para(ui, &p, "SuperSpeed adds bursts: a bulk endpoint may send up to 16 packets of 1024 bytes before waiting for acknowledgement (bMaxBurst in the SuperSpeed Endpoint Companion descriptor), and UAS drives use several streams in parallel.");
}

fn signaling(app: &mut App, ui: &mut Ui) {
    let p = app.p;
    para(ui, &p, "The headline number is the raw signaling rate on the wire. Encoding, packet headers, acknowledgements and the device itself all take their share, which is why a 5 Gbit/s link never moves 625 MB/s.");
    card(ui, &p, |ui| diagrams::encoding(app, ui));
    card(ui, &p, |ui| {
        table(
            ui,
            &p,
            "enc",
            &[
                "Generation",
                "Encoding",
                "Max after encoding",
                "Typical real throughput",
            ],
            &[
                vec![
                    "Hi-Speed 480M".into(),
                    "NRZI + bit stuffing".into(),
                    "~53 MB/s (bulk, theoretical)".into(),
                    "30–42 MB/s".into(),
                ],
                vec![
                    "5 Gbit/s".into(),
                    "8b/10b".into(),
                    "500 MB/s".into(),
                    "350–450 MB/s".into(),
                ],
                vec![
                    "10 Gbit/s".into(),
                    "128b/132b".into(),
                    "~1,200 MB/s".into(),
                    "900–1,050 MB/s".into(),
                ],
                vec![
                    "20 Gbit/s".into(),
                    "128b/132b, 2 lanes".into(),
                    "~2,400 MB/s".into(),
                    "1,800–2,000 MB/s".into(),
                ],
                vec![
                    "USB4 40 Gbit/s".into(),
                    "128b/132b, tunneled".into(),
                    "depends on the tunnel".into(),
                    "~3,000–3,800 MB/s (PCIe NVMe)".into(),
                ],
            ],
        );
    });
    facts(ui, &p, "sig", &[
        ("Differential pairs", "Every data signal travels on two wires with opposite polarity, which cancels noise. USB 2 has one half-duplex pair (D+/D−); SuperSpeed adds a transmit and a receive pair per lane, so it can send and receive at the same time."),
        ("8b/10b", "Every 8 data bits become 10 on the wire so the receiver can recover the clock – 20 % overhead."),
        ("128b/132b", "Gen 2 and later use 132-bit blocks carrying 128 data bits – only ~3 % overhead, one reason 10 Gbit/s is more than twice as fast in practice."),
        ("PAM3", "USB4 v2 sends three voltage levels instead of two, packing more bits into the same frequency."),
    ]);
}

fn link_power(app: &mut App, ui: &mut Ui) {
    let p = app.p;
    para(ui, &p, "Idle USB links drop into low-power states to save energy. Waking up takes time, which occasionally causes lag or disconnects with picky devices.");
    card(ui, &p, |ui| {
        table(
            ui,
            &p,
            "lp",
            &["State", "Applies to", "Meaning", "Exit latency"],
            &[
                vec![
                    "L0 / U0".into(),
                    "USB 2 / USB 3".into(),
                    "Active".into(),
                    "–".into(),
                ],
                vec![
                    "L1 (LPM)".into(),
                    "USB 2".into(),
                    "Light sleep between transfers".into(),
                    "microseconds".into(),
                ],
                vec![
                    "U1".into(),
                    "USB 3".into(),
                    "Standby, fast exit".into(),
                    "~µs (bU1DevExitLat)".into(),
                ],
                vec![
                    "U2".into(),
                    "USB 3".into(),
                    "Deeper standby".into(),
                    "tens–hundreds of µs (wU2DevExitLat)".into(),
                ],
                vec![
                    "L2 / U3".into(),
                    "USB 2 / USB 3".into(),
                    "Suspend – device keeps only 2.5 mA".into(),
                    "milliseconds".into(),
                ],
            ],
        );
    });
    callout(ui, &p, ph::WRENCH, p.info, "Selective suspend", "Windows suspends idle devices individually (\"USB selective suspend\"). If a device drops out after being idle, disabling selective suspend in the power plan or the device's Power Management tab often helps. The exit latencies a device supports are listed in its BOS descriptor.");
}

fn type_c(app: &mut App, ui: &mut Ui) {
    let p = app.p;
    facts(ui, &p, "tc", &[
        ("Type-C", "A reversible 24-pin connector. The CC pins detect orientation, who is host or device, and how much current is available."),
        ("Power Delivery", "A protocol on the CC wire that negotiates higher voltages (up to 20 V, or 48 V with EPR) for laptops and fast charging. It can also swap which side provides power."),
        ("e-marker", "A chip inside better cables that reports their current rating (3 A or 5 A) and speed. Required for 5 A / 100 W+ and for 40 Gbit/s and faster."),
        ("Alternate modes", "The high-speed pairs can be handed to other protocols: DisplayPort Alt Mode drives monitors, Thunderbolt Alt Mode carries Thunderbolt 3."),
        ("USB4 tunneling", "USB4 builds one fast link and runs USB 3, DisplayPort and PCIe traffic through it at the same time, sharing the bandwidth dynamically."),
        ("Billboard device", "If an alternate mode fails, the device may show up as a \"Billboard\" device explaining what went wrong – this app decodes its descriptor."),
    ]);
    ui.add_space(8.0);
    callout(ui, &p, ph::LIGHTNING, p.warn, "Thunderbolt vs USB4", "Thunderbolt 3 was donated to become USB4. Thunderbolt 4 is USB4 at 40 Gbit/s with stricter minimums (PCIe tunneling and two 4K displays required). Thunderbolt 5 matches USB4 v2 at 80 Gbit/s (120 Gbit/s boost). A USB4 port may or may not support Thunderbolt devices – check the lightning-bolt logo.");
}

fn troubleshooting(app: &mut App, ui: &mut Ui) {
    let p = app.p;
    card(ui, &p, |ui| {
        table(
            ui,
            &p,
            "ts",
            &["Symptom", "Likely cause", "What to try"],
            &[
                vec![
                    "Code 43, VID_0000&PID_0002".into(),
                    "Enumeration failed (bad cable, power, firmware)".into(),
                    "Other cable/port, powered hub, re-plug; update firmware".into(),
                ],
                vec![
                    "\"Device caused over-current\"".into(),
                    "Short circuit or device drawing too much".into(),
                    "Unplug, check the cable and device; the port recovers after the fault clears"
                        .into(),
                ],
                vec![
                    "USB 3 drive at 480 Mbit/s".into(),
                    "Came up on the USB 2 lane or behind a USB 2 hub".into(),
                    "USB 3 cable, direct port, no extension – see Companion ports".into(),
                ],
                vec![
                    "Random disconnects".into(),
                    "Selective suspend, weak power, EMI".into(),
                    "Disable selective suspend, use a powered hub, shorter cable".into(),
                ],
                vec![
                    "\"Not enough power\"".into(),
                    "Bus-powered hub or port budget exceeded".into(),
                    "Use a self-powered hub or a rear port".into(),
                ],
                vec![
                    "Hub nested too deeply".into(),
                    "More than 5 hubs in a row".into(),
                    "Connect closer to the computer".into(),
                ],
                vec![
                    "Code 28 / 10".into(),
                    "Driver missing or failed to start".into(),
                    "Install the vendor driver, check Device Manager".into(),
                ],
            ],
        );
    });
    callout(ui, &p, ph::LIGHTBULB, p.info, "Let the app help", "The Insights panel (Ctrl+J) and the Problems filter check for all of these automatically and explain the result for each device.");
}

fn your_pc(app: &mut App, ui: &mut Ui) {
    let p = app.p;
    let Some(snap) = app.snapshot.clone() else {
        para(ui, &p, "Waiting for the first scan…");
        return;
    };
    let phys = Physical::build(&snap);
    let mut sockets = 0;
    let mut usb3 = 0;
    let mut typec = 0;
    let mut free = 0;
    fn walk(c: &[physical::Connector], f: &mut dyn FnMut(&physical::Connector)) {
        for x in c {
            f(x);
            walk(&x.children, f);
        }
    }
    for pc in &phys.controllers {
        walk(&pc.connectors, &mut |c| {
            sockets += 1;
            if c.is_usb3() {
                usb3 += 1;
            }
            if c.type_c {
                typec += 1;
            }
            if c.attached.is_empty() {
                free += 1;
            }
        });
    }
    let mut fastest = Speed::Unknown;
    let mut slow: Vec<(String, String)> = Vec::new();
    for n in &app.flat.nodes {
        if let tree::NodePath::Port(ci, ch) = &n.path {
            if let Some(d) = tree::port(&snap, *ci, ch).and_then(|pp| pp.device.as_ref()) {
                if !d.is_hub {
                    fastest = fastest.max(d.speed);
                }
                if physical::explain_speed(&snap, &phys, *ci, ch).is_some() {
                    slow.push((n.id.clone(), app.display_label(n)));
                }
            }
        }
    }
    let cols = 4;
    let stats = [
        (ph::PLUGS, "Physical sockets", sockets.to_string(), p.accent),
        (
            ph::LIGHTNING,
            "USB 3 capable",
            usb3.to_string(),
            p.speed(Speed::Super),
        ),
        (
            ph::PLUG,
            "Type-C",
            typec.to_string(),
            p.speed(Speed::SuperPlus),
        ),
        (ph::CIRCLE_DASHED, "Free", free.to_string(), p.text_muted),
    ];
    ui.columns(cols, |uis| {
        for (i, (icon, label, value, c)) in stats.iter().enumerate() {
            widgets::card(&p).show(&mut uis[i], |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    widgets::icon_tile(ui, &p, icon, 34.0, *c);
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(value.as_str()).font(crate::app::theme::semibold(20.0)),
                        );
                        ui.label(RichText::new(*label).size(12.0).color(p.text_muted));
                    });
                });
            });
        }
    });
    ui.add_space(12.0);
    let fastest_tier = tiers().into_iter().find(|t| t.speed == Some(fastest));
    card(ui, &p, |ui| {
        ui.label(RichText::new("Your fastest active link").font(crate::app::theme::semibold(15.0)));
        match fastest_tier {
            Some(t) => {
                ui.label(
                    RichText::new(format!("{} – {}", t.title, fmt_rate(t.mbps)))
                        .size(18.0)
                        .color(tier_color(&p, &t)),
                );
                ui.label(RichText::new(t.names).color(p.text_muted));
            }
            None => {
                ui.label(RichText::new("No devices connected").color(p.text_muted));
            }
        }
        ui.add_space(4.0);
        ui.label(
            RichText::new(format!("Of your {sockets} physical sockets, {usb3} have a SuperSpeed lane – that's {} logical ports in Windows' eyes.", sockets + usb3))
                .size(12.5)
                .color(p.text_faint),
        );
    });
    if slow.is_empty() {
        callout(
            ui,
            &p,
            ph::CHECK_CIRCLE,
            p.ok,
            "No speed bottlenecks",
            "Every device runs as fast as its socket and path allow.",
        );
    } else {
        card(ui, &p, |ui| {
            ui.label(
                RichText::new(format!(
                    "{} {} could be faster",
                    ph::LIGHTNING,
                    if slow.len() == 1 {
                        "One device"
                    } else {
                        "These devices"
                    }
                ))
                .font(crate::app::theme::semibold(15.0))
                .color(p.slow_lane),
            );
            for (id, name) in slow {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&name).strong());
                    if ui.link("Show details").clicked() {
                        app.dispatch(Action::SetView(ViewMode::Tree));
                        app.dispatch(Action::Select(id.clone()));
                    }
                });
            }
        });
    }
}

/// Decoder table: one column per kind of name, with a drawn port marking.
fn names_table(ui: &mut Ui, p: &crate::app::theme::Palette) {
    use super::{column_widths, table_cell, table_row};
    let header = [
        "Port marking",
        "Marketing name",
        "Speed name",
        "Current spec name",
        "Former names",
        "Rate",
        "Connectors",
        "Typical port color",
    ];
    let fixed = 86.0;
    let widths = column_widths(
        ui.available_width(),
        &[1.0, 1.1, 1.5, 1.5, 0.8, 1.1, 1.3],
        fixed,
    );
    table_row(ui, p, None, |ui| {
        table_cell(
            ui,
            fixed,
            RichText::new(header[0])
                .size(12.0)
                .strong()
                .color(p.text_faint),
        );
        for (i, h) in header.iter().skip(1).enumerate() {
            table_cell(
                ui,
                widths[i],
                RichText::new(*h).size(12.0).strong().color(p.text_faint),
            );
        }
    });
    for (ri, t) in tiers().iter().enumerate() {
        let c = tier_color(p, t);
        table_row(ui, p, Some(ri), |ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(fixed, 0.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_width(fixed);
                    diagrams::port_marking(ui, t.short, c);
                },
            );
            table_cell(ui, widths[0], RichText::new(t.title).strong().color(p.text));
            table_cell(ui, widths[1], RichText::new(t.speed_name).color(c));
            table_cell(ui, widths[2], RichText::new(t.current_spec).color(p.text));
            table_cell(ui, widths[3], RichText::new(t.former).color(p.text_muted));
            table_cell(
                ui,
                widths[4],
                RichText::new(format!(
                    "{}
{}",
                    fmt_rate(t.mbps),
                    t.year
                ))
                .color(p.text_muted),
            );
            table_cell(
                ui,
                widths[5],
                RichText::new(t.connectors).color(p.text_muted),
            );
            table_cell(
                ui,
                widths[6],
                RichText::new(t.port_color).color(p.text_muted),
            );
        });
    }
}
