//! Binary Object Store (BOS) and device capability descriptors.

use super::{bcd, header, hex_bytes, uuid, DescKind, DescNode, Reader};

const WEBUSB_UUID: &str = "{3408B638-09A9-47A0-8BFD-A0768815B665}";
const MSOS20_UUID: &str = "{D8DD60DF-4589-4CC7-9CD2-659D9E648A9F}";

pub fn capability_name(t: u8) -> &'static str {
    match t {
        0x01 => "Wireless USB",
        0x02 => "USB 2.0 Extension",
        0x03 => "SuperSpeed USB",
        0x04 => "Container ID",
        0x05 => "Platform",
        0x06 => "Power Delivery",
        0x07 => "Battery Info",
        0x08 => "PD Consumer Port",
        0x09 => "PD Provider Port",
        0x0A => "SuperSpeedPlus",
        0x0B => "Precision Time Measurement",
        0x0C => "Wireless USB Ext",
        0x0D => "Billboard",
        0x0E => "Authentication",
        0x0F => "Billboard Ex",
        0x10 => "Configuration Summary",
        0x11 => "FWStatus",
        _ => "Unknown capability",
    }
}

pub fn decode_bos(b: &[u8]) -> DescNode {
    let hlen = (b.first().copied().unwrap_or(5) as usize).min(b.len());
    let mut r = Reader::new(&b[..hlen], 0, "BOS Descriptor", DescKind::Bos);
    header(&mut r, 0x0F);
    let total = r.num("wTotalLength", 2, |v| format!("0x{v:04X} ({v} bytes)"));
    r.dec("bNumDeviceCaps", 1);
    let mut node = r.finish();
    if let Some(t) = total {
        if t as usize != b.len() {
            node.warnings.push(format!("wTotalLength is {t} but {} bytes were returned", b.len()));
        }
    }
    let mut pos = hlen.max(2);
    while pos + 3 <= b.len() {
        let len = b[pos] as usize;
        if len < 3 {
            node.warnings.push(format!("Invalid capability length {len} at offset {pos}"));
            break;
        }
        let end = (pos + len).min(b.len());
        node.children.push(decode_capability(&b[pos..end], pos));
        pos += len;
    }
    node
}

fn decode_capability(d: &[u8], base: usize) -> DescNode {
    let cap = d.get(2).copied().unwrap_or(0);
    let mut r = Reader::new(d, base, capability_name(cap), DescKind::DeviceCapability);
    header(&mut r, 0x10);
    r.hex_note("bDevCapabilityType", 1, |v| capability_name(v as u8).to_string());
    match cap {
        0x02 => {
            r.num("bmAttributes", 4, |v| {
                let mut p = Vec::new();
                if v & 0x2 != 0 { p.push("LPM") }
                if v & 0x4 != 0 { p.push("BESL & alternate HIRD") }
                if v & 0x8 != 0 { p.push("baseline BESL valid") }
                if v & 0x10 != 0 { p.push("deep BESL valid") }
                format!("0x{v:08X} ({})", if p.is_empty() { "none".into() } else { p.join(", ") })
            });
        }
        0x03 => {
            r.num("bmAttributes", 1, |v| {
                format!("0x{v:02X} ({})", if v & 2 != 0 { "LTM capable" } else { "no LTM" })
            });
            r.num("wSpeedsSupported", 2, |v| {
                let mut p = Vec::new();
                if v & 1 != 0 { p.push("Low") }
                if v & 2 != 0 { p.push("Full") }
                if v & 4 != 0 { p.push("High") }
                if v & 8 != 0 { p.push("5 Gbps") }
                format!("0x{v:04X} ({})", p.join(", "))
            });
            r.num("bFunctionalitySupport", 1, |v| {
                let s = ["Low Speed", "Full Speed", "High Speed", "SuperSpeed"].get(v as usize).copied().unwrap_or("?");
                format!("0x{v:02X} (lowest fully functional: {s})")
            });
            r.num("bU1DevExitLat", 1, |v| format!("{v} µs"));
            r.num("wU2DevExitLat", 2, |v| format!("{v} µs"));
        }
        0x04 => {
            r.hex("bReserved", 1);
            r.bytes("ContainerID", 16, uuid);
        }
        0x05 => {
            r.hex("bReserved", 1);
            let id = r.bytes("PlatformCapabilityUUID", 16, |u| {
                let s = uuid(u);
                let known = match s.as_str() {
                    WEBUSB_UUID => " – WebUSB",
                    MSOS20_UUID => " – Microsoft OS 2.0",
                    _ => "",
                };
                format!("{s}{known}")
            });
            let id = id.map(uuid).unwrap_or_default();
            if id == WEBUSB_UUID {
                r.num("bcdVersion", 2, |v| format!("0x{v:04X} ({})", bcd(v)));
                r.hex("bVendorCode", 1);
                r.dec("iLandingPage", 1);
                r.node.title = "Platform – WebUSB".into();
            } else if id == MSOS20_UUID {
                r.num("dwWindowsVersion", 4, |v| format!("0x{v:08X}"));
                r.num("wMSOSDescriptorSetTotalLength", 2, |v| format!("{v} bytes"));
                r.hex("bMS_VendorCode", 1);
                r.hex("bAltEnumCode", 1);
                r.node.title = "Platform – Microsoft OS 2.0".into();
            }
        }
        0x0A => {
            r.hex("bReserved", 1);
            let attr = r
                .num("bmAttributes", 4, |v| {
                    format!("0x{v:08X} ({} sublink speed attributes)", (v & 0x1F) + 1)
                })
                .unwrap_or(0);
            r.hex("wFunctionalitySupport", 2);
            r.hex("wReserved", 2);
            let n = (attr & 0x1F) + 1;
            for i in 0..n {
                r.num(&format!("bmSublinkSpeedAttr[{i}]"), 4, |v| {
                    let exp = ["b/s", "Kb/s", "Mb/s", "Gb/s"][((v >> 4) & 3) as usize];
                    let mantissa = v >> 16;
                    let dir = if (v >> 6) & 1 == 1 { "Tx" } else { "Rx" };
                    let mode = if (v >> 7) & 1 == 1 { "asymmetric" } else { "symmetric" };
                    let proto = if (v >> 14) & 3 == 1 { "SuperSpeedPlus" } else { "SuperSpeed" };
                    format!("0x{v:08X} (ID {}, {mantissa} {exp}, {mode} {dir}, {proto})", v & 0xF)
                });
            }
        }
        0x0D => {
            r.dec("iAdditionalInfoURL", 1);
            r.dec("bNumberOfAlternateOrUSB4Modes", 1);
            r.dec("bPreferredAlternateOrUSB4Mode", 1);
            r.hex("VCONNPower", 2);
            r.bytes("bmConfigured", 32, hex_bytes);
            r.num("bcdVersion", 2, |v| format!("0x{v:04X} ({})", bcd(v)));
            r.hex("bAdditionalFailureInfo", 1);
            r.hex("bReserved", 1);
        }
        _ => {}
    }
    r.rest("data");
    r.finish()
}
