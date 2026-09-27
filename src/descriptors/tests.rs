use super::*;

fn no_strings(_: u8) -> Option<String> {
    None
}

// Logitech USB receiver style device descriptor.
const DEVICE: [u8; 18] = [
    0x12, 0x01, 0x00, 0x02, 0x00, 0x00, 0x00, 0x08, 0x6D, 0x04, 0x2B, 0xC5, 0x03, 0x12, 0x01, 0x02,
    0x00, 0x01,
];

#[test]
fn device_descriptor_roundtrip() {
    let d = DeviceDescriptor::from_bytes(&DEVICE).unwrap();
    assert_eq!(d.vid, 0x046D);
    assert_eq!(d.pid, 0xC52B);
    assert_eq!(d.bcd_usb, 0x0200);
    assert_eq!(d.to_bytes(), DEVICE);
}

#[test]
fn decode_device_fields_have_offsets() {
    let lookup = |i: u8| {
        if i == 2 {
            Some("USB Receiver".to_string())
        } else {
            None
        }
    };
    let n = decode_device(&DEVICE, &lookup);
    let vid = n.field("idVendor").unwrap();
    assert_eq!((vid.offset, vid.size, vid.value), (8, 2, 0x046D));
    assert_eq!(n.field("bcdUSB").unwrap().display, "0x0200 (USB 2.00)");
    assert_eq!(
        n.field("iProduct").unwrap().display,
        "0x02 (\"USB Receiver\")"
    );
    assert!(n.warnings.is_empty());
}

#[test]
fn truncated_device_descriptor_warns() {
    let n = decode_device(&DEVICE[..10], &no_strings);
    assert!(n.warnings.iter().any(|w| w.contains("truncated")));
}

// Config: 1 IAD grouping two interfaces (CDC ACM), plus a HID interface.
fn composite_config() -> Vec<u8> {
    let mut c = vec![0x09, 0x02, 0, 0, 0x03, 0x01, 0x00, 0xA0, 0x32];
    c.extend([0x08, 0x0B, 0x00, 0x02, 0x02, 0x02, 0x01, 0x00]); // IAD if0..1
    c.extend([0x09, 0x04, 0x00, 0x00, 0x01, 0x02, 0x02, 0x01, 0x00]); // if0 CDC
    c.extend([0x05, 0x24, 0x00, 0x10, 0x01]); // CDC header
    c.extend([0x07, 0x05, 0x81, 0x03, 0x08, 0x00, 0x10]); // ep 0x81 int
    c.extend([0x09, 0x04, 0x01, 0x00, 0x02, 0x0A, 0x00, 0x00, 0x00]); // if1 CDC data
    c.extend([0x07, 0x05, 0x02, 0x02, 0x40, 0x00, 0x00]);
    c.extend([0x07, 0x05, 0x82, 0x02, 0x40, 0x00, 0x00]);
    c.extend([0x09, 0x04, 0x02, 0x00, 0x01, 0x03, 0x01, 0x02, 0x00]); // if2 HID mouse
    c.extend([0x09, 0x21, 0x11, 0x01, 0x00, 0x01, 0x22, 0x34, 0x00]);
    c.extend([0x07, 0x05, 0x83, 0x03, 0x04, 0x00, 0x0A]);
    let total = c.len() as u16;
    c[2..4].copy_from_slice(&total.to_le_bytes());
    c
}

#[test]
fn configuration_nests_iad_interfaces_endpoints() {
    let c = composite_config();
    let n = decode_configuration(&c, &no_strings, false);
    assert!(n.warnings.is_empty(), "{:?}", n.warnings);
    assert_eq!(n.field("MaxPower").unwrap().display, "0x32 (100 mA)");
    assert_eq!(n.children.len(), 2, "IAD + HID interface at top level");
    let iad = &n.children[0];
    assert_eq!(iad.kind, DescKind::InterfaceAssociation);
    assert_eq!(iad.children.len(), 2);
    assert_eq!(iad.children[0].children.len(), 2, "CDC header + endpoint");
    assert_eq!(iad.children[1].children.len(), 2);
    let hid = &n.children[1];
    assert_eq!(hid.kind, DescKind::Interface);
    assert!(hid.title.contains("HID"));
    assert_eq!(hid.children[0].kind, DescKind::Hid);
    assert_eq!(hid.children[1].title, "Endpoint 0x83 IN Interrupt");
    // offsets are absolute
    assert_eq!(hid.children[1].offset, c.len() - 7);
}

