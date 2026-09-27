//! Platform-independent USB descriptor decoding.
//!
//! Every decoder produces a [`DescNode`] tree whose [`Field`]s remember their
//! byte offset inside the raw blob, so the UI can cross-highlight a decoded
//! field with the bytes of the hex view.

mod bos;
mod class;
mod names;

#[cfg(test)]
mod tests;

pub use names::{class_name, lang_name};

use serde::{Deserialize, Serialize};

/// One decoded field of a descriptor.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Field {
    pub name: String,
    /// Absolute offset in the raw blob this node was parsed from.
    pub offset: usize,
    pub size: usize,
    pub value: u64,
    /// Human readable value, e.g. `0x0200 (USB 2.00)`.
    pub display: String,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum DescKind {
    Device,
    Configuration,
    InterfaceAssociation,
    Interface,
    Endpoint,
    SsEndpointCompanion,
    SspIsocCompanion,
    Hid,
    ClassSpecific,
    Otg,
    Bos,
    DeviceCapability,
    Hub,
    String,
    Unknown,
}

/// A decoded descriptor with nested children (interfaces under a config etc.).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DescNode {
    pub title: String,
    pub kind: DescKind,
    pub offset: usize,
    pub len: usize,
    pub fields: Vec<Field>,
    pub children: Vec<DescNode>,
    pub warnings: Vec<String>,
}

impl DescNode {
    fn new(title: impl Into<String>, kind: DescKind, offset: usize, len: usize) -> Self {
        Self {
            title: title.into(),
            kind,
            offset,
            len,
            fields: Vec::new(),
            children: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// Depth-first iterator over this node and all descendants.
    pub fn walk(&self) -> Vec<&DescNode> {
        let mut out = vec![self];
        for c in &self.children {
            out.extend(c.walk());
        }
        out
    }

    pub fn field(&self, name: &str) -> Option<&Field> {
        self.fields.iter().find(|f| f.name == name)
    }
}

/// Little helper that reads fields from a slice while recording offsets.
pub(crate) struct Reader<'a> {
    data: &'a [u8],
    base: usize,
    pos: usize,
    pub node: DescNode,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8], base: usize, title: impl Into<String>, kind: DescKind) -> Self {
        let len = data.len();
        Self {
            data,
            base,
            pos: 0,
            node: DescNode::new(title, kind, base, len),
        }
    }

    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    fn raw(&mut self, size: usize) -> Option<u64> {
        if self.pos + size > self.data.len() {
            return None;
        }
        let mut v = 0u64;
        for i in 0..size {
            v |= (self.data[self.pos + i] as u64) << (8 * i);
        }
        Some(v)
    }

    /// Reads a little-endian integer of `size` bytes and records a field.
    /// `fmt` renders the display string from the value.
    pub fn num(&mut self, name: &str, size: usize, fmt: impl FnOnce(u64) -> String) -> Option<u64> {
        let Some(v) = self.raw(size) else {
            self.node
                .warnings
                .push(format!("Descriptor truncated before {name}"));
            self.pos = self.data.len();
            return None;
        };
        self.node.fields.push(Field {
            name: name.to_string(),
            offset: self.base + self.pos,
            size,
            value: v,
            display: fmt(v),
        });
        self.pos += size;
        Some(v)
    }

    pub fn hex(&mut self, name: &str, size: usize) -> Option<u64> {
        self.num(name, size, |v| hex_n(v, size))
    }

    pub fn hex_note(
        &mut self,
        name: &str,
        size: usize,
        note: impl FnOnce(u64) -> String,
    ) -> Option<u64> {
        self.num(name, size, |v| {
            let n = note(v);
            if n.is_empty() {
                hex_n(v, size)
            } else {
                format!("{} ({n})", hex_n(v, size))
            }
        })
    }

    pub fn dec(&mut self, name: &str, size: usize) -> Option<u64> {
        self.num(name, size, |v| format!("{v}"))
    }

    /// Records a run of bytes (e.g. a UUID) as a single field.
    pub fn bytes(
        &mut self,
        name: &str,
        size: usize,
        fmt: impl FnOnce(&[u8]) -> String,
    ) -> Option<&'a [u8]> {
        if self.pos + size > self.data.len() {
            self.node
                .warnings
                .push(format!("Descriptor truncated before {name}"));
            self.pos = self.data.len();
            return None;
        }
        let s = &self.data[self.pos..self.pos + size];
        self.node.fields.push(Field {
            name: name.to_string(),
            offset: self.base + self.pos,
            size,
            value: 0,
            display: fmt(s),
        });
        self.pos += size;
        Some(s)
    }

    /// Records the remaining bytes as an opaque field if any remain.
    pub fn rest(&mut self, name: &str) {
        let n = self.remaining();
        if n > 0 {
            self.bytes(name, n, hex_bytes);
        }
    }

    pub fn finish(self) -> DescNode {
        self.node
    }
}

