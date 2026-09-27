//! Class-specific descriptors (HID, Audio, Video, CDC, OTG, ...).

use super::{bcd, header, hex_n, DescKind, DescNode, Reader};

pub fn decode_class_specific(d: &[u8], base: usize, class: u8, sub: u8, _proto: u8) -> DescNode {
    let typ = d.get(1).copied().unwrap_or(0);
    match (typ, class) {
        (0x21, 0x03) => decode_hid(d, base),
        (0x09, _) => decode_otg(d, base),
        (0x24, 0x01) => decode_audio(d, base, sub),
        (0x24, 0x0E) => decode_video(d, base, sub),
        (0x24, 0x02) | (0x24, 0x0A) => decode_cdc(d, base),
        (0x21, 0xFE) if sub == 0x01 => decode_dfu(d, base),
        _ => {
            let title = match typ {
                0x24 => "Class-specific Interface",
                0x25 => "Class-specific Endpoint",
                0x21 => "Class-specific (0x21)",
                _ => super::descriptor_type_name(typ),
            };
            let mut r = Reader::new(d, base, title, DescKind::ClassSpecific);
            header(&mut r, 0);
            if typ == 0x24 || typ == 0x25 {
                r.hex("bDescriptorSubtype", 1);
            }
            r.rest("data");
            r.finish()
        }
    }
}

fn decode_hid(d: &[u8], base: usize) -> DescNode {
    let mut r = Reader::new(d, base, "HID Descriptor", DescKind::Hid);
    header(&mut r, 0x21);
    r.num("bcdHID", 2, |v| format!("0x{v:04X} (HID {})", bcd(v)));
    r.num("bCountryCode", 1, |v| {
        let c = match v {
            0 => "not localized",
            9 => "German",
            13 => "International (ISO)",
            33 => "US",
            32 => "UK",
            8 => "French",
            15 => "Japanese",
            _ => "",
        };
        if c.is_empty() { format!("0x{v:02X}") } else { format!("0x{v:02X} ({c})") }
    });
    let n = r.dec("bNumDescriptors", 1).unwrap_or(0);
    for i in 0..n {
        r.hex_note(&format!("bDescriptorType[{i}]"), 1, |v| match v {
            0x22 => "Report".into(),
            0x23 => "Physical".into(),
            _ => String::new(),
        });
        r.num(&format!("wDescriptorLength[{i}]"), 2, |v| format!("{v} bytes"));
    }
    r.rest("extra");
    r.finish()
}

fn decode_otg(d: &[u8], base: usize) -> DescNode {
    let mut r = Reader::new(d, base, "OTG Descriptor", DescKind::Otg);
    header(&mut r, 0x09);
    r.num("bmAttributes", 1, |v| {
        let mut p = Vec::new();
        if v & 1 != 0 { p.push("SRP") }
        if v & 2 != 0 { p.push("HNP") }
        if v & 4 != 0 { p.push("ADP") }
        format!("0x{v:02X} ({})", p.join(", "))
    });
    if r.remaining() >= 2 {
        r.num("bcdOTG", 2, |v| format!("0x{v:04X} ({})", bcd(v)));
    }
    r.finish()
}

fn decode_dfu(d: &[u8], base: usize) -> DescNode {
    let mut r = Reader::new(d, base, "DFU Functional Descriptor", DescKind::ClassSpecific);
    header(&mut r, 0x21);
    r.num("bmAttributes", 1, |v| {
        let mut p = Vec::new();
        if v & 1 != 0 { p.push("download") }
        if v & 2 != 0 { p.push("upload") }
        if v & 4 != 0 { p.push("manifestation tolerant") }
        if v & 8 != 0 { p.push("will detach") }
        format!("0x{v:02X} ({})", p.join(", "))
    });
    r.num("wDetachTimeOut", 2, |v| format!("{v} ms"));
    r.num("wTransferSize", 2, |v| format!("{v} bytes"));
    if r.remaining() >= 2 {
        r.num("bcdDFUVersion", 2, |v| format!("0x{v:04X} ({})", bcd(v)));
    }
    r.finish()
}

