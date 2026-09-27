//! Flattens a [`Snapshot`] into a list of display nodes with stable ids,
//! and diffs two snapshots into arrival / removal events.

use std::collections::HashMap;

use crate::model::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NodePath {
    Computer,
    Controller(usize),
    RootHub(usize),
    /// Controller index + chain of port indexes (0-based into `ports`).
    Port(usize, Vec<usize>),
    /// A Windows child device node below a USB device: port path + chain of
    /// indexes into `DevInfo::children`.
    Child(usize, Vec<usize>, Vec<usize>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Health {
    Empty,
    Ok,
    Info,
    Warning,
    Error,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NodeKind {
    Computer,
    Controller,
    RootHub,
    EmptyPort,
    Device,
    Hub,
    Child,
}

#[derive(Clone, Debug)]
pub struct FlatNode {
    pub id: String,
    pub parent: Option<usize>,
    pub depth: usize,
    pub kind: NodeKind,
    pub path: NodePath,
    pub label: String,
    /// Secondary text (drive letters, COM port, VID:PID ...).
    pub detail: String,
    pub port_label: String,
    pub speed: Speed,
    pub port_max_speed: Speed,
    pub health: Health,
    pub classes: Vec<u8>,
    pub children: Vec<usize>,
    pub search_text: String,
}

#[derive(Default)]
pub struct Flat {
    pub nodes: Vec<FlatNode>,
    pub by_id: HashMap<String, usize>,
}

impl Flat {
    pub fn get(&self, id: &str) -> Option<&FlatNode> {
        self.by_id.get(id).map(|&i| &self.nodes[i])
    }

    /// Ancestors of `idx`, root first.
    pub fn ancestors(&self, idx: usize) -> Vec<usize> {
        let mut v = Vec::new();
        let mut cur = self.nodes[idx].parent;
        while let Some(p) = cur {
            v.push(p);
            cur = self.nodes[p].parent;
        }
        v.reverse();
        v
    }
}

pub fn port<'a>(snap: &'a Snapshot, ctrl: usize, chain: &[usize]) -> Option<&'a Port> {
    let mut hub = snap.controllers.get(ctrl)?.root_hub.as_ref()?;
    let (last, head) = chain.split_last()?;
    for &i in head {
        hub = hub.ports.get(i)?.device.as_ref()?.hub.as_deref()?;
    }
    hub.ports.get(*last)
}

pub fn child<'a>(dev: &'a DevInfo, chain: &[usize]) -> Option<&'a DevInfo> {
    let mut cur = dev;
    for &i in chain {
        cur = cur.children.get(i)?;
    }
    Some(cur)
}

/// Stable, human-readable port chain like `1-4-2`.
pub fn port_chain(snap: &Snapshot, ctrl: usize, chain: &[usize]) -> String {
    let mut parts = Vec::new();
    for n in 1..=chain.len() {
        if let Some(p) = port(snap, ctrl, &chain[..n]) {
            parts.push(p.index.to_string());
        }
    }
    parts.join("-")
}

fn ctrl_id(c: &Controller, i: usize) -> String {
    if c.info.instance_id.is_empty() {
        format!("ctrl{i}")
    } else {
        c.info.instance_id.clone()
    }
}

pub struct FlattenOptions {
    pub show_empty_ports: bool,
    pub show_child_devices: bool,
}

