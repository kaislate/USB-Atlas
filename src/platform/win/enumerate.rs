//! USB topology enumeration, following Microsoft's usbview sample.
//! Buffers are parsed by byte offset (the usbioctl.h structs are packed).

use std::collections::HashMap;
use std::time::Instant;

use windows::core::PCWSTR;
use windows::Win32::Devices::DeviceAndDriverInstallation::*;
use windows::Win32::Devices::Usb::*;

use super::devnode;
use super::util::*;
use crate::descriptors::{decode_langids, decode_string, DeviceDescriptor};
use crate::model::*;

pub const IOCTL_USB_USER_REQUEST: u32 = 0x0022_0438;

struct Ctx {
    keys: HashMap<String, u32>,
}

impl Ctx {
    fn devinfo_for_key(&self, key: &str, depth: u32) -> Option<DevInfo> {
        self.keys.get(&key.to_uppercase()).map(|&dn| devnode::read(dn, depth))
    }
}

pub fn scan() -> Snapshot {
    let t0 = Instant::now();
    let ctx = Ctx { keys: devnode::driver_key_map() };
    let mut controllers = Vec::new();
    unsafe {
        if let Ok(h) = SetupDiGetClassDevsW(
            Some(&GUID_DEVINTERFACE_USB_HOST_CONTROLLER),
            PCWSTR::null(),
            None,
            DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
        ) {
            let mut i = 0;
            loop {
                let mut di = SP_DEVINFO_DATA { cbSize: std::mem::size_of::<SP_DEVINFO_DATA>() as u32, ..Default::default() };
                if SetupDiEnumDeviceInfo(h, i, &mut di).is_err() {
                    break;
                }
                i += 1;
                let mut ifd = SP_DEVICE_INTERFACE_DATA {
                    cbSize: std::mem::size_of::<SP_DEVICE_INTERFACE_DATA>() as u32,
                    ..Default::default()
                };
                if SetupDiEnumDeviceInterfaces(h, Some(&di), &GUID_DEVINTERFACE_USB_HOST_CONTROLLER, 0, &mut ifd).is_err() {
                    continue;
                }
                let Some(path) = interface_path(h, &ifd) else { continue };
                controllers.push(controller(&ctx, di.DevInst, &path));
            }
            let _ = SetupDiDestroyDeviceInfoList(h);
        }
    }
    controllers.sort_by(|a, b| a.info.display_name().cmp(b.info.display_name()));
    Snapshot {
        format: Snapshot::FORMAT,
        app_version: env!("CARGO_PKG_VERSION").into(),
        taken_at: super::system::now_string(),
        computer: super::system::computer_info(),
        controllers,
        scan_ms: t0.elapsed().as_millis() as u64,
    }
}

unsafe fn interface_path(h: HDEVINFO, ifd: &SP_DEVICE_INTERFACE_DATA) -> Option<String> {
    let mut size = 0u32;
    let _ = SetupDiGetDeviceInterfaceDetailW(h, ifd, None, 0, Some(&mut size), None);
    if size == 0 {
        return None;
    }
    // u32-aligned buffer for SP_DEVICE_INTERFACE_DETAIL_DATA_W.
    let mut buf = vec![0u32; (size as usize).div_ceil(4)];
    let detail = buf.as_mut_ptr() as *mut SP_DEVICE_INTERFACE_DETAIL_DATA_W;
    (*detail).cbSize = std::mem::size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32;
    SetupDiGetDeviceInterfaceDetailW(h, ifd, Some(detail), size, None, None).ok()?;
    let bytes = std::slice::from_raw_parts(buf.as_ptr() as *const u8, size as usize);
    Some(wide_at(bytes, 4))
}

fn flavor_name(f: u32) -> String {
    match f {
        0 => "Generic".into(),
        1 => "OHCI Generic".into(),
        2 => "OHCI Hydra".into(),
        3 => "OHCI NEC".into(),
        100 => "UHCI Generic".into(),
        101..=109 => "UHCI (Intel PIIX/ICH)".into(),
        200 => "UHCI Intel".into(),
        201 => "UHCI VIA".into(),
        1000 => "EHCI Generic".into(),
        2000 => "EHCI NEC".into(),
        2001 => "EHCI Lucent".into(),
        3000 | 3001 => "EHCI NVIDIA Tegra".into(),
        4000 => "EHCI Intel Medfield".into(),
        n => format!("Flavor {n}"),
    }
}

