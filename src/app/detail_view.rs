//! The right-hand detail pane.

use egui::{Align2, CornerRadius, FontId, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Ui, Vec2};
use egui_phosphor::regular as ph;

use super::theme::{self, with_alpha, Palette};
use super::{hex, icons, tree_view, widgets, Action, App, DetailTab};
use crate::details::{self, human_bytes, Section};
use crate::insights::Severity;
use crate::model::*;
use crate::tree::{self, FlatNode, Health, NodeKind, NodePath};

pub fn show(app: &mut App, ui: &mut Ui) {
    let p = app.p;
    let Some(n) = app.selected_node().cloned() else {
        empty_state(ui, &p, app.scanning || app.snapshot.is_none());
        return;
    };
    let Some(snap) = app.snapshot.clone() else { return };
    let sections = details::sections(&snap, &n.path);
    let has_desc = sections.iter().any(|s| s.desc.is_some());
    if !has_desc && app.detail_tab == DetailTab::Descriptors {
        app.detail_tab = DetailTab::Overview;
    }

    egui::ScrollArea::vertical().auto_shrink([false; 2]).id_salt(("detail", &n.id)).show(ui, |ui| {
        egui::Frame::new().inner_margin(egui::Margin { left: 22, right: 22, top: 18, bottom: 28 }).show(ui, |ui| {
            ui.set_max_width(ui.available_width());
            header(app, ui, &n, &snap);
            ui.add_space(14.0);

            let mut tab = app.detail_tab;
            let mut opts = vec![(DetailTab::Overview, "Overview")];
            if has_desc {
                opts.push((DetailTab::Descriptors, "Descriptors"));
            }
            opts.push((DetailTab::Report, "Text report"));
            let labels: Vec<(DetailTab, String)> = opts.iter().map(|(t, s)| (*t, s.to_string())).collect();
            let refs: Vec<(DetailTab, &str)> = labels.iter().map(|(t, s)| (*t, s.as_str())).collect();
            widgets::segmented(ui, &p, "detail-tabs", &mut tab, &refs);
            app.detail_tab = tab;
            ui.add_space(14.0);

            match app.detail_tab {
                DetailTab::Overview => overview(app, ui, &n, &snap, &sections),
                DetailTab::Descriptors => descriptors(app, ui, &sections),
                DetailTab::Report => report(app, ui, &n, &sections),
            }
        });
    });
}

fn empty_state(ui: &mut Ui, p: &Palette, scanning: bool) {
    ui.vertical_centered(|ui| {
        ui.add_space(ui.available_height() * 0.3);
        if scanning {
            ui.add(egui::Spinner::new().size(28.0).color(p.accent));
            ui.add_space(10.0);
            ui.label(RichText::new("Scanning USB bus…").font(theme::semibold(16.0)).color(p.text));
            ui.label(RichText::new("Reading host controllers, hubs and device descriptors").color(p.text_muted));
        } else {
            ui.label(RichText::new(ph::TREE_STRUCTURE).size(40.0).color(p.text_faint));
            ui.label(RichText::new("Select a device").color(p.text_muted));
        }
    });
}

fn kind_label(n: &FlatNode) -> &'static str {
    match n.kind {
        NodeKind::Computer => "Computer",
        NodeKind::Controller => "Host controller",
        NodeKind::RootHub => "Root hub",
        NodeKind::Hub => "Hub",
        NodeKind::Device => "USB device",
        NodeKind::EmptyPort => "Port",
        NodeKind::Child => "Windows device",
    }
}

