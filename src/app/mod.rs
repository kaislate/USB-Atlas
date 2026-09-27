//! The egui application: state, background scanning and top-level layout.

mod bottom;
mod detail_view;
mod hex;
pub mod icon;
mod icons;
mod palette;
mod settings;
mod theme;
mod toasts;
mod tree_view;
mod widgets;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};

use egui::{Key, KeyboardShortcut, Modifiers};
use egui_phosphor::regular as ph;

use crate::insights::{self, Insight};
use crate::model::*;
use crate::platform;
use crate::tree::{self, Change, ChangeKind, Flat, FlatNode, FlattenOptions, NodePath};

pub use settings::Settings;
use theme::{Palette, ThemeMode};
use toasts::{Toast, ToastKind};

pub const APP_NAME: &str = "USB Atlas";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DetailTab {
    Overview,
    Descriptors,
    Report,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BottomTab {
    Activity,
    Insights,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum QuickFilter {
    #[default]
    All,
    Problems,
    SuperSpeed,
    Storage,
    Input,
}

#[derive(Clone, Debug)]
pub struct Event {
    pub time: String,
    pub kind: ChangeKind,
    pub node_id: String,
    pub name: String,
    pub vid_pid: Option<(u16, u16)>,
}

pub struct Ghost {
    pub node: FlatNode,
    pub parent_id: String,
    pub at: Instant,
}

#[derive(Clone, Debug)]
pub enum Action {
    Refresh,
    SafelyRemove { id: String, name: String },
    Restart { id: String, name: String },
    SetEnabled { id: String, name: String, enable: bool },
    CyclePort { hub: String, port: u32, name: String },
    Properties(String),
    OpenPath(String),
    CopyText(String),
    CopyNodeReport(String),
    CopyFullReport,
    SaveSnapshot,
    OpenSnapshot,
    OpenSnapshotPath(PathBuf),
    BackToLive,
    ExportText,
    ExportHtml,
    Compare,
    RunAsAdmin,
    DeviceManager,
    ToggleTheme,
    ToggleEmptyPorts,
    ToggleChildDevices,
    ToggleLive,
    ExpandAll,
    CollapseAll,
    Select(String),
    Rename(String),
    TogglePin(String),
    OpenPalette,
    OpenSettings,
    About,
}

pub enum Source {
    Live,
    File(PathBuf),
    Demo,
}

pub struct CompareState {
    pub name: String,
    pub changes: Vec<Change>,
}

/// A pending destructive action awaiting confirmation.
pub struct Confirm {
    pub title: String,
    pub body: String,
    pub action: Action,
}

pub struct App {
    pub settings: Settings,
    pub p: Palette,
    system_dark: bool,

    pub snapshot: Option<Snapshot>,
    pub source: Source,
    pub flat: Flat,
    pub insights: Vec<Insight>,
    /// node id -> (hub symbolic name, port number) for companion lookups.
    pub port_map: HashMap<String, (String, u32)>,

    pub selected: Option<String>,
    pub collapsed: HashSet<String>,
    pub scroll_to_selected: bool,
    pub search: String,
    pub quick_filter: QuickFilter,
    pub focus_search: bool,

    pub events: Vec<Event>,
    pub arrivals: HashMap<String, Instant>,
    pub ghosts: Vec<Ghost>,
    pub toasts: Vec<Toast>,

    scan_tx: Sender<()>,
    scan_rx: Receiver<Snapshot>,
    pub scanning: bool,
    scan_started: Option<Instant>,
    rescan_at: Option<Instant>,
    notify_rx: Receiver<()>,
    _watcher: Option<platform::Watcher>,
    action_tx: Sender<(String, Result<String, String>)>,
    action_rx: Receiver<(String, Result<String, String>)>,

    pub detail_tab: DetailTab,
    pub bottom_open: bool,
    pub bottom_tab: BottomTab,
    pub palette_open: bool,
    pub palette_query: String,
    pub palette_sel: usize,
    pub hover_bytes: Option<(String, usize, usize)>,
    /// Hover state of the previous frame, used for highlighting.
    pub hover_view: Option<(String, usize, usize)>,
    pub hover_from_hex: bool,
    pub hover_from_hex_prev: bool,
    pub confirm: Option<Confirm>,
    pub rename: Option<(String, String)>,
    pub compare: Option<CompareState>,
    pub settings_open: bool,
    pub about_open: bool,
    pub report_cache: Option<(String, String)>,
    pending: Vec<Action>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, open: Option<PathBuf>) -> Self {
        theme::install_fonts(&cc.egui_ctx);
        let settings = Settings::load();
        let system_dark = cc.egui_ctx.system_theme().map(|t| t == egui::Theme::Dark).unwrap_or(true);
        let dark = match settings.theme {
            ThemeMode::System => system_dark,
            ThemeMode::Dark => true,
            ThemeMode::Light => false,
        };
        let p = Palette::new(dark, settings.accent);
        theme::apply(&cc.egui_ctx, &p);

        // Scanner thread: every request produces a fresh snapshot.
        let (scan_tx, req_rx) = channel::<()>();
        let (res_tx, scan_rx) = channel::<Snapshot>();
        let ctx = cc.egui_ctx.clone();
        std::thread::Builder::new()
            .name("usb-scan".into())
            .spawn(move || {
                while req_rx.recv().is_ok() {
                    // Coalesce queued requests.
                    while req_rx.try_recv().is_ok() {}
                    let snap = platform::scan();
                    if res_tx.send(snap).is_err() {
                        break;
                    }
                    ctx.request_repaint();
                }
            })
            .expect("spawn scanner");

        let (ntx, notify_rx) = channel::<()>();
        let ctx2 = cc.egui_ctx.clone();
        let watcher = platform::Watcher::start(ntx, move || ctx2.request_repaint());
        let (action_tx, action_rx) = channel();

        let mut app = Self {
            settings,
            p,
            system_dark,
            snapshot: None,
            source: Source::Live,
            flat: Flat::default(),
            insights: Vec::new(),
            port_map: HashMap::new(),
            selected: None,
            collapsed: HashSet::new(),
            scroll_to_selected: false,
            search: String::new(),
            quick_filter: QuickFilter::All,
            focus_search: false,
            events: Vec::new(),
            arrivals: HashMap::new(),
            ghosts: Vec::new(),
            toasts: Vec::new(),
            scan_tx,
            scan_rx,
            scanning: false,
            scan_started: None,
            rescan_at: None,
            notify_rx,
            _watcher: watcher,
            action_tx,
            action_rx,
            detail_tab: DetailTab::Overview,
            bottom_open: false,
            bottom_tab: BottomTab::Activity,
            palette_open: false,
            palette_query: String::new(),
            palette_sel: 0,
            hover_bytes: None,
            hover_view: None,
            hover_from_hex: false,
            hover_from_hex_prev: false,
            confirm: None,
            rename: None,
            compare: None,
            settings_open: false,
            about_open: false,
            report_cache: None,
            pending: Vec::new(),
        };
        if app.settings.always_on_top {
            cc.egui_ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(egui::WindowLevel::AlwaysOnTop));
        }
        match open {
            Some(path) => app.load_snapshot_file(path),
            None => app.request_scan(),
        }
        app
    }

    pub fn is_live(&self) -> bool {
        matches!(self.source, Source::Live)
    }

    fn request_scan(&mut self) {
        if !self.is_live() {
            return;
        }
        self.scanning = true;
        self.scan_started = Some(Instant::now());
        let _ = self.scan_tx.send(());
    }

    pub fn dispatch(&mut self, a: Action) {
        self.pending.push(a);
    }

    pub fn toast(&mut self, kind: ToastKind, title: impl Into<String>, body: impl Into<String>) {
        self.toasts.push(Toast::new(kind, title.into(), body.into()));
    }

    pub fn flatten_opts(&self) -> FlattenOptions {
        FlattenOptions { show_empty_ports: self.settings.show_empty_ports, show_child_devices: self.settings.show_child_devices }
    }

    /// Rebuilds the flat tree (and ghosts) from the current snapshot.
    pub fn rebuild(&mut self) {
        let Some(snap) = &self.snapshot else {
            self.flat = Flat::default();
            return;
        };
        let mut flat = tree::flatten(snap, &self.flatten_opts());
        // Apply nicknames.
        for n in flat.nodes.iter_mut() {
            if let Some(nick) = self.nickname_for(n) {
                n.search_text.push(' ');
                n.search_text.push_str(&nick.to_lowercase());
            }
        }
        // Keep recently removed devices visible as fading ghosts.
        self.ghosts.retain(|g| g.at.elapsed() < Duration::from_secs(8));
        for g in &self.ghosts {
            if flat.by_id.contains_key(&g.node.id) {
                continue;
            }
            if let Some(&pi) = flat.by_id.get(&g.parent_id) {
                let mut n = g.node.clone();
                n.parent = Some(pi);
                n.children.clear();
                n.depth = flat.nodes[pi].depth + 1;
                let idx = flat.nodes.len();
                flat.by_id.insert(n.id.clone(), idx);
                flat.nodes.push(n);
                // Insert in port order among siblings.
                let port_no = |n: &FlatNode| n.port_label.trim_start_matches("Port ").parse::<u32>().unwrap_or(u32::MAX);
                let my = port_no(&flat.nodes[idx]);
                let pos = flat.nodes[pi].children.iter().position(|&c| port_no(&flat.nodes[c]) > my);
                match pos {
                    Some(p) => flat.nodes[pi].children.insert(p, idx),
                    None => flat.nodes[pi].children.push(idx),
                }
            }
        }
        self.port_map.clear();
        for n in &flat.nodes {
            if let NodePath::Port(ci, ch) = &n.path {
                let hub_sym = if ch.len() == 1 {
                    snap.controllers[*ci].root_hub.as_ref().map(|h| h.symbolic_name.clone())
                } else {
                    tree::port(snap, *ci, &ch[..ch.len() - 1])
                        .and_then(|p| p.device.as_ref())
                        .and_then(|d| d.hub.as_ref())
                        .map(|h| h.symbolic_name.clone())
                };
                if let (Some(h), Some(p)) = (hub_sym, tree::port(snap, *ci, ch)) {
                    self.port_map.insert(n.id.clone(), (h, p.index));
                }
            }
        }
        self.insights = insights::analyze(snap, &flat);
        self.flat = flat;
        self.report_cache = None;
        if self.selected.as_ref().is_none_or(|s| !self.flat.by_id.contains_key(s)) {
            self.selected = self.flat.nodes.first().map(|n| n.id.clone());
        }
    }

    pub fn is_ghost(&self, id: &str) -> Option<f32> {
        self.ghosts.iter().find(|g| g.node.id == id).map(|g| g.at.elapsed().as_secs_f32())
    }

    fn on_new_snapshot(&mut self, snap: Snapshot) {
        let first = self.snapshot.is_none();
        let changes = match &self.snapshot {
            Some(old) if self.is_live() => tree::diff(old, &snap),
            _ => vec![],
        };
        let old_flat = std::mem::take(&mut self.flat);
        let now = platform::now_string();
        let mut jump: Option<String> = None;
        for c in &changes {
            match c.kind {
                ChangeKind::Arrived => {
                    self.arrivals.insert(c.node_id.clone(), Instant::now());
                    self.ghosts.retain(|g| g.node.id != c.node_id);
                    if self.settings.notify_arrivals {
                        self.toasts.push(Toast::new(ToastKind::Arrived, "Device connected".into(), c.name.clone()).with_target(c.node_id.clone()));
                    }
                    if self.settings.jump_to_new {
                        jump = Some(c.node_id.clone());
                    }
                }
                ChangeKind::Removed => {
                    if let Some(n) = old_flat.get(&c.node_id) {
                        let parent_id = n.parent.map(|p| old_flat.nodes[p].id.clone()).unwrap_or_default();
                        self.ghosts.push(Ghost { node: n.clone(), parent_id, at: Instant::now() });
                    }
                    if self.settings.notify_arrivals {
                        self.toasts.push(Toast::new(ToastKind::Removed, "Device disconnected".into(), c.name.clone()));
                    }
                }
                ChangeKind::ProblemAppeared(code) => {
                    self.toasts.push(
                        Toast::new(ToastKind::Error, format!("Problem code {code}"), format!("{} – {}", c.name, insights::problem_text(code)))
                            .with_target(c.node_id.clone()),
                    );
                }
                ChangeKind::ProblemCleared => {}
            }
            self.events.push(Event { time: now.clone(), kind: c.kind.clone(), node_id: c.node_id.clone(), name: c.name.clone(), vid_pid: c.vid_pid });
        }
        if self.events.len() > 1000 {
            let n = self.events.len() - 1000;
            self.events.drain(..n);
        }
        self.snapshot = Some(snap);
        self.rebuild();
        if first {
            // Start with the first device-bearing controller selected.
            if let Some(n) = self.flat.nodes.iter().find(|n| n.kind == tree::NodeKind::Device) {
                self.selected = Some(n.id.clone());
            }
        }
        if let Some(j) = jump {
            self.select(j);
        }
    }

    pub fn select(&mut self, id: String) {
        if let Some(&idx) = self.flat.by_id.get(&id) {
            for a in self.flat.ancestors(idx) {
                self.collapsed.remove(&self.flat.nodes[a].id);
            }
            self.selected = Some(id);
            self.scroll_to_selected = true;
            self.hover_bytes = None;
            self.report_cache = None;
        }
    }

    pub fn selected_node(&self) -> Option<&FlatNode> {
        self.selected.as_ref().and_then(|s| self.flat.get(s))
    }

    pub fn device_at(&self, path: &NodePath) -> Option<&Device> {
        match path {
            NodePath::Port(ci, ch) => tree::port(self.snapshot.as_ref()?, *ci, ch)?.device.as_ref(),
            _ => None,
        }
    }

    pub fn port_at(&self, path: &NodePath) -> Option<&Port> {
        match path {
            NodePath::Port(ci, ch) => tree::port(self.snapshot.as_ref()?, *ci, ch),
            _ => None,
        }
    }

    /// Stable key for nicknames / pins.
    pub fn device_key(&self, n: &FlatNode) -> Option<String> {
        let d = self.device_at(&n.path)?;
        let (v, p) = d.vid_pid()?;
        Some(match d.serial() {
            Some(s) if !s.is_empty() => format!("{v:04X}:{p:04X}:{s}"),
            _ => format!("{v:04X}:{p:04X}@{}", n.id),
        })
    }

    pub fn nickname_for(&self, n: &FlatNode) -> Option<String> {
        let key = self.device_key(n)?;
        self.settings.nicknames.get(&key).cloned()
    }

    pub fn display_label(&self, n: &FlatNode) -> String {
        self.nickname_for(n).unwrap_or_else(|| n.label.clone())
    }

    /// Node id of the companion port of `id` (USB2 <-> USB3 halves).
    pub fn companions_of(&self, id: &str) -> Vec<String> {
        let Some(port) = self.flat.get(id).and_then(|n| self.port_at(&n.path)) else { return vec![] };
        let Some((my_hub, my_port)) = self.port_map.get(id) else { return vec![] };
        let mut out = Vec::new();
        if let Some(c) = &port.connector {
            if c.companion_port != 0 && !c.companion_hub.is_empty() {
                for (nid, (h, p)) in &self.port_map {
                    if *p == c.companion_port as u32 && h.eq_ignore_ascii_case(&c.companion_hub) {
                        out.push(nid.clone());
                    }
                }
            }
        }
        // Reverse direction: ports that name us as their companion.
        for (nid, _) in self.port_map.iter() {
            if nid == id || out.contains(nid) {
                continue;
            }
            if let Some(pp) = self.flat.get(nid).and_then(|n| self.port_at(&n.path)) {
                if let Some(c) = &pp.connector {
                    if c.companion_port as u32 == *my_port && c.companion_hub.eq_ignore_ascii_case(my_hub) {
                        out.push(nid.clone());
                    }
                }
            }
        }
        out
    }

    fn apply_theme(&mut self, ctx: &egui::Context) {
        let dark = match self.settings.theme {
            ThemeMode::System => self.system_dark,
            ThemeMode::Dark => true,
            ThemeMode::Light => false,
        };
        self.p = Palette::new(dark, self.settings.accent);
        theme::apply(ctx, &self.p);
    }

    pub fn load_demo(&mut self) {
        self.source = Source::Demo;
        self.snapshot = None;
        self.on_new_snapshot(crate::demo::snapshot());
    }

    fn load_snapshot_file(&mut self, path: PathBuf) {
        match std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|s| serde_json::from_str::<Snapshot>(&s).map_err(|e| e.to_string())) {
            Ok(snap) => {
                self.source = Source::File(path.clone());
                self.snapshot = None;
                self.ghosts.clear();
                self.arrivals.clear();
                self.on_new_snapshot(snap);
                self.toast(ToastKind::Info, "Snapshot opened", path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default());
            }
            Err(e) => self.toast(ToastKind::Error, "Could not open snapshot", e),
        }
    }

    fn run_device_action(&mut self, title: &str, f: impl FnOnce() -> Result<String, String> + Send + 'static) {
        let tx = self.action_tx.clone();
        let title = title.to_string();
        self.toast(ToastKind::Info, format!("{title}…"), "Working");
        std::thread::spawn(move || {
            let r = f();
            let _ = tx.send((title, r));
        });
    }

    fn needs_confirm(&self, a: &Action) -> Option<(String, String)> {
        if !self.settings.confirm_actions {
            return None;
        }
        match a {
            Action::Restart { name, .. } => Some((format!("Restart {name}?"), "The device will be stopped and started again. Unsaved data on it may be lost.".into())),
            Action::SetEnabled { name, enable: false, .. } => Some((format!("Disable {name}?"), "The device stays disabled until you enable it again (also after a reboot).".into())),
            Action::CyclePort { name, port, .. } => Some((format!("Cycle port {port}?"), format!("{name} will be disconnected and re-enumerated, like unplugging and plugging it back in."))),
            _ => None,
        }
    }

    fn handle(&mut self, a: Action, ctx: &egui::Context, confirmed: bool) {
        if !confirmed {
            if let Some((title, body)) = self.needs_confirm(&a) {
                self.confirm = Some(Confirm { title, body, action: a });
                return;
            }
        }
        match a {
            Action::Refresh => {
                if self.is_live() {
                    self.request_scan();
                } else {
                    self.toast(ToastKind::Info, "Viewing a snapshot", "Switch back to live view to refresh.");
                }
            }
            Action::SafelyRemove { id, name } => self.run_device_action(&format!("Eject {name}"), move || platform::safely_remove(&id)),
            Action::Restart { id, name } => self.run_device_action(&format!("Restart {name}"), move || platform::restart(&id)),
            Action::SetEnabled { id, name, enable } => {
                let t = if enable { format!("Enable {name}") } else { format!("Disable {name}") };
                self.run_device_action(&t, move || platform::set_enabled(&id, enable))
            }
            Action::CyclePort { hub, port, name } => self.run_device_action(&format!("Cycle port ({name})"), move || platform::cycle_port(&hub, port)),
            Action::Properties(id) => {
                if let Err(e) = platform::open_device_properties(&id) {
                    self.toast(ToastKind::Error, "Could not open properties", e);
                }
            }
            Action::OpenPath(p) => {
                if let Err(e) = platform::open_path(&p) {
                    self.toast(ToastKind::Error, "Could not open", e);
                }
            }
            Action::CopyText(t) => {
                ctx.copy_text(t);
                self.toast(ToastKind::Success, "Copied to clipboard", "");
            }
            Action::CopyNodeReport(id) => {
                if let (Some(n), Some(s)) = (self.flat.get(&id), &self.snapshot) {
                    let t = crate::details::to_text(&self.display_label(n), &crate::details::sections(s, &n.path), self.settings.hexdumps_in_reports);
                    ctx.copy_text(t);
                    self.toast(ToastKind::Success, "Report copied", n.label.clone());
                }
            }
            Action::CopyFullReport => {
                if let Some(s) = &self.snapshot {
                    ctx.copy_text(crate::details::full_report(s, self.settings.hexdumps_in_reports));
                    self.toast(ToastKind::Success, "Full report copied", "");
                }
            }
            Action::SaveSnapshot => {
                if let Some(s) = &self.snapshot {
                    let name = format!("usb-snapshot-{}.json", chrono::Local::now().format("%Y%m%d-%H%M%S"));
                    if let Some(path) = rfd::FileDialog::new().add_filter("Snapshot", &["json"]).set_file_name(name).save_file() {
                        match serde_json::to_string_pretty(s).map_err(|e| e.to_string()).and_then(|j| std::fs::write(&path, j).map_err(|e| e.to_string())) {
                            Ok(()) => self.toast(ToastKind::Success, "Snapshot saved", path.display().to_string()),
                            Err(e) => self.toast(ToastKind::Error, "Save failed", e),
                        }
                    }
                }
            }
            Action::OpenSnapshot => {
                if let Some(path) = rfd::FileDialog::new().add_filter("Snapshot", &["json"]).pick_file() {
                    self.load_snapshot_file(path);
                }
            }
            Action::OpenSnapshotPath(path) => self.load_snapshot_file(path),
            Action::BackToLive => {
                self.source = Source::Live;
                self.snapshot = None;
                self.compare = None;
                self.request_scan();
            }
            Action::ExportText | Action::ExportHtml => {
                let html = matches!(a, Action::ExportHtml);
                if let Some(s) = &self.snapshot {
                    let (ext, content) = if html {
                        ("html", crate::export::html_report(s, self.settings.hexdumps_in_reports))
                    } else {
                        ("txt", crate::details::full_report(s, self.settings.hexdumps_in_reports))
                    };
                    let name = format!("usb-report-{}.{ext}", chrono::Local::now().format("%Y%m%d-%H%M%S"));
                    if let Some(path) = rfd::FileDialog::new().add_filter(ext.to_uppercase(), &[ext]).set_file_name(name).save_file() {
                        match std::fs::write(&path, content) {
                            Ok(()) => {
                                let p = path.display().to_string();
                                self.toasts.push(Toast::new(ToastKind::Success, "Report exported".into(), p.clone()).with_open(p));
                            }
                            Err(e) => self.toast(ToastKind::Error, "Export failed", e.to_string()),
                        }
                    }
                }
            }
            Action::Compare => {
                if let (Some(cur), Some(path)) = (&self.snapshot, rfd::FileDialog::new().add_filter("Snapshot", &["json"]).pick_file()) {
                    match std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|s| serde_json::from_str::<Snapshot>(&s).map_err(|e| e.to_string())) {
                        Ok(other) => {
                            let changes = tree::diff(&other, cur);
                            self.compare = Some(CompareState { name: path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default(), changes });
                        }
                        Err(e) => self.toast(ToastKind::Error, "Could not read snapshot", e),
                    }
                }
            }
            Action::RunAsAdmin => match platform::relaunch_as_admin() {
                Ok(()) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                Err(e) => self.toast(ToastKind::Error, "Could not restart elevated", e),
            },
            Action::DeviceManager => {
                let _ = platform::open_device_manager();
            }
            Action::ToggleTheme => {
                let dark_now = self.p.dark;
                self.settings.theme = if dark_now { ThemeMode::Light } else { ThemeMode::Dark };
                self.apply_theme(ctx);
                self.settings.save();
            }
            Action::ToggleEmptyPorts => {
                self.settings.show_empty_ports = !self.settings.show_empty_ports;
                self.rebuild();
                self.settings.save();
            }
            Action::ToggleChildDevices => {
                self.settings.show_child_devices = !self.settings.show_child_devices;
                self.rebuild();
                self.settings.save();
            }
            Action::ToggleLive => {
                self.settings.auto_refresh = !self.settings.auto_refresh;
                self.settings.save();
                self.toast(ToastKind::Info, if self.settings.auto_refresh { "Live updates on" } else { "Live updates paused" }, "");
            }
            Action::ExpandAll => self.collapsed.clear(),
            Action::CollapseAll => {
                self.collapsed = self
                    .flat
                    .nodes
                    .iter()
                    .filter(|n| !n.children.is_empty() && n.depth >= 1)
                    .map(|n| n.id.clone())
                    .collect();
            }
            Action::Select(id) => self.select(id),
            Action::Rename(id) => {
                if let Some(n) = self.flat.get(&id) {
                    let cur = self.nickname_for(n).unwrap_or_else(|| n.label.clone());
                    self.rename = Some((id, cur));
                }
            }
            Action::TogglePin(id) => {
                if let Some(pos) = self.settings.pinned.iter().position(|x| *x == id) {
                    self.settings.pinned.remove(pos);
                } else {
                    self.settings.pinned.push(id);
                }
                self.settings.save();
            }
            Action::OpenPalette => {
                self.palette_open = true;
                self.palette_query.clear();
                self.palette_sel = 0;
            }
            Action::OpenSettings => self.settings_open = true,
            Action::About => self.about_open = true,
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let sc = |m: Modifiers, k: Key| KeyboardShortcut::new(m, k);
        let mut acts = Vec::new();
        ctx.input_mut(|i| {
            if i.consume_shortcut(&sc(Modifiers::NONE, Key::F5)) || i.consume_shortcut(&sc(Modifiers::COMMAND, Key::R)) {
                acts.push(Action::Refresh);
            }
            if i.consume_shortcut(&sc(Modifiers::COMMAND, Key::K)) || i.consume_shortcut(&sc(Modifiers::COMMAND, Key::P)) {
                acts.push(Action::OpenPalette);
            }
            if i.consume_shortcut(&sc(Modifiers::COMMAND, Key::F)) {
                self.focus_search = true;
            }
            if i.consume_shortcut(&sc(Modifiers::COMMAND, Key::S)) {
                acts.push(Action::SaveSnapshot);
            }
            if i.consume_shortcut(&sc(Modifiers::COMMAND, Key::O)) {
                acts.push(Action::OpenSnapshot);
            }
            if i.consume_shortcut(&sc(Modifiers::COMMAND | Modifiers::SHIFT, Key::C)) {
                acts.push(Action::CopyFullReport);
            }
            if i.consume_shortcut(&sc(Modifiers::COMMAND, Key::E)) {
                acts.push(Action::ExportHtml);
            }
            if i.consume_shortcut(&sc(Modifiers::COMMAND, Key::J)) {
                self.bottom_open = !self.bottom_open;
            }
            if i.consume_shortcut(&sc(Modifiers::COMMAND, Key::Comma)) {
                acts.push(Action::OpenSettings);
            }
            if i.consume_shortcut(&sc(Modifiers::COMMAND | Modifiers::SHIFT, Key::L)) {
                acts.push(Action::ToggleTheme);
            }
        });
        for a in acts {
            self.dispatch(a);
        }
    }
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Hot-plug notifications -> debounced rescan.
        let mut notified = false;
        while self.notify_rx.try_recv().is_ok() {
            notified = true;
        }
        if notified && self.settings.auto_refresh && self.is_live() {
            self.rescan_at = Some(Instant::now() + Duration::from_millis(350));
        }
        if let Some(t) = self.rescan_at {
            if Instant::now() >= t {
                self.rescan_at = None;
                self.request_scan();
            } else {
                ctx.request_repaint_after(t - Instant::now());
            }
        }
        while let Ok(snap) = self.scan_rx.try_recv() {
            self.scanning = false;
            if self.is_live() {
                self.on_new_snapshot(snap);
            }
        }
        while let Ok((title, r)) = self.action_rx.try_recv() {
            match r {
                Ok(msg) => self.toast(ToastKind::Success, title, msg),
                Err(msg) => self.toast(ToastKind::Error, title, msg),
            }
            self.request_scan();
        }
        // System theme follow.
        if let Some(t) = ctx.system_theme() {
            let d = t == egui::Theme::Dark;
            if d != self.system_dark {
                self.system_dark = d;
                if self.settings.theme == ThemeMode::System {
                    self.apply_theme(ctx);
                }
            }
        }
        // Keep animations (arrival glow, ghosts, toasts) ticking.
        let animating = self.arrivals.values().any(|t| t.elapsed() < Duration::from_secs(6))
            || !self.ghosts.is_empty()
            || !self.toasts.is_empty()
            || self.scanning;
        if animating {
            ctx.request_repaint_after(Duration::from_millis(33));
        }
        let before = self.ghosts.len();
        self.ghosts.retain(|g| g.at.elapsed() < Duration::from_secs(8));
        self.arrivals.retain(|_, t| t.elapsed() < Duration::from_secs(8));
        if before != self.ghosts.len() {
            self.rebuild();
        }
        // Dropped snapshot files.
        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect());
        if let Some(p) = dropped.into_iter().next() {
            self.dispatch(Action::OpenSnapshotPath(p));
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.hover_view = self.hover_bytes.take();
        self.hover_from_hex_prev = std::mem::take(&mut self.hover_from_hex);
        if !self.palette_open && self.rename.is_none() && self.confirm.is_none() {
            self.shortcuts(&ctx);
        }

        self.top_bar(ui);
        self.status_bar(ui);
        bottom::show(self, ui);

        let p = self.p;
        egui::Panel::left("tree")
            .resizable(true)
            .default_size(self.settings.tree_width)
            .size_range(280.0..=900.0)
            .frame(egui::Frame::new().fill(p.panel).inner_margin(egui::Margin { left: 8, right: 4, top: 8, bottom: 8 }))
            .show(ui, |ui| {
                tree_view::show(self, ui);
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(p.bg).inner_margin(egui::Margin::same(0)))
            .show(ui, |ui| {
                detail_view::show(self, ui);
            });

        palette::show(self, &ctx);
        self.dialogs(&ctx);
        toasts::show(self, &ctx);

        let actions = std::mem::take(&mut self.pending);
        for a in actions {
            self.handle(a, &ctx, false);
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.settings.save();
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        self.p.bg.to_normalized_gamma_f32()
    }
}

impl App {
    fn top_bar(&mut self, ui: &mut egui::Ui) {
        let p = self.p;
        egui::Panel::top("top")
            .exact_size(52.0)
            .frame(egui::Frame::new().fill(p.panel).inner_margin(egui::Margin::symmetric(14, 10)).stroke(egui::Stroke::new(1.0, p.border)))
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    // Brand
                    let (r, _) = ui.allocate_exact_size(egui::vec2(28.0, 28.0), egui::Sense::hover());
                    ui.painter().rect_filled(r, egui::CornerRadius::same(8), p.accent);
                    ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, ph::TREE_STRUCTURE, theme::icon_fill(17.0), if p.dark { p.bg } else { egui::Color32::WHITE });
                    ui.label(egui::RichText::new(APP_NAME).font(theme::semibold(15.5)).color(p.text));
                    ui.add_space(14.0);

                    // Search
                    let search_w = (ui.available_width() * 0.34).clamp(200.0, 420.0);
                    egui::Frame::new()
                        .fill(if p.dark { p.bg } else { p.card })
                        .stroke(egui::Stroke::new(1.0, p.border))
                        .corner_radius(egui::CornerRadius::same(9))
                        .inner_margin(egui::Margin::symmetric(10, 4))
                        .show(ui, |ui| {
                            ui.set_width(search_w);
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(ph::MAGNIFYING_GLASS).color(p.text_muted));
                                let resp = ui.add(
                                    egui::TextEdit::singleline(&mut self.search)
                                        .hint_text("Search devices, VID:PID, drive, serial…")
                                        .frame(egui::Frame::NONE)
                                        .desired_width(search_w - 70.0),
                                );
                                if self.focus_search {
                                    resp.request_focus();
                                    self.focus_search = false;
                                }
                                if resp.changed() && !self.search.is_empty() {
                                    self.scroll_to_selected = true;
                                }
                                if !self.search.is_empty() {
                                    if widgets::icon_button(ui, &p, ph::X, "Clear search").clicked() {
                                        self.search.clear();
                                    }
                                } else {
                                    ui.label(egui::RichText::new("Ctrl F").size(11.0).color(p.text_faint));
                                }
                            });
                        });
                    ui.add_space(6.0);
                    let mut qf = self.quick_filter;
                    let problems = self.insights.iter().filter(|i| i.severity >= insights::Severity::Warning).count();
                    let prob_label = if problems > 0 { format!("{} Problems {problems}", ph::WARNING) } else { format!("{} Problems", ph::WARNING) };
                    widgets::segmented(
                        ui,
                        &p,
                        "quick-filter",
                        &mut qf,
                        &[
                            (QuickFilter::All, "All"),
                            (QuickFilter::Problems, prob_label.as_str()),
                            (QuickFilter::SuperSpeed, "USB 3"),
                            (QuickFilter::Storage, "Storage"),
                            (QuickFilter::Input, "Input"),
                        ],
                    );
                    self.quick_filter = qf;

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.menu_button(egui::RichText::new(ph::DOTS_THREE_VERTICAL).size(18.0), |ui| self.main_menu(ui));
                        let theme_icon = if p.dark { ph::SUN } else { ph::MOON };
                        if widgets::icon_button(ui, &p, theme_icon, "Toggle light/dark (Ctrl+Shift+L)").clicked() {
                            self.dispatch(Action::ToggleTheme);
                        }
                        if widgets::icon_button(ui, &p, ph::COMMAND, "Command palette (Ctrl+K)").clicked() {
                            self.dispatch(Action::OpenPalette);
                        }
                        let empty_icon = if self.settings.show_empty_ports { ph::CIRCLE_DASHED } else { ph::PLUGS_CONNECTED };
                        if widgets::icon_button(ui, &p, empty_icon, if self.settings.show_empty_ports { "Hide empty ports" } else { "Show empty ports" }).clicked() {
                            self.dispatch(Action::ToggleEmptyPorts);
                        }
                        // Refresh with spinning icon while scanning.
                        let (r, resp) = ui.allocate_exact_size(egui::vec2(30.0, 30.0), egui::Sense::click());
                        let resp = resp.on_hover_text("Refresh (F5)").on_hover_cursor(egui::CursorIcon::PointingHand);
                        if resp.hovered() {
                            ui.painter().rect_filled(r, egui::CornerRadius::same(8), p.card_hover);
                        }
                        if self.scanning {
                            let t = ui.input(|i| i.time) as f32;
                            let c = r.center();
                            let rad = 7.5;
                            let start = t * 6.0;
                            let pts: Vec<egui::Pos2> = (0..=24)
                                .map(|k| {
                                    let a = start + k as f32 / 24.0 * std::f32::consts::PI * 1.5;
                                    c + egui::vec2(a.cos(), a.sin()) * rad
                                })
                                .collect();
                            ui.painter().add(egui::Shape::line(pts, egui::Stroke::new(2.0, p.accent)));
                        } else {
                            ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, ph::ARROWS_CLOCKWISE, egui::FontId::proportional(17.0), if resp.hovered() { p.text } else { p.text_muted });
                        }
                        if resp.clicked() {
                            self.dispatch(Action::Refresh);
                        }
                        // Live indicator
                        if self.is_live() {
                            let live = self.settings.auto_refresh;
                            let color = if live { p.ok } else { p.text_faint };
                            let (r, resp) = ui.allocate_exact_size(egui::vec2(62.0, 26.0), egui::Sense::click());
                            let resp = resp
                                .on_hover_text(if live { "Watching for device changes – click to pause" } else { "Paused – click to resume live updates" })
                                .on_hover_cursor(egui::CursorIcon::PointingHand);
                            ui.painter().rect_filled(r, egui::CornerRadius::same(255), p.soft(color));
                            let dot_c = egui::pos2(r.left() + 13.0, r.center().y);
                            if live {
                                let t = ui.input(|i| i.time) as f32;
                                let pulse = (t * 2.0).sin() * 0.5 + 0.5;
                                ui.painter().circle_filled(dot_c, 3.5 + pulse * 3.0, theme::with_alpha(color, 0.25 * (1.0 - pulse)));
                                ui.ctx().request_repaint_after(Duration::from_millis(50));
                            }
                            ui.painter().circle_filled(dot_c, 3.5, color);
                            ui.painter().text(egui::pos2(r.left() + 22.0, r.center().y), egui::Align2::LEFT_CENTER, if live { "Live" } else { "Paused" }, egui::FontId::proportional(12.0), color);
                            if resp.clicked() {
                                self.dispatch(Action::ToggleLive);
                            }
                        } else if ui.add(egui::Button::new(egui::RichText::new(format!("{}  Back to live", ph::PULSE)).color(p.accent))).clicked() {
                            self.dispatch(Action::BackToLive);
                        }
                    });
                });
            });
    }

    fn main_menu(&mut self, ui: &mut egui::Ui) {
        ui.set_min_width(250.0);
        let item = |ui: &mut egui::Ui, icon: &str, text: &str, sc: &str| -> bool {
            ui.add(egui::Button::new(format!("{icon}   {text}")).shortcut_text(sc)).clicked()
        };
        if item(ui, ph::FOLDER_OPEN, "Open snapshot…", "Ctrl+O") {
            self.dispatch(Action::OpenSnapshot);
        }
        if item(ui, ph::FLOPPY_DISK, "Save snapshot…", "Ctrl+S") {
            self.dispatch(Action::SaveSnapshot);
        }
        if item(ui, ph::GIT_DIFF, "Compare with snapshot…", "") {
            self.dispatch(Action::Compare);
        }
        ui.separator();
        if item(ui, ph::EXPORT, "Export HTML report…", "Ctrl+E") {
            self.dispatch(Action::ExportHtml);
        }
        if item(ui, ph::EXPORT, "Export text report…", "") {
            self.dispatch(Action::ExportText);
        }
        if item(ui, ph::COPY, "Copy full report", "Ctrl+Shift+C") {
            self.dispatch(Action::CopyFullReport);
        }
        ui.separator();
        if item(ui, ph::ROWS, "Expand all", "") {
            self.dispatch(Action::ExpandAll);
        }
        if item(ui, ph::LIST, "Collapse all", "") {
            self.dispatch(Action::CollapseAll);
        }
        let child_label = if self.settings.show_child_devices { "Hide Windows child devices" } else { "Show Windows child devices" };
        if item(ui, ph::STACK, child_label, "") {
            self.dispatch(Action::ToggleChildDevices);
        }
        if item(ui, ph::PULSE, "Activity & insights panel", "Ctrl+J") {
            self.bottom_open = !self.bottom_open;
        }
        ui.separator();
        if !platform::is_admin() && item(ui, ph::SHIELD_CHECK, "Restart as administrator", "") {
            self.dispatch(Action::RunAsAdmin);
        }
        if item(ui, ph::WRENCH, "Device Manager", "") {
            self.dispatch(Action::DeviceManager);
        }
        if item(ui, ph::HARD_DRIVES, "Disk Management", "") {
            self.dispatch(Action::OpenPath("diskmgmt.msc".into()));
        }
        if item(ui, ph::GEAR, "Settings", "Ctrl+,") {
            self.dispatch(Action::OpenSettings);
        }
        if item(ui, ph::INFO, &format!("About {APP_NAME}"), "") {
            self.dispatch(Action::About);
        }
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        let p = self.p;
        egui::Panel::bottom("status")
            .exact_size(26.0)
            .frame(egui::Frame::new().fill(p.panel).inner_margin(egui::Margin::symmetric(12, 4)).stroke(egui::Stroke::new(1.0, p.border)))
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    let small = |t: String, c: egui::Color32| egui::RichText::new(t).size(11.5).color(c);
                    match &self.source {
                        Source::Live => {
                            ui.label(small(format!("{}  Live system", ph::PULSE), p.text_muted));
                        }
                        Source::File(f) => {
                            ui.label(small(format!("{}  Snapshot: {}", ph::FLOPPY_DISK, f.file_name().map(|x| x.to_string_lossy().to_string()).unwrap_or_default()), p.warn));
                        }
                        Source::Demo => {
                            ui.label(small(format!("{}  Demo data", ph::SPARKLE), p.warn));
                        }
                    }
                    if let Some(s) = &self.snapshot {
                        let devs = self.flat.nodes.iter().filter(|n| matches!(n.kind, tree::NodeKind::Device | tree::NodeKind::Hub)).count();
                        ui.label(small(format!("·  {} controllers  ·  {devs} devices  ·  scanned in {} ms  ·  {}", s.controllers.len(), s.scan_ms, s.taken_at), p.text_faint));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if platform::is_admin() {
                            ui.label(small(format!("{}  Administrator", ph::SHIELD_CHECK), p.ok));
                        } else if ui
                            .add(egui::Label::new(small(format!("{}  Standard user – click to elevate", ph::SHIELD_WARNING), p.text_muted)).sense(egui::Sense::click()))
                            .on_hover_text("Restart, cycle port and enable/disable need administrator rights")
                            .clicked()
                        {
                            self.dispatch(Action::RunAsAdmin);
                        }
                        let n_err = self.insights.iter().filter(|i| i.severity == insights::Severity::Error).count();
                        let n_warn = self.insights.iter().filter(|i| i.severity == insights::Severity::Warning).count();
                        let n_info = self.insights.iter().filter(|i| i.severity == insights::Severity::Info).count();
                        let txt = format!("{} {n_err}   {} {n_warn}   {} {n_info}", ph::X_CIRCLE, ph::WARNING, ph::LIGHTBULB);
                        let col = if n_err > 0 { p.error } else if n_warn > 0 { p.warn } else { p.text_muted };
                        if ui.add(egui::Label::new(small(txt, col)).sense(egui::Sense::click())).on_hover_text("Show insights (Ctrl+J)").clicked() {
                            self.bottom_open = true;
                            self.bottom_tab = BottomTab::Insights;
                        }
                        if !self.events.is_empty()
                            && ui.add(egui::Label::new(small(format!("{}  {} events", ph::CLOCK_COUNTER_CLOCKWISE, self.events.len()), p.text_muted)).sense(egui::Sense::click())).clicked()
                        {
                            self.bottom_open = true;
                            self.bottom_tab = BottomTab::Activity;
                        }
                    });
                });
            });
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        let p = self.p;
        // Confirmation
        if let Some(c) = &self.confirm {
            let (title, body) = (c.title.clone(), c.body.clone());
            let mut decision: Option<bool> = None;
            egui::Modal::new(egui::Id::new("confirm")).show(ctx, |ui| {
                ui.set_width(380.0);
                ui.horizontal(|ui| {
                    widgets::icon_tile(ui, &p, ph::WARNING, 36.0, p.warn);
                    ui.label(egui::RichText::new(&title).font(theme::semibold(16.0)));
                });
                ui.add_space(6.0);
                ui.label(egui::RichText::new(&body).color(p.text_muted));
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(egui::Button::new(egui::RichText::new("Continue").color(egui::Color32::WHITE)).fill(p.accent)).clicked() {
                            decision = Some(true);
                        }
                        if ui.button("Cancel").clicked() {
                            decision = Some(false);
                        }
                    });
                });
                if ui.input(|i| i.key_pressed(Key::Escape)) {
                    decision = Some(false);
                }
            });
            if let Some(d) = decision {
                let c = self.confirm.take().unwrap();
                if d {
                    self.handle(c.action, ctx, true);
                }
            }
        }
        // Rename
        if let Some((id, mut text)) = self.rename.take() {
            let mut done: Option<bool> = None;
            egui::Modal::new(egui::Id::new("rename")).show(ctx, |ui| {
                ui.set_width(360.0);
                ui.label(egui::RichText::new("Nickname").font(theme::semibold(16.0)));
                ui.label(egui::RichText::new("Shown instead of the device name. Stored per device (VID:PID + serial).").color(p.text_muted).size(12.0));
                ui.add_space(6.0);
                let r = ui.add(egui::TextEdit::singleline(&mut text).desired_width(f32::INFINITY));
                r.request_focus();
                if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                    done = Some(true);
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Reset to default").clicked() {
                        text.clear();
                        done = Some(true);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(egui::Button::new(egui::RichText::new("Save").color(egui::Color32::WHITE)).fill(p.accent)).clicked() {
                            done = Some(true);
                        }
                        if ui.button("Cancel").clicked() {
                            done = Some(false);
                        }
                    });
                });
                if ui.input(|i| i.key_pressed(Key::Escape)) {
                    done = Some(false);
                }
            });
            match done {
                None => self.rename = Some((id, text)),
                Some(false) => {}
                Some(true) => {
                    if let Some(key) = self.flat.get(&id).and_then(|n| self.device_key(n)) {
                        let label = self.flat.get(&id).map(|n| n.label.clone()).unwrap_or_default();
                        if text.trim().is_empty() || text.trim() == label {
                            self.settings.nicknames.remove(&key);
                        } else {
                            self.settings.nicknames.insert(key, text.trim().to_string());
                        }
                        self.settings.save();
                        self.rebuild();
                    }
                }
            }
        }
        // Compare results
        if self.compare.is_some() {
            let mut open = true;
            let mut jump = None;
            let c = self.compare.as_ref().unwrap();
            egui::Window::new(format!("{}  Compare with {}", ph::GIT_DIFF, c.name))
                .open(&mut open)
                .default_width(460.0)
                .collapsible(false)
                .show(ctx, |ui| {
                    if c.changes.is_empty() {
                        ui.label(egui::RichText::new("No differences – the same devices are connected at the same ports.").color(p.text_muted));
                    }
                    egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                        for ch in &c.changes {
                            let (icon, color, verb) = match ch.kind {
                                ChangeKind::Arrived => (ph::PLUGS_CONNECTED, p.ok, "new since snapshot"),
                                ChangeKind::Removed => (ph::PLUG, p.error, "missing now"),
                                ChangeKind::ProblemAppeared(_) => (ph::WARNING, p.warn, "has a new problem"),
                                ChangeKind::ProblemCleared => (ph::CHECK_CIRCLE, p.ok, "problem resolved"),
                            };
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(icon).color(color));
                                ui.label(egui::RichText::new(&ch.name).strong());
                                ui.label(egui::RichText::new(verb).color(p.text_muted));
                                if ch.kind != ChangeKind::Removed && ui.small_button("Show").clicked() {
                                    jump = Some(ch.node_id.clone());
                                }
                            });
                        }
                    });
                });
            if let Some(j) = jump {
                self.select(j);
            }
            if !open {
                self.compare = None;
            }
        }
        if self.settings_open {
            self.settings_window(ctx);
        }
        if self.about_open {
            let mut open = true;
            egui::Window::new(format!("About {APP_NAME}")).open(&mut open).collapsible(false).resizable(false).show(ctx, |ui| {
                ui.horizontal(|ui| {
                    widgets::icon_tile(ui, &p, ph::TREE_STRUCTURE, 48.0, p.accent);
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new(APP_NAME).font(theme::semibold(18.0)));
                        ui.label(egui::RichText::new(format!("Version {}", env!("CARGO_PKG_VERSION"))).color(p.text_muted));
                    });
                });
                ui.add_space(8.0);
                ui.label("A modern USB topology explorer, inspired by Uwe Sieber's USB Device Tree Viewer.");
                ui.label(egui::RichText::new(format!("usb.ids database {}", crate::usbids::db().version)).color(p.text_muted).size(12.0));
                ui.add_space(6.0);
                ui.label(egui::RichText::new("Shortcuts").strong());
                for (k, v) in [
                    ("F5 / Ctrl+R", "Refresh"),
                    ("Ctrl+K", "Command palette"),
                    ("Ctrl+F", "Search"),
                    ("↑ ↓ ← →", "Navigate the tree"),
                    ("F2", "Rename device"),
                    ("Ctrl+C", "Copy selected report"),
                    ("Ctrl+J", "Activity & insights"),
                    ("Ctrl+S / Ctrl+O", "Save / open snapshot"),
                    ("Ctrl+E", "Export HTML report"),
                ] {
                    ui.horizontal(|ui| {
                        ui.add_sized([130.0, 18.0], egui::Label::new(egui::RichText::new(k).monospace().color(p.accent)));
                        ui.label(v);
                    });
                }
            });
            self.about_open = open;
        }
    }

    fn settings_window(&mut self, ctx: &egui::Context) {
        let p = self.p;
        let mut open = true;
        let mut changed = false;
        let mut retheme = false;
        egui::Window::new(format!("{}  Settings", ph::GEAR)).open(&mut open).collapsible(false).default_width(420.0).show(ctx, |ui| {
            ui.label(egui::RichText::new("Appearance").font(theme::semibold(14.0)));
            ui.horizontal(|ui| {
                ui.label("Theme");
                let mut t = self.settings.theme;
                if widgets::segmented(ui, &p, "theme", &mut t, &[(ThemeMode::System, "System"), (ThemeMode::Dark, "Dark"), (ThemeMode::Light, "Light")]) {
                    self.settings.theme = t;
                    retheme = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label("Accent");
                for a in theme::Accent::ALL {
                    let c = a.color(p.dark);
                    let (r, resp) = ui.allocate_exact_size(egui::vec2(22.0, 22.0), egui::Sense::click());
                    ui.painter().circle_filled(r.center(), 9.0, c);
                    if self.settings.accent == a {
                        ui.painter().circle_stroke(r.center(), 11.0, egui::Stroke::new(2.0, p.text));
                    }
                    if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                        self.settings.accent = a;
                        retheme = true;
                    }
                }
            });
            changed |= ui.checkbox(&mut self.settings.compact_rows, "Compact tree rows").changed();
            changed |= ui.checkbox(&mut self.settings.show_port_numbers, "Show port numbers in the tree").changed();
            ui.add_space(8.0);
            ui.label(egui::RichText::new("Tree content").font(theme::semibold(14.0)));
            let a = ui.checkbox(&mut self.settings.show_empty_ports, "Show empty ports").changed();
            let b = ui.checkbox(&mut self.settings.show_child_devices, "Show Windows child devices (disks, HID, COM…)").changed();
            if a || b {
                self.rebuild();
                changed = true;
            }
            ui.add_space(8.0);
            ui.label(egui::RichText::new("Behavior").font(theme::semibold(14.0)));
            changed |= ui.checkbox(&mut self.settings.auto_refresh, "Refresh automatically when devices change").changed();
            changed |= ui.checkbox(&mut self.settings.notify_arrivals, "Show notifications for connected / removed devices").changed();
            changed |= ui.checkbox(&mut self.settings.jump_to_new, "Jump to newly connected devices").changed();
            changed |= ui.checkbox(&mut self.settings.confirm_actions, "Ask before restart / disable / cycle port").changed();
            if ui.checkbox(&mut self.settings.always_on_top, "Keep window always on top").changed() {
                changed = true;
                let level = if self.settings.always_on_top { egui::WindowLevel::AlwaysOnTop } else { egui::WindowLevel::Normal };
                ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(level));
            }
            changed |= ui.checkbox(&mut self.settings.hexdumps_in_reports, "Include hex dumps in reports").changed();
            ui.add_space(8.0);
            if !self.settings.nicknames.is_empty() {
                ui.label(egui::RichText::new(format!("{} device nicknames stored", self.settings.nicknames.len())).color(p.text_muted));
                if ui.button("Clear all nicknames").clicked() {
                    self.settings.nicknames.clear();
                    self.rebuild();
                    changed = true;
                }
            }
        });
        if retheme {
            self.apply_theme(ctx);
            changed = true;
        }
        if changed {
            self.settings.save();
        }
        self.settings_open = open;
    }
}