pub fn flatten(snap: &Snapshot, opts: &FlattenOptions) -> Flat {
    let mut f = Flat::default();
    let comp = push(
        &mut f,
        FlatNode {
            id: "computer".into(),
            parent: None,
            depth: 0,
            kind: NodeKind::Computer,
            path: NodePath::Computer,
            label: if snap.computer.name.is_empty() { "This Computer".into() } else { snap.computer.name.clone() },
            detail: snap.computer.os.clone(),
            port_label: String::new(),
            speed: Speed::Unknown,
            port_max_speed: Speed::Unknown,
            health: Health::Ok,
            classes: vec![],
            children: vec![],
            search_text: String::new(),
        },
    );
    for (ci, c) in snap.controllers.iter().enumerate() {
        let cid = ctrl_id(c, ci);
        let health = if c.info.has_problem() || c.error.is_some() { Health::Error } else { Health::Ok };
        let mut detail = c.kind().to_string();
        if let (Some(v), Some(d)) = (c.pci_vendor, c.pci_device) {
            detail = format!("{detail} · PCI {v:04X}:{d:04X}");
        }
        let cn = push(
            &mut f,
            FlatNode {
                id: cid.clone(),
                parent: Some(comp),
                depth: 1,
                kind: NodeKind::Controller,
                path: NodePath::Controller(ci),
                label: c.info.display_name().to_string(),
                search_text: search_blob(&[c.info.display_name(), &c.info.instance_id, &detail]),
                detail,
                port_label: String::new(),
                speed: Speed::Unknown,
                port_max_speed: Speed::Unknown,
                health,
                classes: vec![],
                children: vec![],
            },
        );
        if let Some(rh) = &c.root_hub {
            let rn = push(
                &mut f,
                FlatNode {
                    id: format!("{cid}/RH"),
                    parent: Some(cn),
                    depth: 2,
                    kind: NodeKind::RootHub,
                    path: NodePath::RootHub(ci),
                    label: "Root Hub".into(),
                    detail: format!("{} ports", rh.num_ports),
                    port_label: String::new(),
                    speed: Speed::Unknown,
                    port_max_speed: Speed::Unknown,
                    health: if rh.error.is_some() { Health::Warning } else { Health::Ok },
                    classes: vec![0x09],
                    children: vec![],
                    search_text: search_blob(&["root hub", &rh.symbolic_name]),
                },
            );
            add_ports(&mut f, snap, opts, rn, 3, ci, &cid, vec![], rh);
        }
    }
    f
}

fn push(f: &mut Flat, n: FlatNode) -> usize {
    let idx = f.nodes.len();
    if let Some(p) = n.parent {
        f.nodes[p].children.push(idx);
    }
    f.by_id.insert(n.id.clone(), idx);
    f.nodes.push(n);
    idx
}

fn search_blob(parts: &[&str]) -> String {
    parts.iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(" ").to_lowercase()
}

#[allow(clippy::too_many_arguments)]
fn add_ports(
    f: &mut Flat,
    snap: &Snapshot,
    opts: &FlattenOptions,
    parent: usize,
    depth: usize,
    ci: usize,
    cid: &str,
    chain: Vec<usize>,
    hub: &Hub,
) {
    for (pi, p) in hub.ports.iter().enumerate() {
        let mut ch = chain.clone();
        ch.push(pi);
        let pchain = port_chain(snap, ci, &ch);
        let id = format!("{cid}/{pchain}");
        let port_label = format!("Port {}", p.index);
        let Some(dev) = &p.device else {
            if !opts.show_empty_ports && !p.status.is_error() {
                continue;
            }
            let health = if p.status.is_error() { Health::Error } else { Health::Empty };
            let label = if p.status.is_error() { p.status.label() } else { "Empty".into() };
            let mut tags = Vec::new();
            if p.connector.as_ref().is_some_and(|c| c.type_c) {
                tags.push("Type-C");
            }
            if p.connector.as_ref().is_some_and(|c| !c.user_connectable) {
                tags.push("internal");
            }
            let detail = tags.join(" · ");
            push(
                f,
                FlatNode {
                    id,
                    parent: Some(parent),
                    depth,
                    kind: NodeKind::EmptyPort,
                    path: NodePath::Port(ci, ch),
                    search_text: search_blob(&[&label, &port_label, &pchain]),
                    label,
                    detail,
                    port_label,
                    speed: Speed::Unknown,
                    port_max_speed: p.max_speed(),
                    health,
                    classes: vec![],
                    children: vec![],
                },
            );
            continue;
        };
        let name = dev.name();
        let info = dev.info.as_ref();
        let mut extras: Vec<String> = Vec::new();
        if let Some(i) = info {
            for v in i.all_volumes() {
                for m in &v.mount_points {
                    extras.push(m.trim_end_matches('\\').to_string());
                }
            }
            for c in i.all_com_ports() {
                extras.push(c.to_string());
            }
        }
        let mut health = Health::Ok;
        if p.status.is_error() || info.is_some_and(|i| i.has_problem() && !i.is_disabled()) {
            health = Health::Error;
        } else if info.is_some_and(DevInfo::is_disabled) {
            health = Health::Warning;
        } else if crate::insights::speed_mismatch(p, dev).is_some() {
            health = Health::Info;
        }
        let vidpid = dev.vid_pid().map(|(v, p)| format!("{v:04X}:{p:04X}")).unwrap_or_default();
        let mut search = vec![name.clone(), port_label.clone(), pchain.clone(), vidpid.clone()];
        if let Some(i) = info {
            search.extend([
                i.instance_id.clone(),
                i.description.clone(),
                i.friendly_name.clone(),
                i.manufacturer.clone(),
                i.service.clone(),
                i.class.clone(),
            ]);
            for c in &i.children {
                search.push(c.display_name().to_string());
                search.push(c.instance_id.clone());
            }
        }
        for s in &dev.strings {
            search.push(s.text.clone());
        }
        if let Some((v, _)) = dev.vid_pid() {
            if let Some(vn) = crate::usbids::db().vendor(v) {
                search.push(vn.to_string());
            }
        }
        search.extend(extras.iter().cloned());
        let refs: Vec<&str> = search.iter().map(String::as_str).collect();
        let kind = if dev.hub.is_some() || dev.is_hub { NodeKind::Hub } else { NodeKind::Device };
        let detail = if extras.is_empty() { vidpid } else { extras.join(" ") };
        let dn = push(
            f,
            FlatNode {
                id,
                parent: Some(parent),
                depth,
                kind,
                path: NodePath::Port(ci, ch.clone()),
                label: name,
                detail,
                port_label,
                speed: dev.speed,
                port_max_speed: p.max_speed(),
                health,
                classes: dev.interface_classes(),
                children: vec![],
                search_text: search_blob(&refs),
            },
        );
        if let Some(h) = &dev.hub {
            add_ports(f, snap, opts, dn, depth + 1, ci, cid, ch.clone(), h);
        } else if opts.show_child_devices {
            if let Some(i) = info {
                add_children(f, dn, depth + 1, ci, &ch, vec![], i);
            }
        }
    }
}