fn subtype(r: &mut Reader, names: &dyn Fn(u8) -> &'static str) -> u8 {
    r.hex_note("bDescriptorSubtype", 1, |v| names(v as u8).to_string()).unwrap_or(0) as u8
}

fn decode_audio(d: &[u8], base: usize, sub: u8) -> DescNode {
    let mut r = Reader::new(d, base, "Audio Class Interface", DescKind::ClassSpecific);
    header(&mut r, 0x24);
    let control = sub == 0x01;
    let st = subtype(&mut r, &|v| match (control, v) {
        (true, 0x01) => "Header",
        (true, 0x02) => "Input Terminal",
        (true, 0x03) => "Output Terminal",
        (true, 0x04) => "Mixer Unit",
        (true, 0x05) => "Selector Unit",
        (true, 0x06) => "Feature Unit",
        (true, 0x07) => "Effect / Processing Unit",
        (true, 0x0A) => "Clock Source",
        (true, 0x0B) => "Clock Selector",
        (true, 0x0C) => "Clock Multiplier",
        (false, 0x01) => "General",
        (false, 0x02) => "Format Type",
        _ => "",
    });
    let name = match (control, st) {
        (true, 0x01) => "Audio Control Header",
        (true, 0x02) => "Audio Input Terminal",
        (true, 0x03) => "Audio Output Terminal",
        (true, 0x06) => "Audio Feature Unit",
        (true, 0x0A) => "Audio Clock Source",
        (false, 0x01) => "Audio Streaming General",
        (false, 0x02) => "Audio Format Type",
        _ => "Audio Class-specific",
    };
    r.node.title = name.into();
    match (control, st) {
        (true, 0x01) => {
            r.num("bcdADC", 2, |v| format!("0x{v:04X} ({})", bcd(v)));
        }
        (true, 0x02) | (true, 0x03) => {
            r.dec("bTerminalID", 1);
            r.hex_note("wTerminalType", 2, |v| terminal_type(v as u16).to_string());
        }
        (false, 0x02) if r.remaining() >= 1 => {
            r.hex("bFormatType", 1);
        }
        _ => {}
    }
    r.rest("data");
    r.finish()
}

pub fn terminal_type(t: u16) -> &'static str {
    match t {
        0x0101 => "USB Streaming",
        0x0201 => "Microphone",
        0x0202 => "Desktop Microphone",
        0x0203 => "Personal Microphone",
        0x0204 => "Omni-directional Microphone",
        0x0205 => "Microphone Array",
        0x0301 => "Speaker",
        0x0302 => "Headphones",
        0x0303 => "Head Mounted Display Audio",
        0x0304 => "Desktop Speaker",
        0x0402 => "Headset",
        0x0403 => "Speakerphone",
        0x0603 => "Line Connector",
        0x0605 => "S/PDIF Interface",
        _ => "",
    }
}

