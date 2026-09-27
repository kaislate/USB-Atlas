//! Serializable model of the USB topology.  A [`Snapshot`] is both what the
//! UI renders and what gets saved / loaded as a JSON report.

use serde::{Deserialize, Serialize};

use crate::descriptors::DeviceDescriptor;

#[derive(
    Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub enum Speed {
    #[default]
    Unknown,
    Low,
    Full,
    High,
    Super,
    SuperPlus,
    SuperPlus20,
}

impl Speed {
    pub fn label(self) -> &'static str {
        match self {
            Speed::Unknown => "Unknown",
            Speed::Low => "Low Speed (1.5 Mbit/s)",
            Speed::Full => "Full Speed (12 Mbit/s)",
            Speed::High => "High Speed (480 Mbit/s)",
            Speed::Super => "SuperSpeed (5 Gbit/s)",
            Speed::SuperPlus => "SuperSpeed+ (10 Gbit/s)",
            Speed::SuperPlus20 => "SuperSpeed+ (20 Gbit/s)",
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            Speed::Unknown => "?",
            Speed::Low => "LS",
            Speed::Full => "FS",
            Speed::High => "HS",
            Speed::Super => "SS",
            Speed::SuperPlus => "10G",
            Speed::SuperPlus20 => "20G",
        }
    }

    pub fn mbps(self) -> f32 {
        match self {
            Speed::Unknown => 0.0,
            Speed::Low => 1.5,
            Speed::Full => 12.0,
            Speed::High => 480.0,
            Speed::Super => 5000.0,
            Speed::SuperPlus => 10000.0,
            Speed::SuperPlus20 => 20000.0,
        }
    }

    pub fn is_super(self) -> bool {
        self >= Speed::Super
    }
}

/// USB_CONNECTION_STATUS from usbioctl.h.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum ConnectionStatus {
    #[default]
    NoDevice,
    Connected,
    FailedEnumeration,
    GeneralFailure,
    Overcurrent,
    NotEnoughPower,
    NotEnoughBandwidth,
    HubNestedTooDeeply,
    InLegacyHub,
    Enumerating,
    Reset,
    Other(u32),
}

impl ConnectionStatus {
    pub fn from_raw(v: u32) -> Self {
        use ConnectionStatus::*;
        match v {
            0 => NoDevice,
            1 => Connected,
            2 => FailedEnumeration,
            3 => GeneralFailure,
            4 => Overcurrent,
            5 => NotEnoughPower,
            6 => NotEnoughBandwidth,
            7 => HubNestedTooDeeply,
            8 => InLegacyHub,
            9 => Enumerating,
            10 => Reset,
            n => Other(n),
        }
    }

    pub fn label(self) -> String {
        use ConnectionStatus::*;
        match self {
            NoDevice => "No device connected".into(),
            Connected => "Device connected".into(),
            FailedEnumeration => "Device failed enumeration".into(),
            GeneralFailure => "Device general failure".into(),
            Overcurrent => "Device caused over-current".into(),
            NotEnoughPower => "Not enough power for device".into(),
            NotEnoughBandwidth => "Not enough bandwidth for device".into(),
            HubNestedTooDeeply => "Hub nested too deeply".into(),
            InLegacyHub => "Device in legacy hub".into(),
            Enumerating => "Device enumerating".into(),
            Reset => "Device being reset".into(),
            Other(n) => format!("Unknown status {n}"),
        }
    }

    pub fn is_error(self) -> bool {
        use ConnectionStatus::*;
        matches!(
            self,
            FailedEnumeration
                | GeneralFailure
                | Overcurrent
                | NotEnoughPower
                | NotEnoughBandwidth
                | HubNestedTooDeeply
        )
    }
}

