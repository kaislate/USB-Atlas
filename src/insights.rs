//! Heuristic diagnostics: "why is my device slow / not working?"

use crate::model::*;
use crate::tree::{self, Flat, NodePath};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug)]
pub struct Insight {
    pub severity: Severity,
    pub node_id: String,
    pub title: String,
    pub detail: String,
}

/// Returns a message if the device could run faster than it currently does.
pub fn problem_text(code: u32) -> &'static str {
    match code {
        1 => "Not configured correctly (CM_PROB_NOT_CONFIGURED)",
        3 => "Driver may be corrupted or out of memory (CM_PROB_OUT_OF_MEMORY)",
        10 => "Device cannot start (CM_PROB_FAILED_START)",
        12 => "Not enough free resources (CM_PROB_NORMAL_CONFLICT)",
        14 => "Restart required (CM_PROB_NEED_RESTART)",
        18 => "Drivers need to be reinstalled (CM_PROB_REINSTALL)",
        19 => "Registry configuration is damaged (CM_PROB_REGISTRY)",
        21 => "Windows is removing the device (CM_PROB_WILL_BE_REMOVED)",
        22 => "Device is disabled (CM_PROB_DISABLED)",
        24 => "Device not present or not working (CM_PROB_DEVICE_NOT_THERE)",
        28 => "Drivers are not installed (CM_PROB_FAILED_INSTALL)",
        29 => "Disabled by firmware (CM_PROB_HARDWARE_DISABLED)",
        31 => "Windows cannot load the drivers (CM_PROB_FAILED_ADD)",
        32 => "Driver service disabled (CM_PROB_DISABLED_SERVICE)",
        37 => "Driver initialization failed (CM_PROB_FAILED_DRIVER_ENTRY)",
        38 => "Previous driver instance still in memory (CM_PROB_DRIVER_FAILED_PRIOR_UNLOAD)",
        39 => "Driver corrupted or missing (CM_PROB_DRIVER_FAILED_LOAD)",
        41 => "Driver loaded but device not found (CM_PROB_PHANTOM)",
        43 => "Device reported problems – often a failed descriptor request (CM_PROB_FAILED_POST_START)",
        45 => "Device is not connected (CM_PROB_PHANTOM)",
        47 => "Prepared for safe removal (CM_PROB_HELD_FOR_EJECT)",
        48 => "Driver blocked (CM_PROB_DRIVER_BLOCKED)",
        49 => "Registry size limit exceeded (CM_PROB_REGISTRY_TOO_LARGE)",
        52 => "Driver signature cannot be verified (CM_PROB_UNSIGNED_DRIVER)",
        _ => "Unknown problem",
    }
}

/// Current each port of a hub can supply in mA.
pub fn port_budget_ma(hub: &Hub, port: &Port) -> u32 {
    if hub.bus_powered && !hub.is_root {
        100
    } else if port.device.as_ref().is_some_and(|d| d.speed.is_super()) {
        900
    } else {
        500
    }
}

/// True if the configuration has a HID boot-keyboard interface (3/1/1).
pub fn has_boot_keyboard(cfg: &[u8]) -> bool {
    let mut pos = 0;
    while pos + 2 <= cfg.len() {
        let len = cfg[pos] as usize;
        if len < 2 {
            break;
        }
        if cfg[pos + 1] == 0x04
            && pos + 7 < cfg.len()
            && cfg[pos + 5] == 0x03
            && cfg[pos + 6] == 0x01
            && cfg[pos + 7] == 0x01
        {
            return true;
        }
        pos += len;
    }
    false
}