fn header(app: &mut App, ui: &mut Ui, n: &FlatNode, snap: &Snapshot) {
    let p = app.p;
    let dev = app.device_at(&n.path).cloned();
    let port = app.port_at(&n.path).cloned();
    let color = match n.health {
        Health::Error => p.error,
        Health::Warning => p.warn,
        _ => match n.kind {
            NodeKind::Device | NodeKind::Hub => p.speed(n.speed),
            NodeKind::EmptyPort => p.text_muted,
            _ => p.accent,
        },
    };
    widgets::card(&p).inner_margin(egui::Margin::same(18)).show(ui, |ui| {
        ui.horizontal(|ui| {
            widgets::icon_tile(ui, &p, icons::for_node(n, dev.as_ref()), 60.0, color);
            ui.add_space(8.0);
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(kind_label(n).to_uppercase()).size(10.5).color(p.text_faint).strong());
                    if app.is_ghost(&n.id).is_some() {
                        widgets::pill(ui, &p, "DISCONNECTED", p.error);
                    }
                });
                let title = app.display_label(n);
                ui.add(egui::Label::new(RichText::new(&title).font(theme::semibold(22.0)).color(p.text)).wrap());
                // Subtitle
                let mut sub: Vec<String> = Vec::new();
                if app.nickname_for(n).is_some() {
                    sub.push(n.label.clone());
                }
                if let Some((v, pid)) = dev.as_ref().and_then(|d| d.vid_pid()) {
                    let db = crate::usbids::db();
                    if let Some(vn) = db.vendor(v) {
                        sub.push(vn.to_string());
                    }
                    sub.push(format!("{v:04X}:{pid:04X}"));
                }
                if let NodePath::Port(ci, ch) = &n.path {
                    sub.push(format!("Port {}", tree::port_chain(snap, *ci, ch)));
                    sub.push(snap.controllers[*ci].kind().to_string());
                }
                if n.kind == NodeKind::Controller || n.kind == NodeKind::Computer {
                    sub.push(n.detail.clone());
                }
                ui.label(RichText::new(sub.join("  ·  ")).color(p.text_muted));
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                let id = n.id.clone();
                ui.menu_button(RichText::new(ph::DOTS_THREE).size(18.0), |ui| tree_view::context_menu(app, ui, &id));
                if app.is_live() {
                    if let Some(d) = &dev {
                        if let Some(info) = &d.info {
                            let name = app.display_label(n);
                            if widgets::action_button(ui, &p, ph::ARROW_COUNTER_CLOCKWISE, "Restart", false)
                                .on_hover_text("Stop and restart the device driver (admin)")
                                .clicked()
                            {
                                app.dispatch(Action::Restart { id: info.instance_id.clone(), name: name.clone() });
                            }
                            if n.kind != NodeKind::Hub
                                && widgets::action_button(ui, &p, ph::EJECT, "Eject", false).on_hover_text("Safely remove hardware").clicked()
                            {
                                app.dispatch(Action::SafelyRemove { id: info.instance_id.clone(), name });
                            }
                        }
                    }
                }
            });
        });

        // Chips
        ui.add_space(10.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
            if let Some(d) = &dev {
                widgets::chip(ui, &p, ph::LIGHTNING, d.speed.label(), p.speed(d.speed));
                if let Some(dd) = d.descriptor() {
                    widgets::chip(ui, &p, ph::USB, &format!("USB {}", crate::descriptors::bcd(dd.bcd_usb as u64)), p.text_muted);
                }
                match (d.self_powered(), d.max_power_ma()) {
                    (Some(true), _) => {
                        widgets::chip(ui, &p, ph::PLUG, "Self-powered", p.text_muted);
                    }
                    (_, Some(ma)) => {
                        widgets::chip(ui, &p, ph::BATTERY_CHARGING, &format!("{ma} mA"), p.text_muted);
                    }
                    _ => {}
                }
                for c in d.interface_classes() {
                    widgets::chip(ui, &p, "", crate::descriptors::class_name(c), p.accent);
                }
                if let Some(info) = &d.info {
                    for v in info.all_volumes() {
                        for m in &v.mount_points {
                            let r = widgets::chip(ui, &p, ph::HARD_DRIVE, m.trim_end_matches('\\'), p.ok)
                                .interact(Sense::click())
                                .on_hover_text("Open in Explorer")
                                .on_hover_cursor(egui::CursorIcon::PointingHand);
                            if r.clicked() {
                                app.dispatch(Action::OpenPath(m.clone()));
                            }
                        }
                    }
                    for c in info.all_com_ports() {
                        widgets::chip(ui, &p, ph::TERMINAL_WINDOW, c, p.ok);
                    }
                }
            } else if let Some(pt) = &port {
                widgets::chip(ui, &p, ph::LIGHTNING, &format!("Up to {}", pt.max_speed().label()), p.speed(pt.max_speed()));
                if pt.connector.as_ref().is_some_and(|c| c.type_c) {
                    widgets::chip(ui, &p, ph::PLUG, "Type-C", p.text_muted);
                }
                if pt.connector.as_ref().is_some_and(|c| !c.user_connectable) {
                    widgets::chip(ui, &p, ph::CIRCUITRY, "Internal", p.text_muted);
                }
            }
            match n.health {
                Health::Error => {
                    widgets::chip(ui, &p, ph::X_CIRCLE, "Problem", p.error);
                }
                Health::Warning => {
                    widgets::chip(ui, &p, ph::WARNING, "Attention", p.warn);
                }
                Health::Ok if dev.is_some() => {
                    widgets::chip(ui, &p, ph::CHECK_CIRCLE, "Working", p.ok);
                }
                _ => {}
            }
            let companions = app.companions_of(&n.id);
            for c in companions {
                if let Some(cn) = app.flat.get(&c) {
                    let label = format!("Companion: {}", cn.port_label);
                    let r = widgets::chip(ui, &p, ph::LINK, &label, p.speed(cn.port_max_speed))
                        .interact(Sense::click())
                        .on_hover_text("The other half of this physical connector (USB 2 / USB 3). Click to jump.")
                        .on_hover_cursor(egui::CursorIcon::PointingHand);
                    if r.clicked() {
                        app.dispatch(Action::Select(c.clone()));
                    }
                }
            }
        });
    });
}