pub fn hex_n(v: u64, size: usize) -> String {
    format!("0x{:0width$X}", v, width = size * 2)
}

pub fn hex_bytes(b: &[u8]) -> String {
    b.iter()
        .map(|x| format!("{x:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn bcd(v: u64) -> String {
    format!("{:X}.{:02X}", (v >> 8) & 0xFF, v & 0xFF)
}

/// Formats a GUID/UUID stored in USB little-endian layout.
pub fn uuid(b: &[u8]) -> String {
    if b.len() != 16 {
        return hex_bytes(b);
    }
    format!(
        "{{{:02X}{:02X}{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}}}",
        b[3], b[2], b[1], b[0], b[5], b[4], b[7], b[6], b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]
    )
}

fn header(r: &mut Reader, expected: u8) {
    r.dec("bLength", 1);
    r.hex_note("bDescriptorType", 1, |v| {
        let n = descriptor_type_name(v as u8);
        if v as u8 != expected && expected != 0 {
            format!("{n} – unexpected")
        } else {
            n.to_string()
        }
    });
}

pub fn descriptor_type_name(t: u8) -> &'static str {
    match t {
        0x01 => "Device",
        0x02 => "Configuration",
        0x03 => "String",
        0x04 => "Interface",
        0x05 => "Endpoint",
        0x06 => "Device Qualifier",
        0x07 => "Other Speed Configuration",
        0x08 => "Interface Power",
        0x09 => "OTG",
        0x0A => "Debug",
        0x0B => "Interface Association",
        0x0F => "BOS",
        0x10 => "Device Capability",
        0x21 => "HID",
        0x22 => "HID Report",
        0x24 => "Class-specific Interface",
        0x25 => "Class-specific Endpoint",
        0x29 => "Hub",
        0x2A => "SuperSpeed Hub",
        0x30 => "SuperSpeed Endpoint Companion",
        0x31 => "SuperSpeedPlus Isochronous Endpoint Companion",
        _ => "Unknown",
    }
}

/// Values of the device descriptor needed elsewhere in the app.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeviceDescriptor {
    pub bcd_usb: u16,
    pub class: u8,
    pub subclass: u8,
    pub protocol: u8,
    pub max_packet0: u8,
    pub vid: u16,
    pub pid: u16,
    pub bcd_device: u16,
    pub i_manufacturer: u8,
    pub i_product: u8,
    pub i_serial: u8,
    pub num_configurations: u8,
}

impl DeviceDescriptor {
    pub fn from_bytes(b: &[u8]) -> Option<Self> {
        if b.len() < 18 {
            return None;
        }
        let w = |i: usize| u16::from_le_bytes([b[i], b[i + 1]]);
        Some(Self {
            bcd_usb: w(2),
            class: b[4],
            subclass: b[5],
            protocol: b[6],
            max_packet0: b[7],
            vid: w(8),
            pid: w(10),
            bcd_device: w(12),
            i_manufacturer: b[14],
            i_product: b[15],
            i_serial: b[16],
            num_configurations: b[17],
        })
    }

