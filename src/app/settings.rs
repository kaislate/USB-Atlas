//! User settings persisted as JSON in %APPDATA%.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::theme::{Accent, LinkColors, ThemeMode};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme: ThemeMode,
    pub accent: Accent,
    pub show_empty_ports: bool,
    pub show_child_devices: bool,
    pub auto_refresh: bool,
    pub notify_arrivals: bool,
    pub jump_to_new: bool,
    pub hexdumps_in_reports: bool,
    pub compact_rows: bool,
    pub show_port_numbers: bool,
    pub confirm_actions: bool,
    pub always_on_top: bool,
    pub map_view: bool,
    pub map_motion: bool,
    pub map_show_empty: bool,
    pub map_legend: bool,
    pub link_colors: LinkColors,
    pub physical_tree: bool,
    pub tree_width: f32,
    /// Device nicknames keyed by `VID:PID:serial` (or `VID:PID@location`).
    pub nicknames: HashMap<String, String>,
    pub pinned: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: ThemeMode::System,
            accent: Accent::Indigo,
            show_empty_ports: true,
            show_child_devices: false,
            auto_refresh: true,
            notify_arrivals: true,
            jump_to_new: false,
            hexdumps_in_reports: true,
            compact_rows: false,
            show_port_numbers: true,
            confirm_actions: true,
            always_on_top: false,
            map_view: false,
            map_motion: true,
            map_show_empty: false,
            map_legend: true,
            link_colors: LinkColors::default(),
            physical_tree: false,
            tree_width: 420.0,
            nicknames: HashMap::new(),
            pinned: Vec::new(),
        }
    }
}

fn path() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from))?;
    Some(base.join(super::APP_NAME).join("settings.json"))
}

impl Settings {
    pub fn load() -> Self {
        path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        if let Some(p) = path() {
            if let Some(dir) = p.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            if let Ok(s) = serde_json::to_string_pretty(self) {
                let _ = std::fs::write(p, s);
            }
        }
    }
}
