//! OS integration. Only Windows is implemented; other targets fall back to
//! demo data so the UI can still be developed and snapshots viewed.

#[cfg(windows)]
mod win;

#[cfg(windows)]
pub use win::*;

#[cfg(not(windows))]
mod fallback {
    use crate::model::{ComputerInfo, Snapshot};
    use std::sync::mpsc::Sender;

    pub fn scan() -> Snapshot {
        crate::demo::snapshot()
    }
    pub fn computer_info() -> ComputerInfo {
        ComputerInfo::default()
    }
    pub fn is_admin() -> bool {
        false
    }
    pub fn now_string() -> String {
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
    }
    fn unsupported() -> Result<String, String> {
        Err("Not supported on this platform".into())
    }
    pub fn safely_remove(_: &str) -> Result<String, String> {
        unsupported()
    }
    pub fn restart(_: &str) -> Result<String, String> {
        unsupported()
    }
    pub fn set_enabled(_: &str, _: bool) -> Result<String, String> {
        unsupported()
    }
    pub fn cycle_port(_: &str, _: u32) -> Result<String, String> {
        unsupported()
    }
    pub fn relaunch_as_admin() -> Result<(), String> {
        Err("Not supported".into())
    }
    pub fn open_path(_: &str) -> Result<(), String> {
        Err("Not supported".into())
    }
    pub fn open_device_properties(_: &str) -> Result<(), String> {
        Err("Not supported".into())
    }
    pub fn open_device_manager() -> Result<(), String> {
        Err("Not supported".into())
    }
    pub struct Watcher;
    impl Watcher {
        pub fn start(_: Sender<()>, _: impl Fn() + Send + Sync + 'static) -> Option<Self> {
            None
        }
    }
}

#[cfg(not(windows))]
pub use fallback::*;
