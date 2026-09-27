//! Static name tables for USB classes and language IDs.

pub fn class_name(c: u8) -> &'static str {
    match c {
        0x00 => "Device (per interface)",
        0x01 => "Audio",
        0x02 => "Communications (CDC)",
        0x03 => "HID",
        0x05 => "Physical",
        0x06 => "Image",
        0x07 => "Printer",
        0x08 => "Mass Storage",
        0x09 => "Hub",
        0x0A => "CDC Data",
        0x0B => "Smart Card",
        0x0D => "Content Security",
        0x0E => "Video",
        0x0F => "Personal Healthcare",
        0x10 => "Audio/Video",
        0x11 => "Billboard",
        0x12 => "USB Type-C Bridge",
        0x13 => "Bulk Display",
        0x14 => "MCTP over USB",
        0x3C => "I3C",
        0xDC => "Diagnostic",
        0xE0 => "Wireless Controller",
        0xEF => "Miscellaneous",
        0xFE => "Application Specific",
        0xFF => "Vendor Specific",
        _ => "Unknown class",
    }
}

pub fn subclass_name(class: u8, sub: u8) -> String {
    let s = match (class, sub) {
        (0x01, 0x01) => "Audio Control",
        (0x01, 0x02) => "Audio Streaming",
        (0x01, 0x03) => "MIDI Streaming",
        (0x02, 0x01) => "Direct Line",
        (0x02, 0x02) => "Abstract Control Model",
        (0x02, 0x06) => "Ethernet Networking",
        (0x02, 0x0D) => "Network Control Model",
        (0x02, 0x0E) => "Mobile Broadband Interface Model",
        (0x03, 0x00) => "No Subclass",
        (0x03, 0x01) => "Boot Interface",
        (0x06, 0x01) => "Still Image Capture",
        (0x08, 0x01) => "RBC",
        (0x08, 0x02) => "MMC-5 (ATAPI)",
        (0x08, 0x04) => "UFI (Floppy)",
        (0x08, 0x06) => "SCSI transparent",
        (0x0E, 0x01) => "Video Control",
        (0x0E, 0x02) => "Video Streaming",
        (0x0E, 0x03) => "Video Interface Collection",
        (0xE0, 0x01) => "RF Controller",
        (0xEF, 0x02) => "Common Class",
        (0xFE, 0x01) => "Device Firmware Upgrade",
        (0xFE, 0x02) => "IrDA Bridge",
        (0xFE, 0x03) => "Test & Measurement",
        _ => "",
    };
    s.to_string()
}

pub fn protocol_name(class: u8, sub: u8, proto: u8) -> String {
    let s = match (class, sub, proto) {
        (0x03, 0x01, 0x01) => "Keyboard",
        (0x03, 0x01, 0x02) => "Mouse",
        (0x08, _, 0x50) => "Bulk-Only Transport",
        (0x08, _, 0x62) => "USB Attached SCSI (UAS)",
        (0x08, _, 0x00) => "Control/Bulk/Interrupt",
        (0x09, _, 0x00) => "Full Speed Hub",
        (0x09, _, 0x01) => "Hi-Speed Hub, single TT",
        (0x09, _, 0x02) => "Hi-Speed Hub, multiple TTs",
        (0x09, _, 0x03) => "SuperSpeed Hub",
        (0xE0, 0x01, 0x01) => "Bluetooth",
        (0xEF, 0x02, 0x01) => "Interface Association",
        (0x02, 0x02, 0x01) => "AT commands (V.250)",
        (0x01, _, 0x20) => "UAC 2.0",
        (0x01, _, 0x30) => "UAC 3.0",
        _ => "",
    };
    s.to_string()
}

pub fn lang_name(id: u16) -> &'static str {
    match id {
        0x0409 => "English (United States)",
        0x0809 => "English (United Kingdom)",
        0x0407 => "German (Germany)",
        0x040C => "French (France)",
        0x0410 => "Italian (Italy)",
        0x0C0A => "Spanish (Modern)",
        0x0411 => "Japanese",
        0x0412 => "Korean",
        0x0804 => "Chinese (PRC)",
        0x0404 => "Chinese (Taiwan)",
        0x0419 => "Russian",
        0x0416 => "Portuguese (Brazil)",
        0x0413 => "Dutch (Netherlands)",
        0x041D => "Swedish",
        0x0415 => "Polish",
        0x041F => "Turkish",
        _ => "Unknown language",
    }
}