fn callouts(app: &App, ui: &mut Ui, n: &FlatNode) {
    let p = app.p;
    for i in app.insights.iter().filter(|i| i.node_id == n.id) {
        let (c, icon) = match i.severity {
            Severity::Error => (p.error, ph::X_CIRCLE),
            Severity::Warning => (p.warn, ph::WARNING),
            Severity::Info => (p.info, ph::LIGHTBULB),
        };
        egui::Frame::new()
            .fill(p.soft(c))
            .stroke(Stroke::new(1.0, with_alpha(c, 0.35)))
            .corner_radius(CornerRadius::same(10))
            .inner_margin(egui::Margin::symmetric(14, 10))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(RichText::new(icon).size(18.0).color(c));
                    ui.vertical(|ui| {
                        ui.label(RichText::new(&i.title).strong().color(p.text));
                        if !i.detail.is_empty() {
                            ui.add(egui::Label::new(RichText::new(&i.detail).color(p.text_muted)).wrap());
                        }
                    });
                });
            });
        ui.add_space(8.0);
    }
}

fn overview(app: &mut App, ui: &mut Ui, n: &FlatNode, snap: &Snapshot, sections: &[Section]) {
    let p = app.p;
    callouts(app, ui, n);

    match &n.path {
        NodePath::Computer => dashboard(app, ui, snap),
        NodePath::Port(..) => {
            let dev = app.device_at(&n.path).cloned();
            let port = app.port_at(&n.path).cloned();
            if let (Some(d), Some(pt)) = (&dev, &port) {
                link_card(ui, &p, d, pt);
                ui.add_space(12.0);
                if let Some(h) = &d.hub {
                    port_map(app, ui, n, h);
                    ui.add_space(12.0);
                }
            }
        }
        NodePath::RootHub(ci) => {
            if let Some(h) = snap.controllers[*ci].root_hub.clone() {
                port_map(app, ui, n, &h);
                ui.add_space(12.0);
            }
        }
        _ => {}
    }

    let cards: Vec<&Section> = sections.iter().filter(|s| s.desc.is_none() && !s.rows.is_empty()).collect();
    let two_col = ui.available_width() > 860.0 && cards.len() > 1;
    if two_col {
        // Masonry: put each card in the currently shorter column.
        let mut cols: [Vec<&Section>; 2] = [vec![], vec![]];
        let mut h = [0usize; 2];
        for c in cards {
            let weight = c.rows.iter().map(|r| 1 + r.value.lines().count()).sum::<usize>() + 3;
            let i = if h[0] <= h[1] { 0 } else { 1 };
            h[i] += weight;
            cols[i].push(c);
        }
        ui.columns(2, |uis| {
            for (ci, col) in cols.iter().enumerate() {
                for s in col {
                    section_card(app, &mut uis[ci], s);
                    uis[ci].add_space(12.0);
                }
            }
        });
    } else {
        for s in cards {
            section_card(app, ui, s);
            ui.add_space(12.0);
        }
    }
}