#[test]
fn super_speed_max_power_uses_8ma_units() {
    let c = [0x09, 0x02, 0x09, 0x00, 0x00, 0x01, 0x00, 0x80, 0x70];
    let n = decode_configuration(&c, &no_strings, true);
    assert_eq!(n.field("MaxPower").unwrap().display, "0x70 (896 mA)");
}

#[test]
fn wrong_total_length_warns() {
    let mut c = composite_config();
    c[2] = 0xFF;
    let n = decode_configuration(&c, &no_strings, false);
    assert!(n.warnings.iter().any(|w| w.contains("wTotalLength")));
}

#[test]
fn zero_length_descriptor_does_not_loop() {
    let c = [
        0x09, 0x02, 0x0B, 0x00, 0x00, 0x01, 0x00, 0x80, 0x32, 0x00, 0x04,
    ];
    let n = decode_configuration(&c, &no_strings, false);
    assert!(n
        .warnings
        .iter()
        .any(|w| w.contains("Invalid descriptor length")));
}

#[test]
fn ss_companion_attaches_to_endpoint() {
    let mut c = vec![0x09, 0x02, 0, 0, 0x01, 0x01, 0x00, 0x80, 0x70];
    c.extend([0x09, 0x04, 0x00, 0x00, 0x01, 0x08, 0x06, 0x50, 0x00]);
    c.extend([0x07, 0x05, 0x81, 0x02, 0x00, 0x04, 0x00]);
    c.extend([0x06, 0x30, 0x0F, 0x00, 0x00, 0x00]);
    let total = c.len() as u16;
    c[2..4].copy_from_slice(&total.to_le_bytes());
    let n = decode_configuration(&c, &no_strings, true);
    let ep = &n.children[0].children[0];
    assert_eq!(ep.kind, DescKind::Endpoint);
    assert_eq!(ep.children[0].kind, DescKind::SsEndpointCompanion);
    assert_eq!(
        ep.children[0].field("bMaxBurst").unwrap().display,
        "15 (16 packets per burst)"
    );
}

#[test]
fn bos_with_usb2ext_ss_and_container_id() {
    let mut b = vec![0x05, 0x0F, 0, 0, 0x03];
    b.extend([0x07, 0x10, 0x02, 0x06, 0x00, 0x00, 0x00]);
    b.extend([0x0A, 0x10, 0x03, 0x00, 0x0E, 0x00, 0x01, 0x0A, 0xFF, 0x07]);
    let mut cid = vec![0x14, 0x10, 0x04, 0x00];
    cid.extend(0u8..16);
    b.extend(cid);
    let total = b.len() as u16;
    b[2..4].copy_from_slice(&total.to_le_bytes());
    let n = decode_bos(&b);
    assert!(n.warnings.is_empty(), "{:?}", n.warnings);
    assert_eq!(n.children.len(), 3);
    assert_eq!(n.children[0].title, "USB 2.0 Extension");
    assert!(n.children[0]
        .field("bmAttributes")
        .unwrap()
        .display
        .contains("LPM"));
    assert!(n.children[1]
        .field("wSpeedsSupported")
        .unwrap()
        .display
        .contains("5 Gbps"));
    assert_eq!(
        n.children[2].field("ContainerID").unwrap().display,
        "{03020100-0504-0706-0809-0A0B0C0D0E0F}"
    );
}

#[test]
fn strings_and_langids() {
    let s = [0x0A, 0x03, b'T', 0, b'e', 0, b's', 0, b't', 0];
    assert_eq!(decode_string(&s).as_deref(), Some("Test"));
    assert_eq!(decode_langids(&[0x04, 0x03, 0x09, 0x04]), vec![0x0409]);
    assert_eq!(decode_string(&[0x02, 0x01]), None);
}

#[test]
fn hub_descriptor() {
    let h = [0x09, 0x29, 0x04, 0xE9, 0x00, 0x32, 0x64, 0x00, 0xFF];
    let n = decode_hub(&h);
    assert_eq!(n.field("bNumberOfPorts").unwrap().value, 4);
    assert_eq!(n.field("bPwrOn2PwrGood").unwrap().display, "50 (100 ms)");
    assert!(n
        .field("wHubCharacteristics")
        .unwrap()
        .display
        .contains("individual port power"));
}