fn decode_video(d: &[u8], base: usize, sub: u8) -> DescNode {
    let mut r = Reader::new(d, base, "Video Class Interface", DescKind::ClassSpecific);
    header(&mut r, 0x24);
    let control = sub == 0x01;
    let st = subtype(&mut r, &|v| match (control, v) {
        (true, 0x01) => "VC Header",
        (true, 0x02) => "Input Terminal",
        (true, 0x03) => "Output Terminal",
        (true, 0x04) => "Selector Unit",
        (true, 0x05) => "Processing Unit",
        (true, 0x06) => "Extension Unit",
        (true, 0x07) => "Encoding Unit",
        (false, 0x01) => "Input Header",
        (false, 0x02) => "Output Header",
        (false, 0x03) => "Still Image Frame",
        (false, 0x04) => "Format Uncompressed",
        (false, 0x05) => "Frame Uncompressed",
        (false, 0x06) => "Format MJPEG",
        (false, 0x07) => "Frame MJPEG",
        (false, 0x0D) => "Color Matching",
        (false, 0x10) => "Format Frame-based",
        (false, 0x11) => "Frame Frame-based",
        _ => "",
    });
    r.node.title = match (control, st) {
        (true, 0x01) => "Video Control Header".into(),
        (true, 0x02) => "Video Input Terminal".into(),
        (true, 0x05) => "Video Processing Unit".into(),
        (true, 0x06) => "Video Extension Unit".into(),
        (false, 0x01) => "Video Streaming Input Header".into(),
        (false, 0x04) => "Video Format – Uncompressed".into(),
        (false, 0x06) => "Video Format – MJPEG".into(),
        (false, 0x10) => "Video Format – Frame-based".into(),
        (false, 0x05) | (false, 0x07) | (false, 0x11) => "Video Frame".into(),
        (false, 0x0D) => "Video Color Matching".into(),
        _ => "Video Class-specific".into(),
    };
    match (control, st) {
        (true, 0x01) => {
            r.num("bcdUVC", 2, |v| format!("0x{v:04X} (UVC {})", bcd(v)));
        }
        (false, 0x05) | (false, 0x07) | (false, 0x11) => {
            r.dec("bFrameIndex", 1);
            r.hex("bmCapabilities", 1);
            let w = r.dec("wWidth", 2).unwrap_or(0);
            let h = r.dec("wHeight", 2).unwrap_or(0);
            r.node.title = format!("Video Frame {w}×{h}");
        }
        (false, 0x04) | (false, 0x06) | (false, 0x10) => {
            r.dec("bFormatIndex", 1);
            r.dec("bNumFrameDescriptors", 1);
            if st != 0x06 {
                r.bytes("guidFormat", 16, |g| {
                    let s = super::uuid(g);
                    let fourcc: String = g[..4].iter().map(|&c| if c.is_ascii_graphic() { c as char } else { '.' }).collect();
                    format!("{s} ({fourcc})")
                });
            }
        }
        _ => {}
    }
    r.rest("data");
    r.finish()
}

fn decode_cdc(d: &[u8], base: usize) -> DescNode {
    let mut r = Reader::new(d, base, "CDC Functional", DescKind::ClassSpecific);
    header(&mut r, 0x24);
    let st = subtype(&mut r, &|v| match v {
        0x00 => "Header",
        0x01 => "Call Management",
        0x02 => "Abstract Control Management",
        0x06 => "Union",
        0x0F => "Ethernet Networking",
        0x1B => "Mobile Broadband",
        0x1C => "MBIM Extended",
        _ => "",
    });
    r.node.title = match st {
        0x00 => "CDC Header".into(),
        0x01 => "CDC Call Management".into(),
        0x02 => "CDC Abstract Control Management".into(),
        0x06 => "CDC Union".into(),
        0x0F => "CDC Ethernet Networking".into(),
        _ => format!("CDC Functional {}", hex_n(st as u64, 1)),
    };
    match st {
        0x00 => {
            r.num("bcdCDC", 2, |v| format!("0x{v:04X} ({})", bcd(v)));
        }
        0x01 => {
            r.hex("bmCapabilities", 1);
            r.dec("bDataInterface", 1);
        }
        0x02 => {
            r.hex("bmCapabilities", 1);
        }
        0x06 => {
            r.dec("bControlInterface", 1);
            let mut i = 0;
            while r.remaining() > 0 {
                r.dec(&format!("bSubordinateInterface[{i}]"), 1);
                i += 1;
            }
        }
        0x0F => {
            r.dec("iMACAddress", 1);
            r.hex("bmEthernetStatistics", 4);
            r.num("wMaxSegmentSize", 2, |v| format!("{v} bytes"));
        }
        _ => {}
    }
    r.rest("data");
    r.finish()
}
