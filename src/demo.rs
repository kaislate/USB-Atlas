//! A realistic synthetic topology used by tests and the in-app demo mode.

use crate::descriptors::DeviceDescriptor;
use crate::model::*;

fn strings(items: &[(u8, &str)]) -> Vec<StringDesc> {
    items.iter().map(|(i, t)| StringDesc { index: *i, lang: 0x0409, text: t.to_string() }).collect()
}

fn config(body: &[&[u8]], attrs: u8, max_power: u8, n_if: u8) -> Vec<u8> {
    let mut c = vec![0x09, 0x02, 0, 0, n_if, 0x01, 0x00, attrs, max_power];
    for b in body {
        c.extend_from_slice(b);
    }
    let t = c.len() as u16;
    c[2..4].copy_from_slice(&t.to_le_bytes());
    c
}

struct D<'a> {
    vid: u16,
    pid: u16,
    bcd_usb: u16,
    class: (u8, u8, u8),
    speed: Speed,
    product: &'a str,
    mfr: &'a str,
    serial: Option<&'a str>,
    cfg: Vec<u8>,
}

fn device(d: D, info: DevInfo) -> Device {
    let desc = DeviceDescriptor {
        bcd_usb: d.bcd_usb,
        class: d.class.0,
        subclass: d.class.1,
        protocol: d.class.2,
        max_packet0: if d.speed.is_super() { 9 } else { 64 },
        vid: d.vid,
        pid: d.pid,
        bcd_device: 0x0100,
        i_manufacturer: 1,
        i_product: 2,
        i_serial: if d.serial.is_some() { 3 } else { 0 },
        num_configurations: 1,
    };
    let mut s = vec![(1, d.mfr), (2, d.product)];
    if let Some(sn) = d.serial {
        s.push((3, sn));
    }
    let ss = d.speed.is_super();
    Device {
        address: 1,
        speed: d.speed,
        is_hub: d.class.0 == 0x09,
        current_config: 1,
        open_pipes: 2,
        device_descriptor: desc.to_bytes().to_vec(),
        config_descriptor: d.cfg,
        bos_descriptor: if d.bcd_usb >= 0x0210 {
            let mut b = vec![0x05, 0x0F, 0x16, 0x00, 0x02];
            b.extend([0x07, 0x10, 0x02, 0x06, 0x00, 0x00, 0x00]);
            b.extend([0x0A, 0x10, 0x03, 0x00, 0x0E, 0x00, 0x01, 0x0A, 0xFF, 0x07]);
            b
        } else {
            vec![]
        },
        device_qualifier: vec![],
        lang_ids: vec![0x0409],
        strings: strings(&s),
        driver_key: String::new(),
        pipes: vec![],
        operating_at_ss_or_higher: ss,
        ss_capable: d.bcd_usb >= 0x0300,
        operating_at_ssp: d.speed >= Speed::SuperPlus,
        ssp_capable: false,
        info: Some(info),
        hub: None,
    }
}

fn info(id: &str, desc: &str, class: &str, service: &str) -> DevInfo {
    DevInfo {
        instance_id: id.into(),
        description: desc.into(),
        class: class.into(),
        service: service.into(),
        manufacturer: "(Standard USB Host Controller)".into(),
        driver_provider: "Microsoft".into(),
        driver_version: "10.0.26100.1".into(),
        driver_date: "2006-06-21".into(),
        location_info: String::new(),
        ..Default::default()
    }
}

fn port(index: u32, usb3: bool, device: Option<Device>) -> Port {
    Port {
        index,
        status: if device.is_some() { ConnectionStatus::Connected } else { ConnectionStatus::NoDevice },
        connector: Some(PortConnector {
            companion_index: 0,
            companion_port: if usb3 { index + 10 } else { 0 } as u16,
            companion_hub: String::new(),
            user_connectable: true,
            debug_capable: false,
            multi_companions: false,
            type_c: false,
        }),
        supports_usb110: true,
        supports_usb200: true,
        supports_usb300: usb3,
        device,
    }
}

