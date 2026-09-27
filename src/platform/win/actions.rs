//! Device actions: safe removal, restart, enable/disable and port cycling.

use windows::core::PCWSTR;
use windows::Win32::Devices::DeviceAndDriverInstallation::*;
use windows::Win32::Devices::Usb::IOCTL_USB_HUB_CYCLE_PORT;

use super::devnode;
use super::util::{from_wide, ioctl, open_device, u32_at, wide};

fn cr_err(what: &str, cr: CONFIGRET) -> String {
    let hint = match cr.0 {
        0x33 => " – administrator rights are required",
        0x17 => " – removal was vetoed",
        0x0D => " – device not found",
        _ => "",
    };
    format!("{what} failed (CONFIGRET 0x{:X}){hint}", cr.0)
}

fn veto_reason(t: i32) -> &'static str {
    match t {
        1 => "a legacy device",
        2 => "a pending close (open handles)",
        3 => "Windows (device is not removable)",
        4 => "a driver",
        5 => "an open handle from an application",
        6 => "a pending operation",
        7 => "a service",
        8 => "a device that cannot be ejected",
        9 => "a driver",
        10 => "a driver that is currently in use",
        12 => "insufficient rights",
        13 => "the device is still being installed",
        _ => "an unknown reason",
    }
}

/// Finds the device or its nearest ancestor that can be ejected.
fn removable_ancestor(dn: u32) -> u32 {
    let mut cur = dn;
    for _ in 0..8 {
        let (st, _) = devnode::status(cur);
        if st & DN_REMOVABLE.0 != 0 {
            return cur;
        }
        let mut parent = 0u32;
        if unsafe { CM_Get_Parent(&mut parent, cur, 0) } != CR_SUCCESS {
            break;
        }
        cur = parent;
    }
    dn
}

pub fn safely_remove(instance_id: &str) -> Result<String, String> {
    let dn = devnode::locate(instance_id).ok_or("Device not found – it may have been unplugged")?;
    let target = removable_ancestor(dn);
    let mut veto = PNP_VETO_TYPE(0);
    let mut name = [0u16; 260];
    let cr = unsafe { CM_Request_Device_EjectW(target, Some(&mut veto), Some(&mut name), 0) };
    if cr == CR_SUCCESS && veto.0 == 0 {
        Ok("It is now safe to remove the device.".into())
    } else if veto.0 != 0 {
        let who = from_wide(&name);
        Err(format!(
            "Windows refused to eject the device because of {}.{}",
            veto_reason(veto.0),
            if who.is_empty() {
                String::new()
            } else {
                format!("\nVetoed by: {who}")
            }
        ))
    } else {
        Err(cr_err("Eject", cr))
    }
}

pub fn set_enabled(instance_id: &str, enable: bool) -> Result<String, String> {
    let dn = devnode::locate(instance_id).ok_or("Device not found")?;
    let cr = unsafe {
        if enable {
            CM_Enable_DevNode(dn, 0)
        } else {
            CM_Disable_DevNode(dn, CM_DISABLE_UI_NOT_OK)
        }
    };
    if cr == CR_SUCCESS {
        Ok(if enable {
            "Device enabled."
        } else {
            "Device disabled."
        }
        .into())
    } else {
        Err(cr_err(if enable { "Enable" } else { "Disable" }, cr))
    }
}

/// Restarts a device the way Device Manager does (DICS_PROPCHANGE).
pub fn restart(instance_id: &str) -> Result<String, String> {
    unsafe {
        let h = SetupDiCreateDeviceInfoList(None, None).map_err(|e| e.to_string())?;
        let id = wide(instance_id);
        let mut di = SP_DEVINFO_DATA {
            cbSize: std::mem::size_of::<SP_DEVINFO_DATA>() as u32,
            ..Default::default()
        };
        let res = (|| {
            SetupDiOpenDeviceInfoW(h, PCWSTR(id.as_ptr()), None, 0, Some(&mut di))
                .map_err(|e| e.to_string())?;
            let params = SP_PROPCHANGE_PARAMS {
                ClassInstallHeader: SP_CLASSINSTALL_HEADER {
                    cbSize: std::mem::size_of::<SP_CLASSINSTALL_HEADER>() as u32,
                    InstallFunction: DIF_PROPERTYCHANGE,
                },
                StateChange: DICS_PROPCHANGE,
                Scope: DICS_FLAG_CONFIGSPECIFIC,
                HwProfile: 0,
            };
            SetupDiSetClassInstallParamsW(
                h,
                Some(&di),
                Some(&params.ClassInstallHeader),
                std::mem::size_of::<SP_PROPCHANGE_PARAMS>() as u32,
            )
            .map_err(|e| e.to_string())?;
            SetupDiCallClassInstaller(DIF_PROPERTYCHANGE, h, Some(&di)).map_err(|e| {
                if e.code().0 as u32 == 0x800F0228 || e.code().0 as u32 == 0x80070005 {
                    "Restart requires administrator rights.".to_string()
                } else {
                    format!("Restart failed: {e}")
                }
            })?;
            Ok("Device restarted.".to_string())
        })();
        let _ = SetupDiDestroyDeviceInfoList(h);
        res
    }
}

/// Power-cycles a hub port (the device re-enumerates). Needs admin.
pub fn cycle_port(hub_symbolic_name: &str, port: u32) -> Result<String, String> {
    let h = open_device(&format!(r"\\.\{hub_symbolic_name}"))
        .map_err(|e| format!("Cannot open hub: {e}"))?;
    let mut buf = [0u8; 8];
    buf[0..4].copy_from_slice(&port.to_le_bytes());
    match ioctl(&h, IOCTL_USB_HUB_CYCLE_PORT, &buf, 8) {
        Ok(b) if u32_at(&b, 4) == 0 => Ok(format!("Port {port} was cycled.")),
        Ok(b) => Err(format!("Hub reported status 0x{:X}", u32_at(&b, 4))),
        Err(e) => Err(if e.code().0 as u32 == 0x80070005 {
            "Cycling a port requires administrator rights.".into()
        } else {
            format!("Cycle port failed: {e}")
        }),
    }
}
