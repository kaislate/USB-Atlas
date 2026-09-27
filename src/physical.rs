//! Physical view of the topology.
//!
//! Windows exposes one physical USB 3 socket as two logical ports ("lanes"):
//! a USB 2 port and a SuperSpeed companion port, possibly on different hubs.
//! A USB 3 hub likewise appears as two hubs. This module regroups lanes into
//! physical connectors and merges hub halves, and derives what a device's
//! link *could* achieve (connector maximum, upstream path limit).

use std::collections::{HashMap, HashSet};

use crate::model::*;
use crate::tree;

/// Controller index + port chain (same addressing as `NodePath::Port`).
pub type PortRef = (usize, Vec<usize>);

#[derive(Clone, Debug)]
pub struct Lane {
    pub port: PortRef,
    pub index: u32,
    pub max: Speed,
    pub occupied: bool,
}

#[derive(Clone, Debug)]
pub struct Connector {
    /// Slowest lane first (USB 2, then SuperSpeed).
    pub lanes: Vec<Lane>,
    pub type_c: bool,
    pub internal: bool,
    pub error: bool,
    /// Lanes that carry a device.
    pub attached: Vec<PortRef>,
    /// If the attached device(s) are hub halves: the merged downstream connectors.
    pub children: Vec<Connector>,
}

impl Connector {
    pub fn max(&self) -> Speed {
        self.lanes
            .iter()
            .map(|l| l.max)
            .max()
            .unwrap_or(Speed::Unknown)
    }

    pub fn label(&self) -> String {
        let nums: Vec<String> = self.lanes.iter().map(|l| l.index.to_string()).collect();
        format!("Port {}", nums.join(" · "))
    }

    /// Lane to represent the connector when it is empty (the fastest one).
    pub fn primary(&self) -> &PortRef {
        match self.attached.last() {
            Some(p) => p,
            None => &self.lanes.last().expect("connector has lanes").port,
        }
    }

    pub fn is_usb3(&self) -> bool {
        self.max().is_super()
    }
}

pub struct PhysController {
    pub ci: usize,
    pub connectors: Vec<Connector>,
}

pub struct Physical {
    pub controllers: Vec<PhysController>,
    adj: HashMap<PortRef, Vec<PortRef>>,
}

fn walk_ports(snap: &Snapshot, mut f: impl FnMut(PortRef, &Hub, &Port)) {
    fn rec(h: &Hub, ci: usize, chain: &mut Vec<usize>, f: &mut dyn FnMut(PortRef, &Hub, &Port)) {
        for (i, p) in h.ports.iter().enumerate() {
            chain.push(i);
            f((ci, chain.clone()), h, p);
            if let Some(sub) = p.device.as_ref().and_then(|d| d.hub.as_deref()) {
                rec(sub, ci, chain, f);
            }
            chain.pop();
        }
    }
    for (ci, c) in snap.controllers.iter().enumerate() {
        if let Some(h) = &c.root_hub {
            rec(h, ci, &mut Vec::new(), &mut f);
        }
    }
}

/// Maximum speed of one lane, including what a device on it proves.
pub fn lane_max(p: &Port) -> Speed {
    let mut m = p.max_speed();
    if let Some(d) = &p.device {
        m = m.max(d.speed);
    }
    m
}

impl Physical {
    pub fn build(snap: &Snapshot) -> Self {
        // (hub symbolic name, port number) -> PortRef
        let mut by_name: HashMap<(String, u32), PortRef> = HashMap::new();
        walk_ports(snap, |r, h, p| {
            by_name.insert((h.symbolic_name.to_uppercase(), p.index), r);
        });
        let mut adj: HashMap<PortRef, Vec<PortRef>> = HashMap::new();
        walk_ports(snap, |r, _h, p| {
            if let Some(c) = &p.connector {
                if c.companion_port != 0 && !c.companion_hub.is_empty() {
                    if let Some(other) =
                        by_name.get(&(c.companion_hub.to_uppercase(), c.companion_port as u32))
                    {
                        if *other != r {
                            adj.entry(r.clone()).or_default().push(other.clone());
                            adj.entry(other.clone()).or_default().push(r.clone());
                        }
                    }
                }
            }
        });
        for v in adj.values_mut() {
            v.sort();
            v.dedup();
        }
        let mut phys = Physical {
            controllers: Vec::new(),
            adj,
        };
        let mut visited = HashSet::new();
        for (ci, c) in snap.controllers.iter().enumerate() {
            let refs: Vec<PortRef> = c
                .root_hub
                .as_ref()
                .map(|h| (0..h.ports.len()).map(|i| (ci, vec![i])).collect())
                .unwrap_or_default();
            let connectors = phys.group(snap, refs, &mut visited);
            phys.controllers.push(PhysController { ci, connectors });
        }
        phys
    }