const HID_MOUSE_IF: &[u8] = &[
    0x09, 0x04, 0x00, 0x00, 0x01, 0x03, 0x01, 0x02, 0x00, 0x09, 0x21, 0x11, 0x01, 0x00, 0x01, 0x22, 0x4A, 0x00, 0x07, 0x05, 0x81,
    0x03, 0x08, 0x00, 0x01,
];
const HID_KBD_IF: &[u8] = &[
    0x09, 0x04, 0x01, 0x00, 0x01, 0x03, 0x01, 0x01, 0x00, 0x09, 0x21, 0x11, 0x01, 0x00, 0x01, 0x22, 0x3B, 0x00, 0x07, 0x05, 0x82,
    0x03, 0x08, 0x00, 0x08,
];
const MSC_IF: &[u8] = &[
    0x09, 0x04, 0x00, 0x00, 0x02, 0x08, 0x06, 0x50, 0x00, 0x07, 0x05, 0x81, 0x02, 0x00, 0x02, 0x00, 0x07, 0x05, 0x02, 0x02, 0x00,
    0x02, 0x00,
];
const UVC_IF: &[u8] = &[
    0x08, 0x0B, 0x00, 0x02, 0x0E, 0x03, 0x00, 0x02, // IAD
    0x09, 0x04, 0x00, 0x00, 0x01, 0x0E, 0x01, 0x00, 0x02, // VC
    0x0D, 0x24, 0x01, 0x00, 0x01, 0x4D, 0x00, 0x80, 0x8D, 0x5B, 0x00, 0x01, 0x01, // VC header
    0x07, 0x05, 0x83, 0x03, 0x10, 0x00, 0x08, // int ep
    0x09, 0x04, 0x01, 0x00, 0x00, 0x0E, 0x02, 0x00, 0x00, // VS alt0
    0x0B, 0x24, 0x06, 0x01, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, // MJPEG format
    0x1E, 0x24, 0x07, 0x01, 0x00, 0x80, 0x07, 0x38, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x15, 0x16, 0x05, 0x00, 0x01, 0x15, 0x16, 0x05, 0x00, // frame 1920x1080
    0x09, 0x04, 0x01, 0x01, 0x01, 0x0E, 0x02, 0x00, 0x00, // VS alt1
    0x07, 0x05, 0x81, 0x05, 0x00, 0x14, 0x01, // iso ep
    0x08, 0x0B, 0x02, 0x02, 0x01, 0x02, 0x00, 0x03, // audio IAD
    0x09, 0x04, 0x02, 0x00, 0x00, 0x01, 0x01, 0x00, 0x03, 0x09, 0x24, 0x01, 0x00, 0x01, 0x26, 0x00, 0x01, 0x03, 0x0C, 0x24, 0x02,
    0x01, 0x01, 0x02, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x09, 0x04, 0x03, 0x00, 0x00, 0x01, 0x02, 0x00, 0x00,
];
const CDC_IF: &[u8] = &[
    0x08, 0x0B, 0x00, 0x02, 0x02, 0x02, 0x01, 0x00, // IAD
    0x09, 0x04, 0x00, 0x00, 0x01, 0x02, 0x02, 0x01, 0x00, 0x05, 0x24, 0x00, 0x10, 0x01, 0x05, 0x24, 0x01, 0x00, 0x01, 0x04, 0x24,
    0x02, 0x02, 0x05, 0x24, 0x06, 0x00, 0x01, 0x07, 0x05, 0x83, 0x03, 0x08, 0x00, 0x10, 0x09, 0x04, 0x01, 0x00, 0x02, 0x0A, 0x00,
    0x00, 0x00, 0x07, 0x05, 0x01, 0x02, 0x40, 0x00, 0x00, 0x07, 0x05, 0x82, 0x02, 0x40, 0x00, 0x00,
];