fn section_icon(id: &str) -> &'static str {
    match id {
        "summary" => ph::SPARKLE,
        "port" => ph::PLUG,
        "connection" => ph::LINK,
        "devinfo" => ph::WRENCH,
        "children" => ph::STACK,
        "strings" => ph::TAG,
        "hub" => ph::USB,
        "controller" => ph::CPU,
        "computer" => ph::DESKTOP_TOWER,
        "stats" => ph::CHART_BAR,
        "volumes" => ph::HARD_DRIVES,
        _ => ph::INFO,
    }
}

fn section_card(app: &mut App, ui: &mut Ui, s: &Section) {
    let p = app.p;
    widgets::card(&p).show(ui, |ui| {
        ui.set_width(ui.available_width());
        widgets::card_title(ui, &p, section_icon(s.id), &s.title);
        let key_w = s.rows.iter().map(|r| r.key.chars().count()).max().unwrap_or(10).min(24) as f32 * 6.8 + 8.0;
        egui::Grid::new(("grid", s.id, &s.title)).num_columns(2).spacing([14.0, 7.0]).min_col_width(key_w.min(170.0)).show(ui, |ui| {
            for r in &s.rows {
                ui.label(RichText::new(&r.key).color(p.text_muted));
                let (color, prefix) = match r.flag {
                    Some(Severity::Error) => (p.error, format!("{}  ", ph::X_CIRCLE)),
                    Some(Severity::Warning) => (p.warn, format!("{}  ", ph::WARNING)),
                    Some(Severity::Info) => (p.info, format!("{}  ", ph::LIGHTBULB)),
                    None => (p.text, String::new()),
                };
                let mono = (r.value.starts_with("0x") && !r.value.contains(" – ") && !r.value.contains(" (")) || r.key.contains("ID") || r.key.contains("Path") || r.key.contains("Symbolic") || r.key.contains("Key") || r.key.contains("GUID");
                let mut text = RichText::new(format!("{prefix}{}", r.value)).color(color);
                if mono {
                    text = text.font(FontId::monospace(12.0));
                }
                let resp = ui.add(egui::Label::new(text).wrap().sense(Sense::click()));
                let value = r.value.clone();
                resp.on_hover_text("Click to copy").on_hover_cursor(egui::CursorIcon::Copy).clicked().then(|| {
                    app.dispatch(Action::CopyText(value));
                });
                ui.end_row();
            }
        });
        // Volume usage bars
        if s.id == "summary" || s.id == "volumes" {
            let vols: Vec<Volume> = app
                .selected_node()
                .cloned()
                .and_then(|n| match &n.path {
                    NodePath::Port(..) => app.device_at(&n.path).and_then(|d| d.info.as_ref()).map(|i| i.all_volumes().into_iter().cloned().collect()),
                    NodePath::Child(ci, ch, cc) => app
                        .snapshot
                        .as_ref()
                        .and_then(|sn| tree::port(sn, *ci, ch))
                        .and_then(|p| p.device.as_ref())
                        .and_then(|d| d.info.as_ref())
                        .and_then(|i| tree::child(i, cc))
                        .map(|c| c.volumes.clone()),
                    _ => None,
                })
                .unwrap_or_default();
            for v in vols.iter().filter(|v| v.total_bytes > 0) {
                ui.add_space(8.0);
                let used = v.total_bytes - v.free_bytes.min(v.total_bytes);
                let frac = used as f32 / v.total_bytes as f32;
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{}  {}", ph::HARD_DRIVE, v.mount_points.first().map(|m| m.trim_end_matches('\\')).unwrap_or("Volume"))).strong());
                    ui.label(RichText::new(format!("{} free of {}", human_bytes(v.free_bytes), human_bytes(v.total_bytes))).color(p.text_muted).size(12.0));
                });
                let c = if frac > 0.9 { p.error } else if frac > 0.75 { p.warn } else { p.accent };
                widgets::usage_bar(ui, &p, frac, c, ui.available_width());
            }
        }
    });
}