    /// All lanes of the connector that `p` belongs to (including `p`).
    pub fn lanes_of(&self, p: &PortRef) -> Vec<PortRef> {
        let mut seen = vec![p.clone()];
        let mut i = 0;
        while i < seen.len() {
            if let Some(n) = self.adj.get(&seen[i]) {
                for x in n {
                    if !seen.contains(x) {
                        seen.push(x.clone());
                    }
                }
            }
            i += 1;
        }
        seen
    }

    fn group(
        &self,
        snap: &Snapshot,
        refs: Vec<PortRef>,
        visited: &mut HashSet<PortRef>,
    ) -> Vec<Connector> {
        let mut out = Vec::new();
        for r in refs {
            if visited.contains(&r) {
                continue;
            }
            let lane_refs = self.lanes_of(&r);
            let mut lanes = Vec::new();
            for lr in &lane_refs {
                visited.insert(lr.clone());
                if let Some(p) = tree::port(snap, lr.0, &lr.1) {
                    lanes.push(Lane {
                        port: lr.clone(),
                        index: p.index,
                        max: lane_max(p),
                        occupied: p.device.is_some(),
                    });
                }
            }
            if lanes.is_empty() {
                continue;
            }
            lanes.sort_by(|a, b| a.max.cmp(&b.max).then(a.index.cmp(&b.index)));
            let ports: Vec<&Port> = lanes
                .iter()
                .filter_map(|l| tree::port(snap, l.port.0, &l.port.1))
                .collect();
            let type_c = ports
                .iter()
                .any(|p| p.connector.as_ref().is_some_and(|c| c.type_c));
            let internal = ports
                .iter()
                .all(|p| p.connector.as_ref().is_some_and(|c| !c.user_connectable));
            let error = ports.iter().any(|p| p.status.is_error());
            let attached: Vec<PortRef> = lanes
                .iter()
                .filter(|l| l.occupied)
                .map(|l| l.port.clone())
                .collect();
            // Merge hub halves: children are the union of all attached hubs' ports.
            let mut child_refs = Vec::new();
            for a in &attached {
                if let Some(h) = tree::port(snap, a.0, &a.1)
                    .and_then(|p| p.device.as_ref())
                    .and_then(|d| d.hub.as_deref())
                {
                    for i in 0..h.ports.len() {
                        let mut ch = a.1.clone();
                        ch.push(i);
                        child_refs.push((a.0, ch));
                    }
                }
            }
            let children = self.group(snap, child_refs, visited);
            out.push(Connector {
                lanes,
                type_c,
                internal,
                error,
                attached,
                children,
            });
        }
        out
    }
}

/// How a device's link relates to what its connector and path allow.
#[derive(Clone, Debug, PartialEq)]
pub struct LinkInfo {
    /// Maximum of the lane the device is on.
    pub lane_max: Speed,
    /// Maximum of the whole physical connector (all lanes).
    pub connector_max: Speed,
    /// Slowest upstream hub link on the current lane, with that hub's name.
    pub path_limit: Option<(Speed, String)>,
    /// Best speed the device could reach at this connector (capability,
    /// lanes and their upstream paths considered).
    pub best_possible: Speed,
    /// Device sits on a slower lane than the connector offers.
    pub on_slow_lane: bool,
    /// (lane max, is the device's lane) for each lane, slowest first.
    pub lanes: Vec<(Speed, bool)>,
}

/// Slowest hub link above the port `chain` (None when directly on a root hub).
pub fn path_limit(snap: &Snapshot, ci: usize, chain: &[usize]) -> Option<(Speed, String)> {
    let mut best: Option<(Speed, String)> = None;
    for k in 1..chain.len() {
        if let Some(d) = tree::port(snap, ci, &chain[..k]).and_then(|p| p.device.as_ref()) {
            if best.as_ref().is_none_or(|(s, _)| d.speed < *s) {
                best = Some((d.speed, d.name()));
            }
        }
    }
    best
}

pub fn link_info(snap: &Snapshot, phys: &Physical, ci: usize, chain: &[usize]) -> Option<LinkInfo> {
    let me: PortRef = (ci, chain.to_vec());
    let port = tree::port(snap, ci, chain)?;
    let cap = port
        .device
        .as_ref()
        .map(|d| d.max_capable_speed())
        .unwrap_or(Speed::SuperPlus20);
    let mut lanes: Vec<(Speed, bool, Speed)> = Vec::new(); // (max, mine, reachable)
    for lr in phys.lanes_of(&me) {
        if let Some(p) = tree::port(snap, lr.0, &lr.1) {
            let m = lane_max(p);
            let reach = match path_limit(snap, lr.0, &lr.1) {
                Some((s, _)) => m.min(s),
                None => m,
            };
            lanes.push((m, lr == me, reach));
        }
    }
    lanes.sort_by(|a, b| a.0.cmp(&b.0));
    let lane_max_v = lane_max(port);
    let connector_max = lanes.iter().map(|l| l.0).max().unwrap_or(lane_max_v);
    let best_possible = lanes
        .iter()
        .map(|l| l.2)
        .max()
        .unwrap_or(lane_max_v)
        .min(cap);
    Some(LinkInfo {
        lane_max: lane_max_v,
        connector_max,
        path_limit: path_limit(snap, ci, chain),
        best_possible,
        on_slow_lane: lane_max_v < connector_max,
        lanes: lanes.into_iter().map(|(m, mine, _)| (m, mine)).collect(),
    })
}

