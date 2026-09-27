//! Builds the information sections shown for a selected node.  Shared by the
//! UI detail pane and the text / HTML report exporters.

use crate::descriptors::{self, lang_name, DescNode};
use crate::insights::{self, problem_text};
use crate::model::*;
use crate::tree::{self, NodePath};
use crate::usbids;

#[derive(Clone, Debug)]
pub struct Row {
    pub key: String,
    pub value: String,
    /// Highlight the row (warning / error).
    pub flag: Option<insights::Severity>,
}

#[derive(Clone, Debug)]
pub struct Section {
    pub id: &'static str,
    pub title: String,
    pub rows: Vec<Row>,
    pub desc: Option<DescNode>,
    pub raw: Option<Vec<u8>>,
}

impl Section {
    fn new(id: &'static str, title: impl Into<String>) -> Self {
        Self { id, title: title.into(), rows: vec![], desc: None, raw: None }
    }

    fn row(&mut self, k: &str, v: impl Into<String>) {
        let v = v.into();
        if !v.is_empty() {
            self.rows.push(Row { key: k.into(), value: v, flag: None });
        }
    }

    fn flag(&mut self, k: &str, v: impl Into<String>, s: insights::Severity) {
        self.rows.push(Row { key: k.into(), value: v.into(), flag: Some(s) });
    }
}

fn yes(b: bool) -> String {
    if b { "yes".into() } else { "no".into() }
}

pub fn human_bytes(b: u64) -> String {
    const U: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = b as f64;
    let mut i = 0;
    while v >= 1000.0 && i < U.len() - 1 {
        v /= 1000.0;
        i += 1;
    }
    if i == 0 { format!("{b} B") } else { format!("{v:.1} {}", U[i]) }
}

pub fn devinfo_section(title: &str, i: &DevInfo) -> Section {
    let mut s = Section::new("devinfo", title);
    s.row("Friendly Name", &i.friendly_name);
    s.row("Device Description", &i.description);
    s.row("Bus Reported Desc", &i.bus_reported_desc);
    s.row("Device ID", &i.instance_id);
    s.row("Hardware IDs", i.hardware_ids.join("\n"));
    s.row("Compatible IDs", i.compatible_ids.join("\n"));
    s.row("Manufacturer", &i.manufacturer);
    s.row("Class", &i.class);
    s.row("Class GUID", &i.class_guid);
    s.row("Service", &i.service);
    s.row("Enumerator", &i.enumerator);
    s.row("Driver Key", &i.driver_key);
    s.row("Driver", {
        let mut d = Vec::new();
        if !i.driver_provider.is_empty() { d.push(i.driver_provider.clone()) }
        if !i.driver_version.is_empty() { d.push(format!("v{}", i.driver_version)) }
        if !i.driver_date.is_empty() { d.push(i.driver_date.clone()) }
        d.join(" · ")
    });
    s.row("Driver INF", &i.driver_inf);
    s.row("Upper Filters", i.upper_filters.join(", "));
    s.row("Lower Filters", i.lower_filters.join(", "));
    s.row("Location Info", &i.location_info);
    s.row("Location Paths", i.location_paths.join("\n"));
    s.row("Container ID", &i.container_id);
    s.row("PDO Name", &i.pdo_name);
    s.row("Power State", &i.power_state);
    s.row("COM Port", &i.com_port);
    if i.status_flags != 0 {
        s.row("Status Flags", format!("0x{:08X}", i.status_flags));
    }
    if i.problem_code != 0 {
        let sev = if i.is_disabled() { insights::Severity::Warning } else { insights::Severity::Error };
        s.flag("Problem Code", format!("{} – {}", i.problem_code, problem_text(i.problem_code)), sev);
    } else {
        s.row("Problem Code", "0 (working properly)");
    }
    s
}