    pub fn to_bytes(self) -> [u8; 18] {
        let mut b = [0u8; 18];
        b[0] = 18;
        b[1] = 1;
        b[2..4].copy_from_slice(&self.bcd_usb.to_le_bytes());
        b[4] = self.class;
        b[5] = self.subclass;
        b[6] = self.protocol;
        b[7] = self.max_packet0;
        b[8..10].copy_from_slice(&self.vid.to_le_bytes());
        b[10..12].copy_from_slice(&self.pid.to_le_bytes());
        b[12..14].copy_from_slice(&self.bcd_device.to_le_bytes());
        b[14] = self.i_manufacturer;
        b[15] = self.i_product;
        b[16] = self.i_serial;
        b[17] = self.num_configurations;
        b
    }
}

/// Resolves string descriptor indexes to text for display.
pub type StringLookup<'a> = &'a dyn Fn(u8) -> Option<String>;

fn string_note(strings: StringLookup, idx: u64) -> String {
    if idx == 0 {
        return "no string".into();
    }
    match strings(idx as u8) {
        Some(s) => format!("\"{s}\""),
        None => "string not available".into(),
    }
}

pub fn decode_device(b: &[u8], strings: StringLookup) -> DescNode {
    let mut r = Reader::new(b, 0, "Device Descriptor", DescKind::Device);
    header(&mut r, 0x01);
    r.num("bcdUSB", 2, |v| format!("0x{v:04X} (USB {})", bcd(v)));
    let class = r.hex_note("bDeviceClass", 1, |v| {
        if v == 0 {
            "defined by interface".into()
        } else {
            class_name(v as u8).to_string()
        }
    });
    r.hex("bDeviceSubClass", 1);
    r.hex("bDeviceProtocol", 1);
    r.num("bMaxPacketSize0", 1, |v| format!("{v} bytes"));
    let vid = r.hex_note("idVendor", 2, |_| String::new());
    r.hex("idProduct", 2);
    r.num("bcdDevice", 2, |v| format!("0x{v:04X} ({})", bcd(v)));
    r.hex_note("iManufacturer", 1, |v| string_note(strings, v));
    r.hex_note("iProduct", 1, |v| string_note(strings, v));
    r.hex_note("iSerialNumber", 1, |v| string_note(strings, v));
    r.dec("bNumConfigurations", 1);
    let mut node = r.finish();
    if b.len() >= 2 && b[0] != 18 {
        node.warnings.push(format!(
            "bLength is {} but a device descriptor is 18 bytes",
            b[0]
        ));
    }
    if vid == Some(0) {
        node.warnings
            .push("idVendor is 0x0000 – the device probably failed enumeration".into());
    }
    if class == Some(0xEF) && b.len() >= 7 && (b[5] != 0x02 || b[6] != 0x01) {
        node.warnings
            .push("Class 0xEF should use SubClass 0x02 / Protocol 0x01 (IAD)".into());
    }
    node
}

/// Context carried while walking a configuration descriptor.
#[derive(Clone, Copy, Default)]
struct IfaceCtx {
    class: u8,
    subclass: u8,
    protocol: u8,
}