/// Explains why a device runs slower than it could, or None if it doesn't.
pub fn explain_speed(
    snap: &Snapshot,
    phys: &Physical,
    ci: usize,
    chain: &[usize],
) -> Option<String> {
    let dev = tree::port(snap, ci, chain)?.device.as_ref()?;
    if dev.is_hub || dev.speed == Speed::Unknown {
        return None;
    }
    let cap = dev.max_capable_speed();
    if cap <= dev.speed {
        return None;
    }
    let li = link_info(snap, phys, ci, chain)?;
    let head = format!(
        "The device supports {} but is connected at {}.",
        cap.label(),
        dev.speed.label()
    );
    if li.best_possible > dev.speed {
        if li.on_slow_lane {
            return Some(format!(
                "{head} This socket also has a {} lane, but the device came up on its USB 2 lane – typically a USB 2 cable or adapter, \
                 or a plug that isn't fully inserted. Re-plugging with a USB 3 cable should give {}.",
                li.connector_max.label(),
                li.best_possible.label()
            ));
        }
        return Some(format!(
            "{head} The port supports {}, so check the cable (it may not be rated for this speed).",
            li.best_possible.label()
        ));
    }
    if let Some((s, hub)) = &li.path_limit {
        if *s < cap && *s <= li.connector_max {
            return Some(format!(
                "{head} Everything behind \"{hub}\" is limited to {} because that hub is connected at that speed. \
                 Connect the device directly to the computer or use a faster hub.",
                s.label()
            ));
        }
    }
    Some(format!(
        "{head} This socket only supports {} – move the device to a {} port.",
        li.connector_max.label(),
        if cap.is_super() {
            "USB 3 (blue / SS)"
        } else {
            "faster"
        }
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find(snap: &Snapshot, name: &str) -> PortRef {
        let mut found = None;
        walk_ports(snap, |r, _, p| {
            if p.device.as_ref().is_some_and(|d| d.name() == name) {
                found = Some(r);
            }
        });
        found.unwrap_or_else(|| panic!("{name} not in demo"))
    }

    #[test]
    fn root_hub_lanes_group_into_connectors() {
        let s = crate::demo::snapshot();
        let p = Physical::build(&s);
        let c0 = &p.controllers[0].connectors;
        // 11 logical root ports -> 4 companion pairs + 3 single USB 2 ports.
        assert_eq!(c0.len(), 7);
        let usb3: Vec<_> = c0.iter().filter(|c| c.lanes.len() == 2).collect();
        assert_eq!(usb3.len(), 4);
        assert!(usb3
            .iter()
            .all(|c| c.lanes[0].max == Speed::High && c.lanes[1].max == Speed::Super));
        assert!(c0[1].type_c);
    }

    #[test]
    fn usb3_hub_halves_merge() {
        let s = crate::demo::snapshot();
        let p = Physical::build(&s);
        let hub_conn = p.controllers[0]
            .connectors
            .iter()
            .find(|c| c.attached.len() == 2)
            .expect("both lanes carry a hub half");
        assert_eq!(
            hub_conn.children.len(),
            4,
            "4 physical downstream sockets, not 8"
        );
        assert!(hub_conn.children.iter().all(|c| c.lanes.len() == 2));
    }

    #[test]
    fn flash_on_usb2_lane_of_usb3_socket() {
        let s = crate::demo::snapshot();
        let p = Physical::build(&s);
        let (ci, ch) = find(&s, "Ultra Fit");
        let li = link_info(&s, &p, ci, &ch).unwrap();
        assert_eq!(li.lane_max, Speed::High);
        assert_eq!(
            li.connector_max,
            Speed::Super,
            "port max must include the SuperSpeed companion lane"
        );
        assert!(li.on_slow_lane);
        assert_eq!(li.best_possible, Speed::Super);
        let why = explain_speed(&s, &p, ci, &ch).unwrap();
        assert!(why.contains("USB 2 lane"), "{why}");
    }

    #[test]
    fn ssd_behind_usb2_hub_is_path_limited() {
        let s = crate::demo::snapshot();
        let p = Physical::build(&s);
        let (ci, ch) = find(&s, "Portable SSD T5");
        let li = link_info(&s, &p, ci, &ch).unwrap();
        assert_eq!(li.path_limit.as_ref().map(|x| x.0), Some(Speed::High));
        assert_eq!(li.best_possible, Speed::High);
        let why = explain_speed(&s, &p, ci, &ch).unwrap();
        assert!(why.contains("USB 2.0 Hub"), "{why}");
    }

    #[test]
    fn device_at_full_speed_has_no_explanation() {
        let s = crate::demo::snapshot();
        let p = Physical::build(&s);
        let (ci, ch) = find(&s, "Logitech BRIO");
        assert!(explain_speed(&s, &p, ci, &ch).is_none());
    }
}