/// Windows device node information (SetupAPI / CfgMgr32 properties).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct DevInfo {
    pub instance_id: String,
    pub friendly_name: String,
    pub description: String,
    pub bus_reported_desc: String,
    pub manufacturer: String,
    pub service: String,
    pub class: String,
    pub class_guid: String,
    pub driver_key: String,
    pub driver_version: String,
    pub driver_date: String,
    pub driver_provider: String,
    pub driver_inf: String,
    pub location_info: String,
    pub location_paths: Vec<String>,
    pub hardware_ids: Vec<String>,
    pub compatible_ids: Vec<String>,
    pub container_id: String,
    pub pdo_name: String,
    pub upper_filters: Vec<String>,
    pub lower_filters: Vec<String>,
    pub problem_code: u32,
    pub status_flags: u32,
    pub power_state: String,
    pub enumerator: String,
    pub com_port: String,
    pub volumes: Vec<Volume>,
    pub children: Vec<DevInfo>,
}

impl DevInfo {
    pub fn display_name(&self) -> &str {
        if !self.friendly_name.is_empty() {
            &self.friendly_name
        } else if !self.description.is_empty() {
            &self.description
        } else {
            &self.instance_id
        }
    }

    pub fn has_problem(&self) -> bool {
        self.problem_code != 0
    }

    pub fn is_disabled(&self) -> bool {
        self.problem_code == 22
    }

    /// Recursively collects volumes of this node and its children.
    pub fn all_volumes(&self) -> Vec<&Volume> {
        let mut v: Vec<&Volume> = self.volumes.iter().collect();
        for c in &self.children {
            v.extend(c.all_volumes());
        }
        v
    }