fn controller(ctx: &Ctx, devinst: u32, path: &str) -> Controller {
    let info = devnode::read(devinst, 0);
    let mut c = Controller { info, ..Default::default() };
    parse_pci_ids(&mut c);
    let h = match open_device(path) {
        Ok(h) => h,
        Err(e) => {
            c.error = Some(format!("Cannot open controller: {e}"));
            return c;
        }
    };
    // USBUSER_GET_CONTROLLER_INFO_0
    let mut req = vec![0u8; 40];
    req[0..4].copy_from_slice(&USBUSER_GET_CONTROLLER_INFO_0.to_le_bytes());
    req[8..12].copy_from_slice(&40u32.to_le_bytes());
    if let Ok(b) = ioctl(&h, IOCTL_USB_USER_REQUEST, &req, 40) {
        if b.len() >= 40 && u32_at(&b, 4) == 0 {
            c.pci_vendor = Some(u32_at(&b, 16) as u16);
            c.pci_device = Some(u32_at(&b, 20) as u16);
            c.pci_revision = Some(u32_at(&b, 24) as u8);
            c.num_root_ports = u32_at(&b, 28);
            c.flavor = flavor_name(u32_at(&b, 32));
        }
    }
    match ioctl_name(&h, IOCTL_USB_GET_ROOT_HUB_NAME, &[], 0, 4) {
        Some(name) => {
            let mut hub = hub(ctx, &name, true);
            hub.info = root_hub_info(devinst);
            c.root_hub = Some(hub);
        }
        None => c.error = Some("Root hub name not available".into()),
    }
    c
}

/// The root hub is the only child devnode of the controller.
fn root_hub_info(ctrl_devinst: u32) -> Option<DevInfo> {
    devnode::children(ctrl_devinst).first().map(|&dn| devnode::read(dn, 0))
}

fn parse_pci_ids(c: &mut Controller) {
    let id = c.info.instance_id.to_uppercase();
    let grab = |tag: &str, n: usize| -> Option<u32> {
        let i = id.find(tag)? + tag.len();
        u32::from_str_radix(id.get(i..i + n)?, 16).ok()
    };
    c.pci_vendor = grab("VEN_", 4).map(|v| v as u16);
    c.pci_device = grab("DEV_", 4).map(|v| v as u16);
    c.pci_revision = grab("REV_", 2).map(|v| v as u8);
}

fn hub(ctx: &Ctx, name: &str, is_root: bool) -> Hub {
    let mut hub = Hub { symbolic_name: name.to_string(), is_root, ..Default::default() };
    let h = match open_device(&format!(r"\\.\{name}")) {
        Ok(h) => h,
        Err(e) => {
            hub.error = Some(format!("Cannot open hub: {e}"));
            return hub;
        }
    };
    // USB_NODE_INFORMATION (76 bytes)
    let mut nports = 0u32;
    if let Ok(b) = ioctl(&h, IOCTL_USB_GET_NODE_INFORMATION, &[0u8; 76], 76) {
        if b.len() >= 76 {
            nports = b[6] as u32;
            hub.bus_powered = b[75] != 0;
            let dlen = (b[4] as usize).clamp(7, 71);
            hub.descriptor = b[4..4 + dlen].to_vec();
        }
    }
    // USB_HUB_INFORMATION_EX (77 bytes) — preferred on Windows 8+
    if let Ok(b) = ioctl(&h, IOCTL_USB_GET_HUB_INFORMATION_EX, &[], 77) {
        if b.len() >= 8 {
            hub.hub_type = match u32_at(&b, 0) {
                1 => "Root Hub",
                2 => "USB 2.0 Hub",
                3 => "USB 3.x Hub",
                _ => "Hub",
            }
            .into();
            let highest = u16_at(&b, 4) as u32;
            if highest > 0 {
                nports = highest;
            }
            let dlen = b[6] as usize;
            if dlen >= 7 && 6 + dlen <= b.len() {
                hub.descriptor = b[6..6 + dlen].to_vec();
            }
        }
    }
    if let Ok(b) = ioctl(&h, IOCTL_USB_GET_HUB_CAPABILITIES_EX, &[], 4) {
        let f = u32_at(&b, 0);
        hub.high_speed_capable = f & 1 != 0;
        hub.multi_tt = f & 8 != 0;
        if f & 0x40 != 0 {
            hub.bus_powered = true;
        }
    }
    hub.num_ports = nports;
    for idx in 1..=nports {
        hub.ports.push(port(ctx, &h, idx));
    }
    hub
}