fn volume_rows(s: &mut Section, v: &Volume) {
    let m = v.mount_points.join(", ");
    let used = v.total_bytes.saturating_sub(v.free_bytes);
    let label = if v.label.is_empty() { "(no label)".to_string() } else { v.label.clone() };
    s.row(
        if m.is_empty() { "Volume" } else { &m },
        format!(
            "{label} · {} · {} of {} used ({} free)",
            v.file_system,
            human_bytes(used),
            human_bytes(v.total_bytes),
            human_bytes(v.free_bytes)
        ),
    );
}

pub fn sections(snap: &Snapshot, path: &NodePath) -> Vec<Section> {
    match path {
        NodePath::Computer => computer(snap),
        NodePath::Controller(ci) => controller(&snap.controllers[*ci]),
        NodePath::RootHub(ci) => match &snap.controllers[*ci].root_hub {
            Some(h) => hub_sections(h),
            None => vec![],
        },
        NodePath::Port(ci, ch) => match tree::port(snap, *ci, ch) {
            Some(p) => port_sections(snap, *ci, ch, p),
            None => vec![],
        },
        NodePath::Child(ci, ch, cc) => {
            let Some(info) = tree::port(snap, *ci, ch).and_then(|p| p.device.as_ref()).and_then(|d| d.info.as_ref()) else {
                return vec![];
            };
            match tree::child(info, cc) {
                Some(c) => {
                    let mut v = vec![devinfo_section("Device Information", c)];
                    if !c.volumes.is_empty() {
                        let mut s = Section::new("volumes", "Volumes");
                        for vol in &c.volumes {
                            volume_rows(&mut s, vol);
                        }
                        v.push(s);
                    }
                    v
                }
                None => vec![],
            }
        }
    }
}

fn computer(snap: &Snapshot) -> Vec<Section> {
    let mut s = Section::new("computer", "Computer");
    s.row("Computer Name", &snap.computer.name);
    s.row("Operating System", &snap.computer.os);
    s.row("User", &snap.computer.user);
    s.row("Administrator", yes(snap.computer.is_admin));
    s.row("Snapshot Taken", &snap.taken_at);
    s.row("Scan Duration", format!("{} ms", snap.scan_ms));
    let mut stats = Section::new("stats", "Statistics");
    let (mut hubs, mut devs, mut ports, mut empty) = (0, 0, 0, 0);
    fn walk(h: &Hub, hubs: &mut u32, devs: &mut u32, ports: &mut u32, empty: &mut u32) {
        *hubs += 1;
        for p in &h.ports {
            *ports += 1;
            match &p.device {
                Some(d) => match &d.hub {
                    Some(sub) => walk(sub, hubs, devs, ports, empty),
                    None => *devs += 1,
                },
                None => *empty += 1,
            }
        }
    }
    for c in &snap.controllers {
        if let Some(h) = &c.root_hub {
            walk(h, &mut hubs, &mut devs, &mut ports, &mut empty);
        }
    }
    stats.row("Host Controllers", snap.controllers.len().to_string());
    stats.row("Hubs (incl. root hubs)", hubs.to_string());
    stats.row("Devices", devs.to_string());
    stats.row("Ports", format!("{ports} ({empty} free)"));
    stats.row("usb.ids Version", &usbids::db().version);
    vec![s, stats]
}

fn controller(c: &Controller) -> Vec<Section> {
    let mut s = Section::new("controller", "Host Controller");
    s.row("Name", c.info.display_name());
    s.row("Type", c.kind());
    if let Some(v) = c.pci_vendor {
        let vn = pci_vendor_name(v);
        s.row("PCI Vendor", format!("0x{v:04X}{}", if vn.is_empty() { String::new() } else { format!(" ({vn})") }));
    }
    if let Some(d) = c.pci_device {
        s.row("PCI Device", format!("0x{d:04X}"));
    }
    if let Some(r) = c.pci_revision {
        s.row("PCI Revision", format!("0x{r:02X}"));
    }
    s.row("Controller Flavor", &c.flavor);
    if c.num_root_ports > 0 {
        s.row("Root Ports", c.num_root_ports.to_string());
    }
    if let Some(e) = &c.error {
        s.flag("Error", e, insights::Severity::Error);
    }
    vec![s, devinfo_section("Device Information", &c.info)]
}