fn link_card(ui: &mut Ui, p: &Palette, d: &Device, pt: &Port) {
    widgets::card(p).show(ui, |ui| {
        ui.set_width(ui.available_width());
        widgets::card_title(ui, p, ph::GAUGE, "Link");
        let dev_max = d.max_capable_speed();
        ui.horizontal_wrapped(|ui| {
            widgets::speed_ladder(ui, p, d.speed, dev_max, pt.max_speed());
            ui.add_space(12.0);
            ui.vertical(|ui| {
                ui.label(RichText::new(d.speed.label()).font(theme::semibold(15.0)).color(p.speed(d.speed)));
                let ratio = if dev_max.mbps() > 0.0 { d.speed.mbps() / dev_max.mbps() } else { 1.0 };
                let msg = if ratio >= 0.999 {
                    "Running at the device's full speed".to_string()
                } else {
                    format!("Running at {:.0}% of what the device supports", ratio * 100.0)
                };
                ui.label(RichText::new(msg).color(p.text_muted));
                ui.horizontal(|ui| {
                    widgets::legend_item(ui, p, p.speed(d.speed), false, "current");
                    widgets::legend_item(ui, p, p.warn, true, "unused capability");
                    let (r, _) = ui.allocate_exact_size(Vec2::new(10.0, 10.0), Sense::hover());
                    ui.painter().line_segment([Pos2::new(r.center().x, r.top()), Pos2::new(r.center().x, r.bottom())], Stroke::new(2.0, p.text_muted));
                    ui.label(RichText::new("port max").size(11.5).color(p.text_muted));
                });
            });
        });
    });
}

/// Visual socket map of a hub's ports.
fn port_map(app: &mut App, ui: &mut Ui, n: &FlatNode, h: &Hub) {
    let p = app.p;
    widgets::card(&p).show(ui, |ui| {
        ui.set_width(ui.available_width());
        let used = h.ports.iter().filter(|x| x.device.is_some()).count();
        ui.horizontal(|ui| {
            widgets::card_title(ui, &p, ph::PLUGS, "Ports");
            ui.label(RichText::new(format!("{used} of {} in use", h.ports.len())).color(p.text_muted));
        });
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);
            for (pi, port) in h.ports.iter().enumerate() {
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(78.0, 64.0), Sense::click());
                let max = port.max_speed();
                let (fill, stroke, icon, label) = match &port.device {
                    Some(d) => {
                        let c = if d.info.as_ref().is_some_and(|i| i.has_problem()) { p.error } else { p.speed(d.speed) };
                        (p.soft(c), Stroke::new(1.5, c), ph::PLUGS_CONNECTED, d.speed.short().to_string())
                    }
                    None if port.status.is_error() => (p.soft(p.error), Stroke::new(1.5, p.error), ph::WARNING, "ERR".into()),
                    None => (p.card, Stroke::new(1.0, p.border), ph::CIRCLE_DASHED, max.short().to_string()),
                };
                let hovered = resp.hovered();
                let fill = if hovered { p.card_hover } else { fill };
                ui.painter().rect_filled(rect, CornerRadius::same(10), fill);
                ui.painter().rect_stroke(rect, CornerRadius::same(10), stroke, StrokeKind::Inside);
                ui.painter().text(Pos2::new(rect.left() + 9.0, rect.top() + 12.0), Align2::LEFT_CENTER, format!("{}", port.index), theme::semibold(12.0), p.text_muted);
                if port.connector.as_ref().is_some_and(|c| c.type_c) {
                    ui.painter().text(Pos2::new(rect.right() - 9.0, rect.top() + 12.0), Align2::RIGHT_CENTER, "C", FontId::proportional(10.5), p.text_faint);
                }
                ui.painter().text(rect.center() + Vec2::new(0.0, 1.0), Align2::CENTER_CENTER, icon, FontId::proportional(18.0), stroke.color);
                ui.painter().text(Pos2::new(rect.center().x, rect.bottom() - 11.0), Align2::CENTER_CENTER, label, FontId::proportional(10.5), p.text_muted);
                let child_id = app
                    .flat
                    .get(&n.id)
                    .and_then(|fnode| fnode.children.iter().map(|&c| &app.flat.nodes[c]).find(|c| c.port_label == format!("Port {}", port.index)))
                    .map(|c| c.id.clone());
                let tip = match &port.device {
                    Some(d) => format!("Port {} – {}\n{}", port.index, d.name(), d.speed.label()),
                    None => format!("Port {} – {}\nMax {}", port.index, port.status.label(), max.label()),
                };
                let resp = resp.on_hover_text(tip).on_hover_cursor(egui::CursorIcon::PointingHand);
                if resp.clicked() {
                    if let Some(c) = child_id {
                        app.dispatch(Action::Select(c));
                    }
                }
                let _ = pi;
            }
        });
        // Power budget for bus-powered hubs
        let demand: u32 = h.ports.iter().filter_map(|x| x.device.as_ref()).filter(|d| d.self_powered() != Some(true)).filter_map(|d| d.max_power_ma()).sum();
        if demand > 0 {
            let budget: u32 = h.ports.iter().map(|x| crate::insights::port_budget_ma(h, x)).sum();
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{}  Bus power", ph::BATTERY_CHARGING)).strong());
                ui.label(RichText::new(format!("{demand} mA requested by bus-powered devices · {budget} mA available")).color(p.text_muted).size(12.0));
            });
            let frac = demand as f32 / budget.max(1) as f32;
            widgets::usage_bar(ui, &p, frac, if frac > 1.0 { p.error } else if frac > 0.8 { p.warn } else { p.ok }, ui.available_width());
        }
    });
}

