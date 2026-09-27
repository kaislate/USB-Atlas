//! Lookup of vendor / product / class names from the embedded `usb.ids`
//! database (http://www.linux-usb.org/usb.ids).

use std::collections::HashMap;
use std::sync::OnceLock;

static RAW: &[u8] = include_bytes!("../assets/usb.ids");

#[derive(Default)]
pub struct UsbIds {
    vendors: HashMap<u16, (String, HashMap<u16, String>)>,
    classes: HashMap<(u8, Option<u8>, Option<u8>), String>,
    langs: HashMap<u16, String>,
    pub version: String,
}

fn hex16(s: &str) -> Option<u16> {
    u16::from_str_radix(s, 16).ok()
}

fn hex8(s: &str) -> Option<u8> {
    u8::from_str_radix(s, 16).ok()
}

/// Splits `"xxxx  name"` into id and name.
fn split(line: &str) -> Option<(&str, &str)> {
    let (id, name) = line.split_once(char::is_whitespace)?;
    Some((id, name.trim()))
}

impl UsbIds {
    pub fn parse(text: &str) -> Self {
        #[derive(Clone, Copy)]
        enum Section {
            Vendor(u16),
            Class(u8, Option<u8>),
            Lang,
            Other,
        }
        let mut ids = UsbIds::default();
        let mut sec = Section::Other;
        for line in text.lines() {
            if let Some(v) = line.strip_prefix("# Version:") {
                ids.version = v.trim().to_string();
            }
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let tabs = line.bytes().take_while(|&b| b == b'\t').count();
            let body = &line[tabs..];
            match tabs {
                0 => {
                    if let Some(rest) = body.strip_prefix("C ") {
                        if let Some((id, name)) = split(rest).and_then(|(i, n)| Some((hex8(i)?, n)))
                        {
                            ids.classes.insert((id, None, None), name.to_string());
                            sec = Section::Class(id, None);
                            continue;
                        }
                    } else if let Some(rest) = body.strip_prefix("L ") {
                        if let Some((id, name)) =
                            split(rest).and_then(|(i, n)| Some((hex16(i)?, n)))
                        {
                            ids.langs.insert(id, name.to_string());
                        }
                        sec = Section::Lang;
                        continue;
                    }
                    match split(body)
                        .and_then(|(i, n)| Some((hex16(i).filter(|_| i.len() == 4)?, n)))
                    {
                        Some((vid, name)) => {
                            ids.vendors.insert(vid, (name.to_string(), HashMap::new()));
                            sec = Section::Vendor(vid);
                        }
                        None => sec = Section::Other,
                    }
                }
                1 => match sec {
                    Section::Vendor(vid) => {
                        if let Some((pid, name)) =
                            split(body).and_then(|(i, n)| Some((hex16(i)?, n)))
                        {
                            if let Some(v) = ids.vendors.get_mut(&vid) {
                                v.1.insert(pid, name.to_string());
                            }
                        }
                    }
                    Section::Class(c, _) => {
                        if let Some((sub, name)) =
                            split(body).and_then(|(i, n)| Some((hex8(i)?, n)))
                        {
                            ids.classes.insert((c, Some(sub), None), name.to_string());
                            sec = Section::Class(c, Some(sub));
                        }
                    }
                    _ => {}
                },
                2 => {
                    if let Section::Class(c, Some(sub)) = sec {
                        if let Some((p, name)) = split(body).and_then(|(i, n)| Some((hex8(i)?, n)))
                        {
                            ids.classes
                                .insert((c, Some(sub), Some(p)), name.to_string());
                        }
                    }
                }
                _ => {}
            }
        }
        ids
    }

    pub fn vendor(&self, vid: u16) -> Option<&str> {
        self.vendors.get(&vid).map(|v| v.0.as_str())
    }

    pub fn product(&self, vid: u16, pid: u16) -> Option<&str> {
        self.vendors.get(&vid)?.1.get(&pid).map(String::as_str)
    }

    pub fn class(&self, c: u8, sub: Option<u8>, proto: Option<u8>) -> Option<&str> {
        self.classes.get(&(c, sub, proto)).map(String::as_str)
    }

    pub fn language(&self, id: u16) -> Option<&str> {
        self.langs.get(&id).map(String::as_str)
    }

    #[cfg(test)]
    pub fn vendor_count(&self) -> usize {
        self.vendors.len()
    }
}

static DB: OnceLock<UsbIds> = OnceLock::new();

/// Global database, parsed on first use (~20 ms).
pub fn db() -> &'static UsbIds {
    DB.get_or_init(|| UsbIds::parse(&String::from_utf8_lossy(RAW)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "# Version: 2026.06.26\n\
046d  Logitech, Inc.\n\
\tc52b  Unifying Receiver\n\
\t\t00  some interface\n\
8087  Intel Corp.\n\
\n\
C 03  Human Interface Device\n\
\t01  Boot Interface Subclass\n\
\t\t02  Mouse\n\
L 0409  English\n";

    #[test]
    fn parses_sample() {
        let ids = UsbIds::parse(SAMPLE);
        assert_eq!(ids.version, "2026.06.26");
        assert_eq!(ids.vendor(0x046D), Some("Logitech, Inc."));
        assert_eq!(ids.product(0x046D, 0xC52B), Some("Unifying Receiver"));
        assert_eq!(ids.vendor(0x8087), Some("Intel Corp."));
        assert_eq!(ids.class(0x03, None, None), Some("Human Interface Device"));
        assert_eq!(ids.class(0x03, Some(1), Some(2)), Some("Mouse"));
        assert_eq!(ids.language(0x0409), Some("English"));
    }

    #[test]
    fn parses_embedded_database() {
        let ids = db();
        assert!(ids.vendor_count() > 3000);
        assert_eq!(ids.vendor(0x046D), Some("Logitech, Inc."));
    }
}