pub fn decode_configuration(b: &[u8], strings: StringLookup, super_speed: bool) -> DescNode {
    let clen = (b.first().copied().unwrap_or(9) as usize)
        .clamp(2, b.len().max(2))
        .min(b.len());
    let mut r = Reader::new(
        &b[..clen],
        0,
        "Configuration Descriptor",
        DescKind::Configuration,
    );
    header(&mut r, 0x02);
    let total = r.num("wTotalLength", 2, |v| format!("0x{v:04X} ({v} bytes)"));
    r.dec("bNumInterfaces", 1);
    r.dec("bConfigurationValue", 1);
    r.hex_note("iConfiguration", 1, |v| string_note(strings, v));
    r.num("bmAttributes", 1, |v| {
        let mut parts = Vec::new();
        if v & 0x40 != 0 {
            parts.push("Self-powered")
        } else {
            parts.push("Bus-powered")
        }
        if v & 0x20 != 0 {
            parts.push("Remote Wakeup")
        }
        format!("0x{v:02X} ({})", parts.join(", "))
    });
    let unit = if super_speed { 8 } else { 2 };
    r.num("MaxPower", 1, move |v| {
        format!("0x{v:02X} ({} mA)", v * unit)
    });
    let mut config = r.finish();

    if let Some(t) = total {
        if t as usize != b.len() {
            config.warnings.push(format!(
                "wTotalLength is {t} but {} bytes were returned",
                b.len()
            ));
        }
    }
    if let Some(a) = config.field("bmAttributes") {
        if a.value & 0x80 == 0 {
            config
                .warnings
                .push("bmAttributes bit 7 must be set (USB 1.1+)".into());
        }
    }

    // Walk the remaining descriptors and nest them:
    // Config > [IAD] > Interface > (class-specific | endpoint > companion)
    let mut pos = clen;
    let mut ctx = IfaceCtx::default();
    let mut iad_idx: Option<usize> = None;
    let mut iad_range = 0u8..0u8;
    let mut cur_iface: Option<DescNode> = None;

    // Pushes the current interface into the config or its IAD.
    fn flush_iface(config: &mut DescNode, iad_idx: Option<usize>, iface: Option<DescNode>) {
        if let Some(i) = iface {
            match iad_idx {
                Some(ix) => config.children[ix].children.push(i),
                None => config.children.push(i),
            }
        }
    }

    while pos + 2 <= b.len() {
        let len = b[pos] as usize;
        let typ = b[pos + 1];
        if len < 2 {
            config
                .warnings
                .push(format!("Invalid descriptor length {len} at offset {pos}"));
            break;
        }
        let end = (pos + len).min(b.len());
        if pos + len > b.len() {
            config.warnings.push(format!(
                "Descriptor at offset {pos} runs past the end of the data"
            ));
        }
        let d = &b[pos..end];
        match typ {
            0x0B => {
                flush_iface(&mut config, iad_idx, cur_iface.take());
                let n = decode_iad(d, pos, strings);
                let first = d.get(2).copied().unwrap_or(0);
                let count = d.get(3).copied().unwrap_or(0);
                iad_range = first..first.saturating_add(count);
                config.children.push(n);
                iad_idx = Some(config.children.len() - 1);
            }
            0x04 => {
                flush_iface(&mut config, iad_idx, cur_iface.take());
                let n = decode_interface(d, pos, strings);
                ctx = IfaceCtx {
                    class: d.get(5).copied().unwrap_or(0),
                    subclass: d.get(6).copied().unwrap_or(0),
                    protocol: d.get(7).copied().unwrap_or(0),
                };
                let num = d.get(2).copied().unwrap_or(0);
                if !iad_range.contains(&num) {
                    iad_idx = None;
                }
                cur_iface = Some(n);
            }
            0x05 => {
                let n = decode_endpoint(d, pos, ctx.class);
                match cur_iface.as_mut() {
                    Some(i) => i.children.push(n),
                    None => config.children.push(n),
                }
            }
            0x30 | 0x31 => {
                let n = if typ == 0x30 {
                    decode_ss_companion(d, pos)
                } else {
                    decode_ssp_isoc_companion(d, pos)
                };
                // Attach to the last endpoint of the current interface.
                let target = cur_iface.as_mut().and_then(|i| {
                    i.children
                        .iter_mut()
                        .rev()
                        .find(|c| c.kind == DescKind::Endpoint)
                });
                match target {
                    Some(ep) => ep.children.push(n),
                    None => config.children.push(n),
                }
            }
            _ => {
                let n = class::decode_class_specific(d, pos, ctx.class, ctx.subclass, ctx.protocol);
                match cur_iface.as_mut() {
                    Some(i) => {
                        // Class-specific endpoint descriptors follow their endpoint.
                        if typ == 0x25 {
                            if let Some(ep) = i
                                .children
                                .iter_mut()
                                .rev()
                                .find(|c| c.kind == DescKind::Endpoint)
                            {
                                ep.children.push(n);
                            } else {
                                i.children.push(n);
                            }
                        } else {
                            i.children.push(n);
                        }
                    }
                    None => config.children.push(n),
                }
            }
        }
        pos += len;
    }
    flush_iface(&mut config, iad_idx, cur_iface.take());
    config
}