pub fn snapshot() -> Snapshot {
    let mut receiver_info = info(r"USB\VID_046D&PID_C52B\5&2A1B3C4D&0&1", "USB Composite Device", "USB", "usbccgp");
    receiver_info.children = vec![
        DevInfo { instance_id: r"USB\VID_046D&PID_C52B&MI_00\6&1".into(), description: "USB Input Device".into(), class: "HIDClass".into(), service: "HidUsb".into(), ..Default::default() },
        DevInfo { instance_id: r"USB\VID_046D&PID_C52B&MI_01\6&2".into(), description: "USB Input Device".into(), class: "HIDClass".into(), service: "HidUsb".into(), ..Default::default() },
    ];
    let receiver = device(
        D {
            vid: 0x046D,
            pid: 0xC52B,
            bcd_usb: 0x0200,
            class: (0, 0, 0),
            speed: Speed::Full,
            product: "USB Receiver",
            mfr: "Logitech",
            serial: None,
            cfg: config(&[HID_MOUSE_IF, HID_KBD_IF], 0xA0, 0x31, 2),
        },
        receiver_info,
    );

    let mut flash_info = info(r"USB\VID_0781&PID_5583\4C530001230416115024", "USB Mass Storage Device", "USB", "USBSTOR");
    flash_info.children = vec![DevInfo {
        instance_id: r"USBSTOR\DISK&VEN_SANDISK&PROD_ULTRA_FIT&REV_1.00\4C530001230416115024&0".into(),
        friendly_name: "SanDisk Ultra Fit USB Device".into(),
        class: "DiskDrive".into(),
        service: "disk".into(),
        volumes: vec![Volume {
            mount_points: vec!["E:\\".into()],
            label: "SANDISK".into(),
            file_system: "exFAT".into(),
            total_bytes: 123_010_547_712,
            free_bytes: 71_334_182_912,
            volume_name: r"\\?\Volume{8e1c2f43-0000-0000-0000-100000000000}\".into(),
        }],
        ..Default::default()
    }];
    let flash = device(
        D {
            vid: 0x0781,
            pid: 0x5583,
            bcd_usb: 0x0320,
            class: (0, 0, 0),
            speed: Speed::High,
            product: "Ultra Fit",
            mfr: "SanDisk",
            serial: Some("4C530001230416115024"),
            cfg: config(&[MSC_IF], 0x80, 0x70, 1),
        },
        flash_info,
    );

    let mut cam_info = info(r"USB\VID_046D&PID_085E\B5F2E1A0", "USB Composite Device", "USB", "usbccgp");
    cam_info.children = vec![
        DevInfo { instance_id: r"USB\VID_046D&PID_085E&MI_00\7&1".into(), friendly_name: "Logitech BRIO".into(), class: "Camera".into(), service: "usbvideo".into(), ..Default::default() },
        DevInfo { instance_id: r"USB\VID_046D&PID_085E&MI_02\7&2".into(), friendly_name: "Microphone (Logitech BRIO)".into(), class: "MEDIA".into(), service: "usbaudio".into(), ..Default::default() },
    ];
    let cam = device(
        D {
            vid: 0x046D,
            pid: 0x085E,
            bcd_usb: 0x0310,
            class: (0xEF, 0x02, 0x01),
            speed: Speed::Super,
            product: "Logitech BRIO",
            mfr: "Logitech",
            serial: Some("B5F2E1A0"),
            cfg: config(&[UVC_IF], 0x80, 0x3F, 4),
        },
        cam_info,
    );

    let mut serial_info = info(r"USB\VID_2E8A&PID_000A\E6614C311B7A8B2F", "USB Serial Device", "Ports", "usbser");
    serial_info.com_port = "COM7".into();
    serial_info.friendly_name = "USB Serial Device (COM7)".into();
    let pico = device(
        D {
            vid: 0x2E8A,
            pid: 0x000A,
            bcd_usb: 0x0200,
            class: (0xEF, 0x02, 0x01),
            speed: Speed::Full,
            product: "Pico",
            mfr: "Raspberry Pi",
            serial: Some("E6614C311B7A8B2F"),
            cfg: config(&[CDC_IF], 0x80, 0xFA, 2),
        },
        serial_info,
    );

    let mut hub_dev = device(
        D {
            vid: 0x2109,
            pid: 0x0817,
            bcd_usb: 0x0320,
            class: (0x09, 0x00, 0x03),
            speed: Speed::Super,
            product: "USB3.0 Hub",
            mfr: "VIA Labs, Inc.",
            serial: None,
            cfg: config(&[&[0x09, 0x04, 0x00, 0x00, 0x01, 0x09, 0x00, 0x00, 0x00, 0x07, 0x05, 0x81, 0x03, 0x02, 0x00, 0x08, 0x06, 0x30, 0x00, 0x00, 0x02, 0x00]], 0xE0, 0x00, 1),
        },
        info(r"USB\VID_2109&PID_0817\5&11", "Generic SuperSpeed USB Hub", "USB", "USBHUB3"),
    );
    hub_dev.hub = Some(Box::new(Hub {
        symbolic_name: "USB#VID_2109&PID_0817#5&11#{f18a0e88-c30c-11d0-8815-00a0c906bed8}".into(),
        is_root: false,
        hub_type: "USB 3.0 Hub".into(),
        descriptor: vec![0x0C, 0x2A, 0x04, 0x09, 0x00, 0x32, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        num_ports: 4,
        bus_powered: false,
        high_speed_capable: true,
        multi_tt: false,
        info: None,
        ports: vec![port(1, true, Some(cam)), port(2, true, Some(pico)), port(3, true, None), port(4, true, None)],
        error: None,
    }));

    let mut bad_info = info(r"USB\VID_0000&PID_0002\5&3", "Unknown USB Device (Device Descriptor Request Failed)", "USB", "");
    bad_info.problem_code = 43;
    let mut bad = Device {
        speed: Speed::Full,
        device_descriptor: vec![0x12, 0x01, 0x00, 0x00, 0, 0, 0, 0, 0, 0, 0x02, 0, 0, 0, 0, 0, 0, 0],
        info: Some(bad_info),
        ..Default::default()
    };
    bad.address = 0;

    let mut oc = port(8, false, None);
    oc.status = ConnectionStatus::Overcurrent;
    let mut typec = port(9, true, None);
    typec.connector.as_mut().unwrap().type_c = true;
    let mut internal = port(10, false, None);
    internal.connector.as_mut().unwrap().user_connectable = false;
    let bt_info = info(r"USB\VID_8087&PID_0033\5&4", "Intel(R) Wireless Bluetooth(R)", "Bluetooth", "BTHUSB");
    internal.device = Some(device(
        D {
            vid: 0x8087,
            pid: 0x0033,
            bcd_usb: 0x0200,
            class: (0xE0, 0x01, 0x01),
            speed: Speed::Full,
            product: "",
            mfr: "",
            serial: None,
            cfg: config(&[&[0x09, 0x04, 0x00, 0x00, 0x01, 0xE0, 0x01, 0x01, 0x00, 0x07, 0x05, 0x81, 0x03, 0x40, 0x00, 0x01]], 0xE0, 0x32, 1),
        },
        bt_info,
    ));
    internal.status = ConnectionStatus::Connected;

    let mut bad_port = port(7, false, Some(bad));
    bad_port.status = ConnectionStatus::Connected;

    let root = Hub {
        symbolic_name: "USB#ROOT_HUB30#4&1&0#{f18a0e88-c30c-11d0-8815-00a0c906bed8}".into(),
        is_root: true,
        hub_type: "USB 3.0 Root Hub".into(),
        descriptor: vec![0x0C, 0x2A, 0x0A, 0x09, 0x00, 0x0A, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        num_ports: 10,
        bus_powered: false,
        high_speed_capable: true,
        multi_tt: false,
        info: Some(info(r"USB\ROOT_HUB30\4&1&0&0", "USB Root Hub (USB 3.0)", "USB", "USBHUB3")),
        ports: vec![
            port(1, false, Some(receiver)),
            port(2, true, None),
            port(3, true, Some(flash)),
            port(4, true, None),
            port(5, true, Some(hub_dev)),
            port(6, false, None),
            bad_port,
            oc,
            typec,
            internal,
        ],
        error: None,
    };

    let mut ctrl_info = info(r"PCI\VEN_8086&DEV_7AE0&SUBSYS_7D251462&REV_11\3&11583659&0&A0", "Intel(R) USB 3.20 eXtensible Host Controller - 1.20 (Microsoft)", "USB", "USBXHCI");
    ctrl_info.location_info = "PCI bus 0, device 20, function 0".into();

    let root2 = Hub {
        symbolic_name: "USB#ROOT_HUB30#4&2&0#{f18a0e88-c30c-11d0-8815-00a0c906bed8}".into(),
        is_root: true,
        hub_type: "USB 3.0 Root Hub".into(),
        num_ports: 4,
        info: Some(info(r"USB\ROOT_HUB30\4&2&0&0", "USB Root Hub (USB 3.0)", "USB", "USBHUB3")),
        ports: vec![port(1, true, None), port(2, true, None), port(3, false, None), port(4, false, None)],
        ..Default::default()
    };

    Snapshot {
        format: Snapshot::FORMAT,
        app_version: env!("CARGO_PKG_VERSION").into(),
        taken_at: "demo".into(),
        computer: ComputerInfo { name: "DEMO-PC".into(), os: "Windows 11 Pro 24H2 (demo data)".into(), user: "demo".into(), is_admin: false },
        controllers: vec![
            Controller {
                info: ctrl_info,
                pci_vendor: Some(0x8086),
                pci_device: Some(0x7AE0),
                pci_revision: Some(0x11),
                flavor: "USB 3 xHCI".into(),
                num_root_ports: 10,
                root_hub: Some(root),
                error: None,
            },
            Controller {
                info: info(r"PCI\VEN_1022&DEV_15B6&SUBSYS_7D251462&REV_00\4&2", "AMD USB 3.10 eXtensible Host Controller - 1.10 (Microsoft)", "USB", "USBXHCI"),
                pci_vendor: Some(0x1022),
                pci_device: Some(0x15B6),
                pci_revision: Some(0),
                flavor: "USB 3 xHCI".into(),
                num_root_ports: 4,
                root_hub: Some(root2),
                error: None,
            },
        ],
        scan_ms: 42,
    }
}