fn dashboard(app: &mut App, ui: &mut Ui, snap: &Snapshot) {
    let p = app.p;
    let devices: Vec<&FlatNode> = app.flat.nodes.iter().filter(|n| matches!(n.kind, NodeKind::Device | NodeKind::Hub)).collect();
    let empty = app.flat.nodes.iter().filter(|n| n.kind == NodeKind::EmptyPort).count();
    let problems = app.insights.iter().filter(|i| i.severity >= Severity::Warning).count();
    let tiles = [
        (ph::CPU, "Controllers", snap.controllers.len().to_string(), p.accent),
        (ph::USB, "Devices", devices.iter().filter(|n| n.kind == NodeKind::Device).count().to_string(), p.info),
        (ph::TREE_STRUCTURE, "Hubs", devices.iter().filter(|n| n.kind == NodeKind::Hub).count().to_string(), p.speed(Speed::High)),
        (ph::CIRCLE_DASHED, "Free ports", empty.to_string(), p.text_muted),
        (ph::WARNING, "Problems", problems.to_string(), if problems > 0 { p.warn } else { p.ok }),
    ];
    let cols = if ui.available_width() > 760.0 { 5 } else { 3 };
    for chunk in tiles.chunks(cols) {
        ui.columns(cols, |uis| {
            for (i, (icon, label, value, c)) in chunk.iter().enumerate() {
                widgets::card(&p).show(&mut uis[i], |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        widgets::icon_tile(ui, &p, icon, 34.0, *c);
                        ui.vertical(|ui| {
                            ui.label(RichText::new(value.as_str()).font(theme::semibold(20.0)));
                            ui.label(RichText::new(*label).size(12.0).color(p.text_muted));
                        });
                    });
                });
            }
        });
        ui.add_space(10.0);
    }
    ui.add_space(12.0);
    // Speed distribution
    widgets::card(&p).show(ui, |ui| {
        ui.set_width(ui.available_width());
        widgets::card_title(ui, &p, ph::CHART_BAR, "Devices by link speed");
        let speeds = [Speed::Low, Speed::Full, Speed::High, Speed::Super, Speed::SuperPlus, Speed::SuperPlus20];
        let counts: Vec<(Speed, usize)> = speeds
            .iter()
            .map(|s| (*s, devices.iter().filter(|n| n.speed == *s && n.kind == NodeKind::Device).count()))
            .filter(|(_, c)| *c > 0)
            .collect();
        let total: usize = counts.iter().map(|c| c.1).sum();
        if total > 0 {
            let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 14.0), Sense::hover());
            let mut x = rect.left();
            for (i, (s, c)) in counts.iter().enumerate() {
                let w = rect.width() * *c as f32 / total as f32;
                let r = Rect::from_min_size(Pos2::new(x, rect.top()), Vec2::new((w - 2.0).max(2.0), rect.height()));
                let cr = if i == 0 || i == counts.len() - 1 { 5 } else { 1 };
                ui.painter().rect_filled(r, CornerRadius::same(cr), p.speed(*s));
                x += w;
            }
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                for (s, c) in &counts {
                    widgets::legend_item(ui, &p, p.speed(*s), false, &format!("{} · {c}", s.label()));
                    ui.add_space(8.0);
                }
            });
        }
    });
    ui.add_space(12.0);
    if !app.settings.pinned.is_empty() {
        widgets::card(&p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            widgets::card_title(ui, &p, ph::PUSH_PIN, "Pinned");
            for id in app.settings.pinned.clone() {
                match app.flat.get(&id).cloned() {
                    Some(n) => {
                        if ui.link(format!("{}  {}", icons::for_node(&n, app.device_at(&n.path)), app.display_label(&n))).clicked() {
                            app.dispatch(Action::Select(id));
                        }
                    }
                    None => {
                        ui.label(RichText::new(format!("{id} (not connected)")).color(p.text_faint));
                    }
                }
            }
        });
        ui.add_space(12.0);
    }
}