fn decode_iad(d: &[u8], base: usize, strings: StringLookup) -> DescNode {
    let mut r = Reader::new(
        d,
        base,
        "Interface Association",
        DescKind::InterfaceAssociation,
    );
    header(&mut r, 0x0B);
    r.dec("bFirstInterface", 1);
    r.dec("bInterfaceCount", 1);
    r.hex_note("bFunctionClass", 1, |v| class_name(v as u8).to_string());
    r.hex("bFunctionSubClass", 1);
    r.hex("bFunctionProtocol", 1);
    r.hex_note("iFunction", 1, |v| string_note(strings, v));
    let mut n = r.finish();
    if let Some(c) = n.field("bFunctionClass") {
        n.title = format!("Interface Association – {}", class_name(c.value as u8));
    }
    n
}

fn decode_interface(d: &[u8], base: usize, strings: StringLookup) -> DescNode {
    let mut r = Reader::new(d, base, "Interface", DescKind::Interface);
    header(&mut r, 0x04);
    let num = r.dec("bInterfaceNumber", 1).unwrap_or(0);
    let alt = r.dec("bAlternateSetting", 1).unwrap_or(0);
    r.dec("bNumEndpoints", 1);
    let class = r
        .hex_note("bInterfaceClass", 1, |v| class_name(v as u8).to_string())
        .unwrap_or(0);
    let sub = r
        .hex_note("bInterfaceSubClass", 1, |v| {
            names::subclass_name(d.get(5).copied().unwrap_or(0), v as u8)
        })
        .unwrap_or(0);
    r.hex_note("bInterfaceProtocol", 1, |v| {
        names::protocol_name(d.get(5).copied().unwrap_or(0), sub as u8, v as u8)
    });
    r.hex_note("iInterface", 1, |v| string_note(strings, v));
    r.rest("extra");
    let mut n = r.finish();
    n.title = if alt == 0 {
        format!("Interface {num} – {}", class_name(class as u8))
    } else {
        format!("Interface {num} alt {alt} – {}", class_name(class as u8))
    };
    n
}

pub fn transfer_type(attr: u8) -> &'static str {
    match attr & 3 {
        0 => "Control",
        1 => "Isochronous",
        2 => "Bulk",
        _ => "Interrupt",
    }
}

fn decode_endpoint(d: &[u8], base: usize, _iface_class: u8) -> DescNode {
    let mut r = Reader::new(d, base, "Endpoint", DescKind::Endpoint);
    header(&mut r, 0x05);
    let addr = r
        .num("bEndpointAddress", 1, |v| {
            format!(
                "0x{v:02X} ({} {})",
                if v & 0x80 != 0 { "IN" } else { "OUT" },
                v & 0x0F
            )
        })
        .unwrap_or(0);
    let attr = r
        .num("bmAttributes", 1, |v| {
            let tt = transfer_type(v as u8);
            let mut s = format!("0x{v:02X} ({tt}");
            if v & 3 == 1 {
                let sync =
                    ["No Sync", "Asynchronous", "Adaptive", "Synchronous"][((v >> 2) & 3) as usize];
                let usage = ["Data", "Feedback", "Implicit Feedback Data", "Reserved"]
                    [((v >> 4) & 3) as usize];
                s += &format!(", {sync}, {usage}");
            } else if v & 3 == 3 && (v >> 4) & 3 == 1 {
                s += ", Notification";
            }
            s + ")"
        })
        .unwrap_or(0);
    r.num("wMaxPacketSize", 2, |v| {
        let size = v & 0x7FF;
        let mult = (v >> 11) & 3;
        if mult > 0 {
            format!(
                "0x{v:04X} ({} x {size} bytes = {} bytes)",
                mult + 1,
                (mult + 1) * size
            )
        } else {
            format!("0x{v:04X} ({size} bytes)")
        }
    });
    r.num("bInterval", 1, |v| match attr & 3 {
        2 => format!("0x{v:02X} (ignored for bulk / NAK rate)"),
        1 | 3 => format!("0x{v:02X} ({v})"),
        _ => format!("0x{v:02X}"),
    });
    r.rest("extra");
    let mut n = r.finish();
    n.title = format!(
        "Endpoint 0x{addr:02X} {} {}",
        if addr & 0x80 != 0 { "IN" } else { "OUT" },
        transfer_type(attr as u8)
    );
    n
}