fn port(ctx: &Ctx, h: &Handle, idx: u32) -> Port {
    let mut p = Port { index: idx, ..Default::default() };
    let ix = idx.to_le_bytes();

    // Connector properties (companion ports, Type-C, user connectable)
    if let Ok(b) = ioctl(h, IOCTL_USB_GET_PORT_CONNECTOR_PROPERTIES, &ix, 18) {
        if b.len() >= 16 {
            let actual = u32_at(&b, 4) as usize;
            let props = u32_at(&b, 8);
            let mut conn = PortConnector {
                companion_index: u16_at(&b, 12),
                companion_port: u16_at(&b, 14),
                user_connectable: props & 1 != 0,
                debug_capable: props & 2 != 0,
                multi_companions: props & 4 != 0,
                type_c: props & 8 != 0,
                ..Default::default()
            };
            if actual > 18 {
                if let Ok(full) = ioctl(h, IOCTL_USB_GET_PORT_CONNECTOR_PROPERTIES, &ix, actual) {
                    conn.companion_hub = wide_at(&full, 16);
                }
            }
            p.connector = Some(conn);
        }
    }

    // V2: supported protocols and SuperSpeed(Plus) flags
    let mut v2 = [0u8; 16];
    v2[0..4].copy_from_slice(&ix);
    v2[4..8].copy_from_slice(&16u32.to_le_bytes());
    v2[8..12].copy_from_slice(&7u32.to_le_bytes());
    let mut flags = 0u32;
    if let Ok(b) = ioctl(h, IOCTL_USB_GET_NODE_CONNECTION_INFORMATION_EX_V2, &v2, 16) {
        if b.len() >= 16 {
            let protos = u32_at(&b, 8);
            p.supports_usb110 = protos & 1 != 0;
            p.supports_usb200 = protos & 2 != 0;
            p.supports_usb300 = protos & 4 != 0;
            flags = u32_at(&b, 12);
        }
    }

    // Connection information (with room for 30 pipes)
    let size = 35 + 30 * 11;
    let b = match ioctl(h, IOCTL_USB_GET_NODE_CONNECTION_INFORMATION_EX, &ix, size) {
        Ok(b) if b.len() >= 35 => b,
        _ => return p,
    };
    p.status = ConnectionStatus::from_raw(u32_at(&b, 31));
    if p.status == ConnectionStatus::NoDevice {
        return p;
    }
    let mut dev = Device {
        device_descriptor: b[4..22].to_vec(),
        current_config: b[22],
        is_hub: b[24] != 0,
        address: u16_at(&b, 25),
        open_pipes: u32_at(&b, 27),
        operating_at_ss_or_higher: flags & 1 != 0,
        ss_capable: flags & 2 != 0,
        operating_at_ssp: flags & 4 != 0,
        ssp_capable: flags & 8 != 0,
        ..Default::default()
    };
    dev.speed = match b[23] {
        0 => Speed::Low,
        1 => Speed::Full,
        2 => Speed::High,
        3 => Speed::Super,
        _ => Speed::Unknown,
    };
    if dev.operating_at_ssp {
        dev.speed = Speed::SuperPlus;
    } else if dev.operating_at_ss_or_higher && dev.speed < Speed::Super {
        dev.speed = Speed::Super;
    }
    for i in 0..(dev.open_pipes as usize).min(30) {
        let o = 35 + i * 11;
        if o + 11 > b.len() {
            break;
        }
        dev.pipes.push(PipeInfo {
            endpoint_address: b[o + 2],
            attributes: b[o + 3],
            max_packet: u16_at(&b, o + 4),
            interval: b[o + 6],
            schedule_offset: u32_at(&b, o + 7),
        });
    }

    // Driver key -> devnode
    let mut key_in = [0u8; 10];
    key_in[0..4].copy_from_slice(&ix);
    if let Some(key) = ioctl_name(h, IOCTL_USB_GET_NODE_CONNECTION_DRIVERKEY_NAME, &key_in, 4, 8) {
        // A hub's child devnodes are the downstream devices, shown separately.
        dev.info = ctx.devinfo_for_key(&key, if dev.is_hub { 0 } else { 3 });
        dev.driver_key = key;
    }

    if p.status == ConnectionStatus::Connected {
        read_descriptors(h, idx, &mut dev);
    }

    if dev.is_hub {
        let mut name_in = [0u8; 10];
        name_in[0..4].copy_from_slice(&ix);
        if let Some(name) = ioctl_name(h, IOCTL_USB_GET_NODE_CONNECTION_NAME, &name_in, 4, 8) {
            let mut sub = hub(ctx, &name, false);
            if sub.hub_type.is_empty() {
                sub.hub_type = "Hub".into();
            }
            dev.hub = Some(Box::new(sub));
        }
    }
    p.device = Some(dev);
    p
}