fn descriptors(app: &mut App, ui: &mut Ui, sections: &[Section]) {
    let p = app.p;
    let wide = ui.available_width() > 760.0;
    for s in sections.iter().filter(|s| s.desc.is_some()) {
        widgets::card(&p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                widgets::card_title(ui, &p, ph::BINARY, &s.title);
                if let Some(raw) = &s.raw {
                    ui.label(RichText::new(format!("{} bytes", raw.len())).color(p.text_faint).size(12.0));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if widgets::icon_button(ui, &p, ph::COPY, "Copy hex").clicked() {
                            app.dispatch(Action::CopyText(crate::descriptors::hex_bytes(raw)));
                        }
                    });
                }
            });
            let desc = s.desc.as_ref().unwrap();
            let raw = s.raw.clone().unwrap_or_default();
            if wide {
                ui.columns(2, |cols| {
                    hex::desc_tree(app, &mut cols[0], s.id, desc);
                    hex::hex_view(app, &mut cols[1], s.id, &raw, desc);
                });
            } else {
                hex::desc_tree(app, ui, s.id, desc);
                ui.add_space(8.0);
                hex::hex_view(app, ui, s.id, &raw, desc);
            }
        });
        ui.add_space(12.0);
    }
}

fn report(app: &mut App, ui: &mut Ui, n: &FlatNode, sections: &[Section]) {
    let p = app.p;
    let valid = app.report_cache.as_ref().is_some_and(|(id, _)| *id == n.id);
    if !valid {
        let t = details::to_text(&app.display_label(n), sections, app.settings.hexdumps_in_reports);
        app.report_cache = Some((n.id.clone(), t));
    }
    let text = app.report_cache.as_ref().map(|c| c.1.clone()).unwrap_or_default();
    ui.horizontal(|ui| {
        if widgets::action_button(ui, &p, ph::COPY, "Copy", false).clicked() {
            app.dispatch(Action::CopyText(text.clone()));
        }
        if widgets::action_button(ui, &p, ph::EXPORT, "Export full report…", false).clicked() {
            app.dispatch(Action::ExportText);
        }
        let mut h = app.settings.hexdumps_in_reports;
        if ui.checkbox(&mut h, "Hex dumps").changed() {
            app.settings.hexdumps_in_reports = h;
            app.report_cache = None;
            app.settings.save();
        }
    });
    ui.add_space(8.0);
    egui::Frame::new()
        .fill(if p.dark { p.bg } else { p.card })
        .stroke(Stroke::new(1.0, p.border))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(egui::Margin::same(14))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let mut s = text.as_str();
            ui.add(egui::TextEdit::multiline(&mut s).font(FontId::monospace(12.5)).frame(egui::Frame::NONE).desired_width(f32::INFINITY).code_editor());
        });
}