fn add_children(f: &mut Flat, parent: usize, depth: usize, ci: usize, port: &[usize], chain: Vec<usize>, info: &DevInfo) {
    for (i, c) in info.children.iter().enumerate() {
        let mut ch = chain.clone();
        ch.push(i);
        let mut detail: Vec<String> = c
            .volumes
            .iter()
            .flat_map(|v| v.mount_points.iter().map(|m| m.trim_end_matches('\\').to_string()))
            .collect();
        if !c.com_port.is_empty() {
            detail.push(c.com_port.clone());
        }
        let pid = f.nodes[parent].id.clone();
        let n = push(
            f,
            FlatNode {
                id: format!("{pid}#{}", c.instance_id),
                parent: Some(parent),
                depth,
                kind: NodeKind::Child,
                path: NodePath::Child(ci, port.to_vec(), ch.clone()),
                label: c.display_name().to_string(),
                detail: if detail.is_empty() { c.class.clone() } else { detail.join(" ") },
                port_label: String::new(),
                speed: Speed::Unknown,
                port_max_speed: Speed::Unknown,
                health: if c.has_problem() { Health::Error } else { Health::Ok },
                classes: vec![],
                children: vec![],
                search_text: search_blob(&[c.display_name(), &c.instance_id, &c.class, &detail.join(" ")]),
            },
        );
        add_children(f, n, depth + 1, ci, port, ch, c);
    }
}

