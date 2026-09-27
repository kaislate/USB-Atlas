//! Chooses an icon glyph for each node.

use egui_phosphor::regular as ph;

use crate::model::Device;
use crate::tree::{FlatNode, NodeKind};

pub fn for_node(n: &FlatNode, dev: Option<&Device>) -> &'static str {
    match n.kind {
        NodeKind::Computer => ph::DESKTOP_TOWER,
        NodeKind::Controller => ph::CPU,
        NodeKind::RootHub => ph::TREE_STRUCTURE,
        NodeKind::Hub => ph::USB,
        NodeKind::EmptyPort => ph::CIRCLE_DASHED,
        NodeKind::Child => for_child_label(&n.label, &n.detail),
        NodeKind::Device => for_device(n, dev),
    }
}

fn for_child_label(label: &str, class: &str) -> &'static str {
    let s = format!("{label} {class}").to_lowercase();
    if s.contains("disk") || s.contains("storage") || s.contains("volume") {
        ph::HARD_DRIVE
    } else if s.contains("keyboard") {
        ph::KEYBOARD
    } else if s.contains("mouse") {
        ph::MOUSE
    } else if s.contains("hid") || s.contains("input") {
        ph::GAME_CONTROLLER
    } else if s.contains("camera") || s.contains("image") {
        ph::WEBCAM
    } else if s.contains("audio")
        || s.contains("media")
        || s.contains("microphone")
        || s.contains("speaker")
    {
        ph::SPEAKER_HIGH
    } else if s.contains("port") || s.contains("com") || s.contains("serial") {
        ph::TERMINAL_WINDOW
    } else if s.contains("net") || s.contains("ethernet") || s.contains("adapter") {
        ph::NETWORK
    } else if s.contains("bluetooth") {
        ph::BLUETOOTH
    } else if s.contains("wpd")
        || s.contains("phone")
        || s.contains("android")
        || s.contains("iphone")
    {
        ph::DEVICE_MOBILE
    } else {
        ph::CUBE
    }
}

fn for_device(n: &FlatNode, dev: Option<&Device>) -> &'static str {
    let name = n.label.to_lowercase();
    let svc = dev
        .and_then(|d| d.info.as_ref())
        .map(|i| format!("{} {}", i.service, i.class).to_lowercase())
        .unwrap_or_default();
    if name.contains("android")
        || name.contains("iphone")
        || name.contains("phone")
        || svc.contains("wpd")
        || svc.contains("wudf") && name.contains("mtp")
    {
        return ph::DEVICE_MOBILE;
    }
    if name.contains("keyboard") || name.contains("keychron") {
        return ph::KEYBOARD;
    }
    if name.contains("mouse") {
        return ph::MOUSE;
    }
    if name.contains("launchkey") || name.contains("midi") || name.contains("piano") {
        return ph::PIANO_KEYS;
    }
    if name.contains("goxlr")
        || name.contains("headset")
        || name.contains("headphone")
        || name.contains("dac")
    {
        return ph::HEADPHONES;
    }
    if name.contains("led") || name.contains("aura") || name.contains("rgb") {
        return ph::LIGHTBULB;
    }
    let classes = &n.classes;
    let has = |c: u8| classes.contains(&c);
    if has(0xE0) || svc.contains("bluetooth") || name.contains("bluetooth") {
        ph::BLUETOOTH
    } else if has(0x0E) {
        ph::WEBCAM
    } else if has(0x08) {
        ph::HARD_DRIVES
    } else if has(0x01) {
        if name.contains("mic") {
            ph::MICROPHONE
        } else {
            ph::SPEAKER_HIGH
        }
    } else if has(0x07) {
        ph::PRINTER
    } else if has(0x06) {
        ph::CAMERA
    } else if has(0x0B) {
        ph::IDENTIFICATION_CARD
    } else if has(0x02) || has(0x0A) {
        if svc.contains("usbser") || svc.contains("ports") {
            ph::TERMINAL_WINDOW
        } else {
            ph::NETWORK
        }
    } else if has(0x03) {
        ph::GAME_CONTROLLER
    } else if has(0xFE) {
        ph::WRENCH
    } else if has(0x11) {
        ph::INFO
    } else {
        ph::CUBE
    }
}
