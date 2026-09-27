//! Computer information, elevation and shell helpers.

use windows::core::{w, PCWSTR};
use windows::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ};
use windows::Win32::UI::Shell::{IsUserAnAdmin, ShellExecuteW};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

use super::util::{from_wide, wide};
use crate::model::ComputerInfo;

pub fn now_string() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

fn reg_string(value: PCWSTR) -> String {
    let mut buf = [0u16; 256];
    let mut size = (buf.len() * 2) as u32;
    unsafe {
        let r = RegGetValueW(
            HKEY_LOCAL_MACHINE,
            w!(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion"),
            value,
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr() as *mut _),
            Some(&mut size),
        );
        if r.is_ok() {
            from_wide(&buf)
        } else {
            String::new()
        }
    }
}

pub fn is_admin() -> bool {
    unsafe { IsUserAnAdmin().as_bool() }
}

pub fn computer_info() -> ComputerInfo {
    let mut product = reg_string(w!("ProductName"));
    let build = reg_string(w!("CurrentBuild"));
    // ProductName still says "Windows 10" on Windows 11.
    if build.parse::<u32>().unwrap_or(0) >= 22000 {
        product = product.replace("Windows 10", "Windows 11");
    }
    let display = reg_string(w!("DisplayVersion"));
    ComputerInfo {
        name: std::env::var("COMPUTERNAME").unwrap_or_default(),
        os: format!("{product} {display} (build {build})")
            .trim()
            .to_string(),
        user: std::env::var("USERNAME").unwrap_or_default(),
        is_admin: is_admin(),
    }
}

fn shell(verb: &str, file: &str, params: &str) -> Result<(), String> {
    let (v, f, p) = (wide(verb), wide(file), wide(params));
    let r = unsafe {
        ShellExecuteW(
            None,
            PCWSTR(v.as_ptr()),
            PCWSTR(f.as_ptr()),
            PCWSTR(p.as_ptr()),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    if r.0 as isize > 32 {
        Ok(())
    } else {
        Err(format!("ShellExecute failed ({})", r.0 as isize))
    }
}

/// Restarts this program elevated; the caller should exit on success.
pub fn relaunch_as_admin() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    shell("runas", &exe.to_string_lossy(), "")
}

pub fn open_path(path: &str) -> Result<(), String> {
    shell("open", path, "")
}

pub fn open_device_properties(instance_id: &str) -> Result<(), String> {
    shell(
        "open",
        "rundll32.exe",
        &format!("devmgr.dll,DeviceProperties_RunDLL /DeviceID \"{instance_id}\""),
    )
}

pub fn open_device_manager() -> Result<(), String> {
    shell("open", "devmgmt.msc", "")
}