fn pci_vendor_name(v: u16) -> &'static str {
    match v {
        0x8086 => "Intel",
        0x1022 => "AMD",
        0x1B21 => "ASMedia",
        0x1912 => "Renesas",
        0x1106 => "VIA",
        0x10DE => "NVIDIA",
        0x1B73 => "Fresco Logic",
        0x104C => "Texas Instruments",
        0x1033 => "NEC",
        0x5143 => "Qualcomm",
        _ => "",
    }
}

fn hub_sections(h: &Hub) -> Vec<Section> {
    let mut s = Section::new("hub", if h.is_root { "Root Hub" } else { "Hub Information" });
    s.row("Hub Type", &h.hub_type);
    s.row("Ports", h.num_ports.to_string());
    s.row("Power", if h.bus_powered { "Bus-powered (100 mA per port)" } else { "Self-powered" });
    s.row("High-Speed Capable", yes(h.high_speed_capable));
    s.row("Multi-TT", yes(h.multi_tt));
    s.row("Symbolic Name", &h.symbolic_name);
    if let Some(e) = &h.error {
        s.flag("Error", e, insights::Severity::Warning);
    }
    let used = h.ports.iter().filter(|p| p.device.is_some()).count();
    s.row("Ports in Use", format!("{used} of {}", h.ports.len()));
    let mut out = vec![s];
    if !h.descriptor.is_empty() {
        let mut d = Section::new("hubdesc", "Hub Descriptor");
        d.desc = Some(descriptors::decode_hub(&h.descriptor));
        d.raw = Some(h.descriptor.clone());
        out.push(d);
    }
    if let Some(i) = &h.info {
        out.push(devinfo_section("Device Information", i));
    }
    out
}