    pub fn all_com_ports(&self) -> Vec<&str> {
        let mut v = Vec::new();
        if !self.com_port.is_empty() {
            v.push(self.com_port.as_str());
        }
        for c in &self.children {
            v.extend(c.all_com_ports());
        }
        v
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Volume {
    pub mount_points: Vec<String>,
    pub label: String,
    pub file_system: String,
    pub total_bytes: u64,
    pub free_bytes: u64,
    pub volume_name: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct PortConnector {
    pub companion_index: u16,
    pub companion_port: u16,
    pub companion_hub: String,
    pub user_connectable: bool,
    pub debug_capable: bool,
    pub multi_companions: bool,
    pub type_c: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct PipeInfo {
    pub endpoint_address: u8,
    pub attributes: u8,
    pub max_packet: u16,
    pub interval: u8,
    pub schedule_offset: u32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct StringDesc {
    pub index: u8,
    pub lang: u16,
    pub text: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Device {
    pub address: u16,
    pub speed: Speed,
    pub is_hub: bool,
    pub current_config: u8,
    pub open_pipes: u32,
    pub device_descriptor: Vec<u8>,
    pub config_descriptor: Vec<u8>,
    pub bos_descriptor: Vec<u8>,
    pub device_qualifier: Vec<u8>,
    pub lang_ids: Vec<u16>,
    pub strings: Vec<StringDesc>,
    pub driver_key: String,
    pub pipes: Vec<PipeInfo>,
    /// USB_NODE_CONNECTION_INFORMATION_EX_V2 flags.
    pub operating_at_ss_or_higher: bool,
    pub ss_capable: bool,
    pub operating_at_ssp: bool,
    pub ssp_capable: bool,
    pub info: Option<DevInfo>,
    pub hub: Option<Box<Hub>>,
}

impl Device {
    pub fn descriptor(&self) -> Option<DeviceDescriptor> {
        DeviceDescriptor::from_bytes(&self.device_descriptor)
    }

    pub fn string(&self, index: u8) -> Option<&str> {
        if index == 0 {
            return None;
        }
        let pref = self.lang_ids.first().copied().unwrap_or(0x0409);
        self.strings
            .iter()
            .find(|s| s.index == index && s.lang == pref)
            .or_else(|| self.strings.iter().find(|s| s.index == index))
            .map(|s| s.text.as_str())
    }

    pub fn vid_pid(&self) -> Option<(u16, u16)> {
        self.descriptor().map(|d| (d.vid, d.pid))
    }

    pub fn serial(&self) -> Option<&str> {
        self.descriptor().and_then(|d| self.string(d.i_serial))
    }

    /// Current requested from the bus in mA according to the config descriptor.
    pub fn max_power_ma(&self) -> Option<u32> {
        let c = &self.config_descriptor;
        if c.len() < 9 {
            return None;
        }
        let unit = if self.speed.is_super() { 8 } else { 2 };
        Some(c[8] as u32 * unit)
    }

    /// Fastest link the device is known to support.
    pub fn max_capable_speed(&self) -> Speed {
        let bcd = self.descriptor().map(|d| d.bcd_usb).unwrap_or(0);
        let cap = if self.ssp_capable {
            Speed::SuperPlus
        } else if self.ss_capable || bcd >= 0x0300 {
            Speed::Super
        } else if !self.device_qualifier.is_empty() {
            Speed::High
        } else {
            self.speed
        };
        cap.max(self.speed)
    }

    pub fn self_powered(&self) -> Option<bool> {
        self.config_descriptor.get(7).map(|a| a & 0x40 != 0)
    }

    /// Best available human name for the device.
    pub fn name(&self) -> String {
        if let Some(d) = self.descriptor() {
            if let Some(p) = self.string(d.i_product) {
                if !p.trim().is_empty() {
                    return p.trim().to_string();
                }
            }
        }
        if let Some(i) = &self.info {
            let n = i.display_name();
            if !n.is_empty() {
                return n.to_string();
            }
        }
        if let Some((v, p)) = self.vid_pid() {
            let db = crate::usbids::db();
            if let Some(n) = db.product(v, p) {
                return n.to_string();
            }
            return format!("Device {v:04X}:{p:04X}");
        }
        "Unknown device".into()
    }

    /// Interface classes present in the configuration descriptor.
    pub fn interface_classes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        if let Some(d) = self.descriptor() {
            if d.class != 0 && d.class != 0xEF {
                out.push(d.class);
            }
        }
        let c = &self.config_descriptor;
        let mut pos = 0;
        while pos + 2 <= c.len() {
            let len = c[pos] as usize;
            if len < 2 {
                break;
            }
            if c[pos + 1] == 0x04 && pos + 5 < c.len() && !out.contains(&c[pos + 5]) {
                out.push(c[pos + 5]);
            }
            pos += len;
        }
        out
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Port {
    pub index: u32,
    pub status: ConnectionStatus,
    pub connector: Option<PortConnector>,
    pub supports_usb110: bool,
    pub supports_usb200: bool,
    pub supports_usb300: bool,
    pub device: Option<Device>,
}

impl Port {
    pub fn max_speed(&self) -> Speed {
        if self.supports_usb300 {
            Speed::Super
        } else if self.supports_usb200 {
            Speed::High
        } else if self.supports_usb110 {
            Speed::Full
        } else {
            Speed::Unknown
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Hub {
    pub symbolic_name: String,
    pub is_root: bool,
    pub hub_type: String,
    pub descriptor: Vec<u8>,
    pub num_ports: u32,
    pub bus_powered: bool,
    pub high_speed_capable: bool,
    pub multi_tt: bool,
    pub info: Option<DevInfo>,
    pub ports: Vec<Port>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Controller {
    pub info: DevInfo,
    pub pci_vendor: Option<u16>,
    pub pci_device: Option<u16>,
    pub pci_revision: Option<u8>,
    pub flavor: String,
    pub num_root_ports: u32,
    pub root_hub: Option<Hub>,
    pub error: Option<String>,
}

impl Controller {
    pub fn kind(&self) -> &'static str {
        let s = format!("{} {}", self.info.display_name(), self.flavor).to_lowercase();
        if s.contains("xhci")
            || s.contains("extensible")
            || s.contains("usb 3")
            || s.contains("usb4")
        {
            "xHCI"
        } else if s.contains("ehci") || s.contains("enhanced") {
            "EHCI"
        } else if s.contains("ohci") || s.contains("open host") {
            "OHCI"
        } else if s.contains("uhci") || s.contains("universal host") {
            "UHCI"
        } else {
            "USB"
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ComputerInfo {
    pub name: String,
    pub os: String,
    pub user: String,
    pub is_admin: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Snapshot {
    pub format: u32,
    pub app_version: String,
    pub taken_at: String,
    pub computer: ComputerInfo,
    pub controllers: Vec<Controller>,
    pub scan_ms: u64,
}

impl Snapshot {
    pub const FORMAT: u32 = 1;
}