/// A device identity used to detect arrivals / removals across snapshots.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DeviceKey {
    pub location: String,
    pub vid: u16,
    pub pid: u16,
    pub serial: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ChangeKind {
    Arrived,
    Removed,
    ProblemAppeared(u32),
    ProblemCleared,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Change {
    pub kind: ChangeKind,
    pub node_id: String,
    pub name: String,
    pub vid_pid: Option<(u16, u16)>,
}

fn device_map(snap: &Snapshot) -> HashMap<String, (DeviceKey, String, u32)> {
    let flat = flatten(snap, &FlattenOptions { show_empty_ports: false, show_child_devices: false });
    let mut m = HashMap::new();
    for n in &flat.nodes {
        if let NodePath::Port(ci, ch) = &n.path {
            if let Some(dev) = port(snap, *ci, ch).and_then(|p| p.device.as_ref()) {
                let (vid, pid) = dev.vid_pid().unwrap_or((0, 0));
                let key = DeviceKey {
                    location: n.id.clone(),
                    vid,
                    pid,
                    serial: dev.serial().unwrap_or("").to_string(),
                };
                let prob = dev.info.as_ref().map(|i| i.problem_code).unwrap_or(0);
                m.insert(n.id.clone(), (key, n.label.clone(), prob));
            }
        }
    }
    m
}

/// Differences between an old and a new snapshot.
pub fn diff(old: &Snapshot, new: &Snapshot) -> Vec<Change> {
    let a = device_map(old);
    let b = device_map(new);
    let mut out = Vec::new();
    for (id, (key, name, _)) in &a {
        match b.get(id) {
            Some((k2, _, _)) if k2 == key => {}
            _ => out.push(Change {
                kind: ChangeKind::Removed,
                node_id: id.clone(),
                name: name.clone(),
                vid_pid: Some((key.vid, key.pid)),
            }),
        }
    }
    for (id, (key, name, prob)) in &b {
        match a.get(id) {
            Some((k1, _, old_prob)) if k1 == key => {
                if *old_prob == 0 && *prob != 0 {
                    out.push(Change {
                        kind: ChangeKind::ProblemAppeared(*prob),
                        node_id: id.clone(),
                        name: name.clone(),
                        vid_pid: Some((key.vid, key.pid)),
                    });
                } else if *old_prob != 0 && *prob == 0 {
                    out.push(Change {
                        kind: ChangeKind::ProblemCleared,
                        node_id: id.clone(),
                        name: name.clone(),
                        vid_pid: Some((key.vid, key.pid)),
                    });
                }
            }
            _ => out.push(Change {
                kind: ChangeKind::Arrived,
                node_id: id.clone(),
                name: name.clone(),
                vid_pid: Some((key.vid, key.pid)),
            }),
        }
    }
    out.sort_by(|x, y| x.node_id.cmp(&y.node_id));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo;

    #[test]
    fn flatten_demo_has_unique_ids_and_parents() {
        let s = demo::snapshot();
        let f = flatten(&s, &FlattenOptions { show_empty_ports: true, show_child_devices: true });
        assert_eq!(f.by_id.len(), f.nodes.len(), "ids must be unique");
        for (i, n) in f.nodes.iter().enumerate() {
            if let Some(p) = n.parent {
                assert!(f.nodes[p].children.contains(&i));
                assert!(p < i);
            }
        }
        assert!(f.nodes.iter().any(|n| n.kind == NodeKind::Hub));
        assert!(f.nodes.iter().any(|n| n.kind == NodeKind::Child));
    }

    #[test]
    fn hiding_empty_ports_removes_them() {
        let s = demo::snapshot();
        let with = flatten(&s, &FlattenOptions { show_empty_ports: true, show_child_devices: false });
        let without = flatten(&s, &FlattenOptions { show_empty_ports: false, show_child_devices: false });
        assert!(with.nodes.len() > without.nodes.len());
        assert!(without.nodes.iter().all(|n| n.kind != NodeKind::EmptyPort || n.health == Health::Error));
    }

    #[test]
    fn port_lookup_follows_hubs() {
        let s = demo::snapshot();
        let f = flatten(&s, &FlattenOptions { show_empty_ports: true, show_child_devices: false });
        for n in &f.nodes {
            if let NodePath::Port(ci, ch) = &n.path {
                assert!(port(&s, *ci, ch).is_some(), "{}", n.id);
            }
        }
    }

    #[test]
    fn diff_detects_arrival_and_removal() {
        let old = demo::snapshot();
        let mut new = old.clone();
        // Unplug the first connected device on the first root hub.
        let rh = new.controllers[0].root_hub.as_mut().unwrap();
        let p = rh.ports.iter_mut().find(|p| p.device.is_some()).unwrap();
        p.device = None;
        p.status = ConnectionStatus::NoDevice;
        let d = diff(&old, &new);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].kind, ChangeKind::Removed);
        let back = diff(&new, &old);
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].kind, ChangeKind::Arrived);
        assert!(diff(&old, &old).is_empty());
    }
}