fn port_sections(snap: &Snapshot, ci: usize, ch: &[usize], p: &Port) -> Vec<Section> {
    let mut out = Vec::new();
    let mut ps = Section::new("port", "Port");
    ps.row("Port Chain", tree::port_chain(snap, ci, ch));
    ps.row("Port Number", p.index.to_string());
    if p.status.is_error() {
        ps.flag("Connection Status", p.status.label(), insights::Severity::Error);
    } else {
        ps.row("Connection Status", p.status.label());
    }
    let mut protos = Vec::new();
    if p.supports_usb110 { protos.push("USB 1.1") }
    if p.supports_usb200 { protos.push("USB 2.0") }
    if p.supports_usb300 { protos.push("USB 3.x") }
    ps.row("Supported Protocols", protos.join(", "));
    ps.row("Max Port Speed", p.max_speed().label());
    if let Some(c) = &p.connector {
        ps.row("User Connectable", yes(c.user_connectable));
        ps.row("Type-C Connector", yes(c.type_c));
        ps.row("Debug Capable", yes(c.debug_capable));
        if c.companion_port != 0 {
            let hub = hub_display_name(snap, &c.companion_hub);
            ps.row(
                "Companion Port",
                format!("Port {}{}", c.companion_port, hub.map(|h| format!(" on {h}")).unwrap_or_default()),
            );
        }
        if c.multi_companions {
            ps.row("Multiple Companions", "yes");
        }
    }

    let Some(d) = &p.device else {
        out.push(ps);
        return out;
    };

    // Summary first — the most useful facts at a glance.
    let mut sum = Section::new("summary", "Summary");
    let dd = d.descriptor();
    if let Some(dd) = dd {
        let db = usbids::db();
        sum.row("Vendor", format!("0x{:04X}{}", dd.vid, db.vendor(dd.vid).map(|n| format!(" – {n}")).unwrap_or_default()));
        sum.row("Product", format!("0x{:04X}{}", dd.pid, db.product(dd.vid, dd.pid).map(|n| format!(" – {n}")).unwrap_or_default()));
        sum.row("Manufacturer String", d.string(dd.i_manufacturer).unwrap_or(""));
        sum.row("Product String", d.string(dd.i_product).unwrap_or(""));
        sum.row("Serial Number", d.string(dd.i_serial).unwrap_or(""));
        sum.row("USB Version", usb_version_label(dd.bcd_usb));
    }
    sum.row("Port Maximum Speed", p.max_speed().label());
    if let Some(dd) = dd {
        let _ = dd;
        sum.row("Device Maximum Speed", d.max_capable_speed().label());
    }
    match insights::speed_mismatch(p, d) {
        Some(m) => sum.flag("Connection Speed", format!("{}\n{m}", d.speed.label()), insights::Severity::Info),
        None => sum.row("Connection Speed", d.speed.label()),
    }
    if let Some(sp) = d.self_powered() {
        sum.row("Self Powered", yes(sp));
    }
    if let Some(ma) = d.max_power_ma() {
        sum.row("Demanded Current", format!("{ma} mA"));
    }
    let classes = d.interface_classes();
    if !classes.is_empty() {
        sum.row(
            "Functions",
            classes
                .iter()
                .map(|c| usbids::db().class(*c, None, None).unwrap_or(descriptors::class_name(*c)))
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
    if let Some(i) = &d.info {
        if i.problem_code != 0 {
            sum.flag("Problem", format!("{} – {}", i.problem_code, problem_text(i.problem_code)), insights::Severity::Error);
        }
        let vols = i.all_volumes();
        for v in vols {
            volume_rows(&mut sum, v);
        }
        let coms = i.all_com_ports();
        if !coms.is_empty() {
            sum.row("COM Ports", coms.join(", "));
        }
    }
    out.push(sum);
    out.push(ps);

    let mut conn = Section::new("connection", "Connection Information");
    conn.row("Device Address", d.address.to_string());
    conn.row("Current Config Value", d.current_config.to_string());
    conn.row("Open Pipes", d.open_pipes.to_string());
    conn.row("Is Hub", yes(d.is_hub));
    conn.row("Operating at SuperSpeed or higher", yes(d.operating_at_ss_or_higher));
    conn.row("SuperSpeed Capable", yes(d.ss_capable));
    conn.row("Operating at SuperSpeedPlus", yes(d.operating_at_ssp));
    conn.row("SuperSpeedPlus Capable", yes(d.ssp_capable));
    for pi in &d.pipes {
        conn.row(
            &format!("Pipe 0x{:02X}", pi.endpoint_address),
            format!(
                "{} {} · {} bytes · interval {}",
                if pi.endpoint_address & 0x80 != 0 { "IN" } else { "OUT" },
                descriptors::transfer_type(pi.attributes),
                pi.max_packet,
                pi.interval
            ),
        );
    }
    out.push(conn);

    if let Some(i) = &d.info {
        out.push(devinfo_section("Device Information", i));
        if !i.children.is_empty() {
            let mut cs = Section::new("children", "Child Devices");
            fn add(cs: &mut Section, c: &DevInfo, depth: usize) {
                let mut v = format!("{} [{}]", c.instance_id, c.class);
                if c.problem_code != 0 {
                    v += &format!(" – problem {}", c.problem_code);
                }
                cs.row(&format!("{}{}", "  ".repeat(depth), c.display_name()), v);
                for g in &c.children {
                    add(cs, g, depth + 1);
                }
            }
            for c in &i.children {
                add(&mut cs, c, 0);
            }
            out.push(cs);
        }
    }

    let lookup = |idx: u8| d.string(idx).map(str::to_string);
    if !d.device_descriptor.is_empty() {
        let mut s = Section::new("devdesc", "Device Descriptor");
        s.desc = Some(descriptors::decode_device(&d.device_descriptor, &lookup));
        s.raw = Some(d.device_descriptor.clone());
        out.push(s);
    }
    if !d.config_descriptor.is_empty() {
        let mut s = Section::new("config", "Configuration Descriptor");
        s.desc = Some(descriptors::decode_configuration(&d.config_descriptor, &lookup, d.speed.is_super()));
        s.raw = Some(d.config_descriptor.clone());
        out.push(s);
    }
    if !d.bos_descriptor.is_empty() {
        let mut s = Section::new("bos", "BOS Descriptor");
        s.desc = Some(descriptors::decode_bos(&d.bos_descriptor));
        s.raw = Some(d.bos_descriptor.clone());
        out.push(s);
    }
    if !d.device_qualifier.is_empty() {
        let mut s = Section::new("qualifier", "Device Qualifier");
        s.desc = Some(descriptors::decode_device_qualifier(&d.device_qualifier));
        s.raw = Some(d.device_qualifier.clone());
        out.push(s);
    }
    if let Some(h) = &d.hub {
        out.extend(hub_sections(h).into_iter().filter(|s| s.id != "devinfo"));
    }
    if !d.strings.is_empty() || !d.lang_ids.is_empty() {
        let mut s = Section::new("strings", "String Descriptors");
        if !d.lang_ids.is_empty() {
            s.row(
                "Language IDs",
                d.lang_ids
                    .iter()
                    .map(|l| format!("0x{l:04X} ({})", usbids::db().language(*l).unwrap_or(lang_name(*l))))
                    .collect::<Vec<_>>()
                    .join(", "),
            );
        }
        for st in &d.strings {
            s.row(&format!("String {} (0x{:04X})", st.index, st.lang), format!("\"{}\"", st.text));
        }
        out.push(s);
    }
    out
}

/// Friendly name for a hub given its symbolic link name.
pub fn hub_display_name(snap: &Snapshot, sym: &str) -> Option<String> {
    if sym.is_empty() {
        return None;
    }
    fn walk(h: &Hub, sym: &str, name: &str) -> Option<String> {
        if h.symbolic_name.eq_ignore_ascii_case(sym) {
            return Some(name.to_string());
        }
        for p in &h.ports {
            if let Some(d) = &p.device {
                if let Some(sub) = &d.hub {
                    if let Some(n) = walk(sub, sym, &format!("{} (port {})", d.name(), p.index)) {
                        return Some(n);
                    }
                }
            }
        }
        None
    }
    for (i, c) in snap.controllers.iter().enumerate() {
        if let Some(h) = &c.root_hub {
            if let Some(n) = walk(h, sym, &format!("Root Hub of controller {}", i + 1)) {
                return Some(n);
            }
        }
    }
    Some(sym.to_string())
}

pub fn usb_version_label(bcd: u16) -> String {
    let v = descriptors::bcd(bcd as u64);
    let note = match bcd {
        0x0100 => "USB 1.0",
        0x0110 => "USB 1.1",
        0x0200 => "USB 2.0",
        0x0201 => "USB 2.0 + LPM",
        0x0210 => "USB 2.1 (with BOS)",
        0x0300 => "USB 3.0 – 5 Gbit/s",
        0x0310 => "USB 3.1 – up to 10 Gbit/s",
        0x0320 => "USB 3.2 – up to 20 Gbit/s",
        _ => "",
    };
    if note.is_empty() { format!("0x{bcd:04X} ({v})") } else { format!("0x{bcd:04X} ({note})") }
}

/// Renders sections as a plain-text report block.
pub fn to_text(title: &str, secs: &[Section], hexdumps: bool) -> String {
    let mut out = String::new();
    out += &format!("{}\n{}\n", title, "=".repeat(title.chars().count().max(8)));
    for s in secs {
        out += &format!("\n---------------- {} ----------------\n", s.title);
        let w = s.rows.iter().map(|r| r.key.chars().count()).max().unwrap_or(0).max(24);
        for r in &s.rows {
            let mut lines = r.value.lines();
            let mark = match r.flag {
                Some(insights::Severity::Error) => " *!* ERROR",
                Some(insights::Severity::Warning) => " *!* WARNING",
                _ => "",
            };
            out += &format!("{:<w$} : {}{}\n", r.key, lines.next().unwrap_or(""), mark);
            for l in lines {
                out += &format!("{:<w$}   {}\n", "", l);
            }
        }
        if let Some(d) = &s.desc {
            desc_text(&mut out, d, 0);
        }
        if hexdumps {
            if let Some(raw) = &s.raw {
                out += &hexdump(raw);
            }
        }
    }
    out
}

fn desc_text(out: &mut String, d: &DescNode, depth: usize) {
    let ind = "  ".repeat(depth);
    if depth > 0 {
        *out += &format!("\n{ind}[{}]\n", d.title);
    }
    for f in &d.fields {
        *out += &format!("{ind}{:<26} : {}\n", f.name, f.display);
    }
    for w in &d.warnings {
        *out += &format!("{ind}*!* WARNING: {w}\n");
    }
    for c in &d.children {
        desc_text(out, c, depth + 1);
    }
}

pub fn hexdump(b: &[u8]) -> String {
    let mut out = String::new();
    for (i, chunk) in b.chunks(16).enumerate() {
        let hex: Vec<String> = chunk.iter().map(|x| format!("{x:02X}")).collect();
        let ascii: String = chunk.iter().map(|&c| if (0x20..0x7F).contains(&c) { c as char } else { '.' }).collect();
        out += &format!("{:04X}  {:<48} {}\n", i * 16, hex.join(" "), ascii);
    }
    out
}

/// Full text report of a snapshot: tree overview followed by every node.
pub fn full_report(snap: &Snapshot, hexdumps: bool) -> String {
    let flat = tree::flatten(snap, &tree::FlattenOptions { show_empty_ports: true, show_child_devices: false });
    let mut out = format!(
        "USB topology report – {} – {}\n\n",
        snap.computer.name, snap.taken_at
    );
    for n in &flat.nodes {
        let port = if n.port_label.is_empty() { String::new() } else { format!("[{}] ", n.port_label) };
        out += &format!("{}{}{}  {}\n", "  ".repeat(n.depth), port, n.label, n.detail);
    }
    out += "\n";
    for n in &flat.nodes {
        if n.kind == tree::NodeKind::EmptyPort {
            continue;
        }
        out += "\n\n";
        out += &to_text(&n.label, &sections(snap, &n.path), hexdumps);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_demo_node_has_sections_and_report_renders() {
        let s = crate::demo::snapshot();
        let f = tree::flatten(&s, &tree::FlattenOptions { show_empty_ports: true, show_child_devices: true });
        for n in &f.nodes {
            assert!(!sections(&s, &n.path).is_empty(), "{}", n.id);
        }
        let r = full_report(&s, true);
        assert!(r.contains("Ultra Fit"));
        assert!(r.contains("Configuration Descriptor"));
        assert!(r.contains("0000  12 01"));
    }

    #[test]
    fn demo_good_devices_decode_without_warnings() {
        let s = crate::demo::snapshot();
        let f = tree::flatten(&s, &tree::FlattenOptions { show_empty_ports: false, show_child_devices: false });
        for n in &f.nodes {
            if let NodePath::Port(ci, ch) = &n.path {
                let Some(d) = tree::port(&s, *ci, ch).unwrap().device.as_ref() else { continue };
                if d.info.as_ref().is_some_and(|i| i.problem_code != 0) {
                    continue;
                }
                let c = descriptors::decode_configuration(&d.config_descriptor, &|_| None, d.speed.is_super());
                let w: Vec<_> = c.walk().into_iter().flat_map(|x| x.warnings.clone()).collect();
                assert!(w.is_empty(), "{}: {w:?}", n.label);
            }
        }
    }

    #[test]
    fn human_bytes_formats() {
        assert_eq!(human_bytes(512), "512 B");
        assert_eq!(human_bytes(123_010_547_712), "123.0 GB");
    }
}