/// GET_DESCRIPTOR via the hub driver. Returns the descriptor bytes.
fn get_descriptor(h: &Handle, port: u32, dtype: u8, index: u8, lang: u16, len: u16) -> Option<Vec<u8>> {
    let mut req = vec![0u8; 12];
    req[0..4].copy_from_slice(&port.to_le_bytes());
    req[6..8].copy_from_slice(&(((dtype as u16) << 8) | index as u16).to_le_bytes());
    req[8..10].copy_from_slice(&lang.to_le_bytes());
    req[10..12].copy_from_slice(&len.to_le_bytes());
    let b = ioctl(h, IOCTL_USB_GET_DESCRIPTOR_FROM_NODE_CONNECTION, &req, 12 + len as usize).ok()?;
    if b.len() <= 12 {
        return None;
    }
    Some(b[12..].to_vec())
}

fn read_descriptors(h: &Handle, port: u32, dev: &mut Device) {
    let Some(dd) = DeviceDescriptor::from_bytes(&dev.device_descriptor) else { return };

    // Configuration descriptor (active one, index 0 is the only one in practice)
    if let Some(head) = get_descriptor(h, port, 2, 0, 0, 9) {
        if head.len() >= 4 {
            let total = u16::from_le_bytes([head[2], head[3]]).max(9);
            if let Some(full) = get_descriptor(h, port, 2, 0, 0, total) {
                dev.config_descriptor = full;
            }
        }
    }
    // BOS (USB 2.01+)
    if dd.bcd_usb > 0x0200 {
        if let Some(head) = get_descriptor(h, port, 0x0F, 0, 0, 5) {
            if head.len() >= 4 && head[1] == 0x0F {
                let total = u16::from_le_bytes([head[2], head[3]]).max(5);
                if let Some(full) = get_descriptor(h, port, 0x0F, 0, 0, total) {
                    dev.bos_descriptor = full;
                }
            }
        }
    }
    // Device qualifier (high-speed capable USB 2.0 devices)
    if dd.bcd_usb == 0x0200 && dev.speed == Speed::High {
        if let Some(q) = get_descriptor(h, port, 0x06, 0, 0, 10) {
            if q.len() >= 10 && q[1] == 0x06 {
                dev.device_qualifier = q;
            }
        }
    }

    // Strings: collect every string index referenced by the descriptors.
    let mut indexes = vec![dd.i_manufacturer, dd.i_product, dd.i_serial];
    let c = &dev.config_descriptor;
    let mut pos = 0;
    while pos + 2 <= c.len() {
        let len = c[pos] as usize;
        if len < 2 {
            break;
        }
        match c[pos + 1] {
            0x02 if pos + 6 < c.len() => indexes.push(c[pos + 6]),
            0x04 if pos + 8 < c.len() => indexes.push(c[pos + 8]),
            0x0B if pos + 7 < c.len() => indexes.push(c[pos + 7]),
            _ => {}
        }
        pos += len;
    }
    indexes.retain(|&i| i != 0);
    indexes.sort_unstable();
    indexes.dedup();
    if indexes.is_empty() {
        return;
    }
    let Some(l0) = get_descriptor(h, port, 3, 0, 0, 255) else { return };
    dev.lang_ids = decode_langids(&l0);
    for &lang in dev.lang_ids.iter().take(4) {
        for &i in &indexes {
            if let Some(s) = get_descriptor(h, port, 3, i, lang, 255).and_then(|d| decode_string(&d)) {
                dev.strings.push(StringDesc { index: i, lang, text: s });
            }
        }
    }
}