pub fn analyze(snap: &Snapshot, flat: &Flat) -> Vec<Insight> {
    let mut out = Vec::new();
    let phys = crate::physical::Physical::build(snap);
    for n in &flat.nodes {
        match &n.path {
            NodePath::Controller(ci) => {
                let c = &snap.controllers[*ci];
                if c.info.has_problem() {
                    out.push(Insight {
                        severity: Severity::Error,
                        node_id: n.id.clone(),
                        title: format!("Controller problem code {}", c.info.problem_code),
                        detail: problem_text(c.info.problem_code).into(),
                    });
                }
            }
            NodePath::Port(ci, ch) => {
                let Some(p) = tree::port(snap, *ci, ch) else {
                    continue;
                };
                if p.status.is_error() {
                    out.push(Insight {
                        severity: Severity::Error,
                        node_id: n.id.clone(),
                        title: p.status.label(),
                        detail: match p.status {
                            ConnectionStatus::Overcurrent => "The device drew too much current. Unplug it; Windows re-enables the port after the fault clears.".into(),
                            ConnectionStatus::FailedEnumeration => "The device did not answer descriptor requests. Try another cable/port or a powered hub.".into(),
                            ConnectionStatus::NotEnoughPower => "Connect the device to a self-powered hub or a root port.".into(),
                            _ => String::new(),
                        },
                    });
                }
                let Some(d) = &p.device else { continue };
                if let Some(msg) = crate::physical::explain_speed(snap, &phys, *ci, ch) {
                    out.push(Insight {
                        severity: Severity::Info,
                        node_id: n.id.clone(),
                        title: format!("{} could be faster", n.label),
                        detail: msg,
                    });
                }
                if let Some(i) = &d.info {
                    if i.has_problem() {
                        out.push(Insight {
                            severity: if i.is_disabled() {
                                Severity::Warning
                            } else {
                                Severity::Error
                            },
                            node_id: n.id.clone(),
                            title: format!("{}: problem code {}", n.label, i.problem_code),
                            detail: problem_text(i.problem_code).into(),
                        });
                    }
                }
                // Power budget
                let parent_hub = if ch.len() == 1 {
                    snap.controllers[*ci].root_hub.as_ref()
                } else {
                    tree::port(snap, *ci, &ch[..ch.len() - 1])
                        .and_then(|pp| pp.device.as_ref())
                        .and_then(|d| d.hub.as_deref())
                };
                if let (Some(h), Some(ma)) = (parent_hub, d.max_power_ma()) {
                    let budget = port_budget_ma(h, p);
                    if ma > budget && d.self_powered() != Some(true) {
                        out.push(Insight {
                            severity: Severity::Warning,
                            node_id: n.id.clone(),
                            title: format!("{} needs {ma} mA, port supplies {budget} mA", n.label),
                            detail: "The device requests more current than this port is specified for. Use a self-powered hub or a root port.".into(),
                        });
                    }
                }
                // BadUSB-style heuristic: a storage/network device that also exposes a keyboard.
                let classes = d.interface_classes();
                let has_hid = has_boot_keyboard(&d.config_descriptor);
                if has_hid
                    && (classes.contains(&0x08)
                        || classes.contains(&0x02)
                        || classes.contains(&0xE0))
                {
                    out.push(Insight {
                        severity: Severity::Warning,
                        node_id: n.id.clone(),
                        title: format!("{} combines HID with storage/network", n.label),
                        detail: "Composite devices that act as a keyboard *and* a drive or network adapter are sometimes malicious (BadUSB). Verify that this is expected.".into(),
                    });
                }
                if classes.contains(&0x08) && d.serial().is_none() {
                    out.push(Insight {
                        severity: Severity::Info,
                        node_id: n.id.clone(),
                        title: format!("{} has no serial number", n.label),
                        detail: "The USB Mass Storage spec requires a unique serial number. Windows will treat the drive as new on every port.".into(),
                    });
                }
                let dd = crate::descriptors::decode_device(&d.device_descriptor, &|_| None);
                let cd = crate::descriptors::decode_configuration(
                    &d.config_descriptor,
                    &|_| None,
                    d.speed.is_super(),
                );
                let warnings: Vec<String> = dd
                    .walk()
                    .into_iter()
                    .chain(if d.config_descriptor.is_empty() {
                        vec![]
                    } else {
                        cd.walk()
                    })
                    .flat_map(|x| x.warnings.clone())
                    .collect();
                if !warnings.is_empty() {
                    out.push(Insight {
                        severity: Severity::Warning,
                        node_id: n.id.clone(),
                        title: format!("{}: descriptor issues", n.label),
                        detail: warnings.join("\n"),
                    });
                }
                if ch.len() > 5 {
                    out.push(Insight {
                        severity: Severity::Info,
                        node_id: n.id.clone(),
                        title: format!("{} is {} hubs deep", n.label, ch.len() - 1),
                        detail: "USB allows at most 5 hubs between host and device.".into(),
                    });
                }
            }
            _ => {}
        }
    }
    out.sort_by(|a, b| b.severity.cmp(&a.severity));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::{flatten, FlattenOptions};

    #[test]
    fn demo_produces_expected_insights() {
        let s = crate::demo::snapshot();
        let f = flatten(
            &s,
            &FlattenOptions {
                show_empty_ports: true,
                show_child_devices: true,
                physical: false,
            },
        );
        let ins = analyze(&s, &f);
        assert!(
            ins.iter().any(|i| i.title.contains("could be faster")),
            "{ins:#?}"
        );
        assert!(ins.iter().any(|i| i.title.contains("problem code 43")));
        assert!(ins.iter().any(|i| i.title.contains("over-current")));
        // Sorted most severe first
        assert!(ins.windows(2).all(|w| w[0].severity >= w[1].severity));
    }
}