fn decode_ss_companion(d: &[u8], base: usize) -> DescNode {
    let mut r = Reader::new(
        d,
        base,
        "SuperSpeed Endpoint Companion",
        DescKind::SsEndpointCompanion,
    );
    header(&mut r, 0x30);
    r.num("bMaxBurst", 1, |v| {
        format!("{v} ({} packets per burst)", v + 1)
    });
    r.hex("bmAttributes", 1);
    r.num("wBytesPerInterval", 2, |v| format!("{v} bytes"));
    r.finish()
}

fn decode_ssp_isoc_companion(d: &[u8], base: usize) -> DescNode {
    let mut r = Reader::new(
        d,
        base,
        "SuperSpeedPlus Isoch Endpoint Companion",
        DescKind::SspIsocCompanion,
    );
    header(&mut r, 0x31);
    r.hex("wReserved", 2);
    r.num("dwBytesPerInterval", 4, |v| format!("{v} bytes"));
    r.finish()
}

pub use bos::decode_bos;

pub fn decode_device_qualifier(d: &[u8]) -> DescNode {
    let mut r = Reader::new(d, 0, "Device Qualifier", DescKind::Unknown);
    header(&mut r, 0x06);
    r.num("bcdUSB", 2, |v| format!("0x{v:04X} (USB {})", bcd(v)));
    r.hex_note("bDeviceClass", 1, |v| class_name(v as u8).to_string());
    r.hex("bDeviceSubClass", 1);
    r.hex("bDeviceProtocol", 1);
    r.num("bMaxPacketSize0", 1, |v| format!("{v} bytes"));
    r.dec("bNumConfigurations", 1);
    r.hex("bReserved", 1);
    r.finish()
}

/// Decodes a USB 2.0 (0x29) or SuperSpeed (0x2A) hub descriptor.
pub fn decode_hub(d: &[u8]) -> DescNode {
    let ss = d.get(1) == Some(&0x2A);
    let mut r = Reader::new(
        d,
        0,
        if ss {
            "SuperSpeed Hub Descriptor"
        } else {
            "Hub Descriptor"
        },
        DescKind::Hub,
    );
    header(&mut r, if ss { 0x2A } else { 0x29 });
    r.dec("bNumberOfPorts", 1);
    r.num("wHubCharacteristics", 2, |v| {
        let power = match v & 3 {
            0 => "ganged power switching",
            1 => "individual port power switching",
            _ => "no power switching",
        };
        let compound = if v & 4 != 0 { ", compound device" } else { "" };
        let oc = match (v >> 3) & 3 {
            0 => "global over-current",
            1 => "individual port over-current",
            _ => "no over-current protection",
        };
        let mut s = format!("0x{v:04X} ({power}{compound}, {oc}");
        if !ss {
            s += &format!(", TT think time {} FS bit times", 8 * (((v >> 5) & 3) + 1));
            if v & 0x80 != 0 {
                s += ", port indicators";
            }
        }
        s + ")"
    });
    r.num("bPwrOn2PwrGood", 1, |v| format!("{v} ({} ms)", v * 2));
    r.num("bHubContrCurrent", 1, |v| {
        if ss {
            format!("{} mA", v * 4)
        } else {
            format!("{v} mA")
        }
    });
    if ss {
        r.hex("bHubHdrDecLat", 1);
        r.num("wHubDelay", 2, |v| format!("{v} ns"));
        r.hex("DeviceRemovable", 2);
    } else {
        r.rest("DeviceRemovable / PortPwrCtrlMask");
    }
    r.finish()
}

/// Decodes a string descriptor to text (UTF-16LE).
pub fn decode_string(d: &[u8]) -> Option<String> {
    if d.len() < 2 || d[1] != 0x03 {
        return None;
    }
    let len = (d[0] as usize).min(d.len());
    let units: Vec<u16> = d[2..len]
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    Some(
        String::from_utf16_lossy(&units)
            .trim_end_matches('\0')
            .to_string(),
    )
}

/// Decodes string descriptor 0 (supported language IDs).
pub fn decode_langids(d: &[u8]) -> Vec<u16> {
    if d.len() < 4 || d[1] != 0x03 {
        return Vec::new();
    }
    let len = (d[0] as usize).min(d.len());
    d[2..len]
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect()
}
