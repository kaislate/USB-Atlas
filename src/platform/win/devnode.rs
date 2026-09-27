//! Reading Device Manager information for a devnode (CfgMgr32).

use std::collections::HashMap;

use windows::core::{GUID, PCWSTR};
use windows::Win32::Devices::DeviceAndDriverInstallation::*;
use windows::Win32::Devices::Properties::*;
use windows::Win32::Foundation::DEVPROPKEY;
use windows::Win32::Storage::FileSystem::{
    GetDiskFreeSpaceExW, GetVolumeInformationW, GetVolumeNameForVolumeMountPointW, GetVolumePathNamesForVolumeNameW,
};
use windows::Win32::System::Ioctl::GUID_DEVINTERFACE_VOLUME;
use windows::Win32::System::Registry::{RegCloseKey, RegQueryValueExW, HKEY, KEY_READ};

use super::util::{filetime_to_date, from_wide, multi_sz, wide};
use crate::model::{DevInfo, Volume};

const DEVPROP_TYPE_STRING_: u32 = 0x12;
const DEVPROP_TYPE_STRING_LIST_: u32 = 0x2012;
const DEVPROP_TYPE_UINT32_: u32 = 0x07;
const DEVPROP_TYPE_FILETIME_: u32 = 0x10;
const DEVPROP_TYPE_GUID_: u32 = 0x0D;

fn prop_raw(dn: u32, key: &DEVPROPKEY) -> Option<(u32, Vec<u8>)> {
    let mut ty = DEVPROPTYPE(0);
    let mut size = 0u32;
    unsafe {
        let _ = CM_Get_DevNode_PropertyW(dn, key, &mut ty, None, &mut size, 0);
        if size == 0 {
            return None;
        }
        let mut buf = vec![0u8; size as usize];
        let cr = CM_Get_DevNode_PropertyW(dn, key, &mut ty, Some(buf.as_mut_ptr()), &mut size, 0);
        if cr != CR_SUCCESS {
            return None;
        }
        buf.truncate(size as usize);
        Some((ty.0, buf))
    }
}

fn to_u16(b: &[u8]) -> Vec<u16> {
    b.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect()
}

fn guid_string(b: &[u8]) -> String {
    if b.len() < 16 {
        return String::new();
    }
    let d1 = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
    let d2 = u16::from_le_bytes([b[4], b[5]]);
    let d3 = u16::from_le_bytes([b[6], b[7]]);
    format!(
        "{{{d1:08X}-{d2:04X}-{d3:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}}}",
        b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]
    )
}

pub fn prop_string(dn: u32, key: &DEVPROPKEY) -> String {
    match prop_raw(dn, key) {
        Some((DEVPROP_TYPE_STRING_, b)) => from_wide(&to_u16(&b)),
        Some((DEVPROP_TYPE_STRING_LIST_, b)) => multi_sz(&to_u16(&b)).join(", "),
        Some((DEVPROP_TYPE_GUID_, b)) => guid_string(&b),
        Some((DEVPROP_TYPE_FILETIME_, b)) if b.len() >= 8 => {
            filetime_to_date(u64::from_le_bytes(b[..8].try_into().unwrap()))
        }
        _ => String::new(),
    }
}

fn prop_list(dn: u32, key: &DEVPROPKEY) -> Vec<String> {
    match prop_raw(dn, key) {
        Some((DEVPROP_TYPE_STRING_LIST_, b)) => multi_sz(&to_u16(&b)),
        Some((DEVPROP_TYPE_STRING_, b)) => vec![from_wide(&to_u16(&b))],
        _ => vec![],
    }
}

fn prop_u32(dn: u32, key: &DEVPROPKEY) -> Option<u32> {
    match prop_raw(dn, key) {
        Some((DEVPROP_TYPE_UINT32_, b)) if b.len() >= 4 => Some(u32::from_le_bytes(b[..4].try_into().unwrap())),
        _ => None,
    }
}

pub fn instance_id(dn: u32) -> String {
    let mut buf = [0u16; 512];
    unsafe {
        if CM_Get_Device_IDW(dn, &mut buf, 0) == CR_SUCCESS {
            from_wide(&buf)
        } else {
            String::new()
        }
    }
}

pub fn locate(instance_id: &str) -> Option<u32> {
    let w = wide(instance_id);
    let mut dn = 0u32;
    unsafe {
        if CM_Locate_DevNodeW(&mut dn, PCWSTR(w.as_ptr()), CM_LOCATE_DEVNODE_NORMAL) == CR_SUCCESS {
            Some(dn)
        } else {
            None
        }
    }
}

pub fn children(dn: u32) -> Vec<u32> {
    let mut out = Vec::new();
    let mut c = 0u32;
    unsafe {
        if CM_Get_Child(&mut c, dn, 0) != CR_SUCCESS {
            return out;
        }
        out.push(c);
        let mut cur = c;
        while CM_Get_Sibling(&mut c, cur, 0) == CR_SUCCESS {
            out.push(c);
            cur = c;
            if out.len() > 256 {
                break;
            }
        }
    }
    out
}

pub fn status(dn: u32) -> (u32, u32) {
    let mut st = CM_DEVNODE_STATUS_FLAGS(0);
    let mut prob = CM_PROB(0);
    unsafe {
        if CM_Get_DevNode_Status(&mut st, &mut prob, dn, 0) == CR_SUCCESS {
            (st.0, prob.0)
        } else {
            (0, 0)
        }
    }
}

fn com_port(dn: u32) -> String {
    unsafe {
        let mut key = HKEY::default();
        if CM_Open_DevNode_Key(dn, KEY_READ.0, 0, RegDisposition_OpenExisting, &mut key, CM_REGISTRY_HARDWARE) != CR_SUCCESS {
            return String::new();
        }
        let name = wide("PortName");
        let mut buf = [0u16; 64];
        let mut size = (buf.len() * 2) as u32;
        let r = RegQueryValueExW(key, PCWSTR(name.as_ptr()), None, None, Some(buf.as_mut_ptr() as *mut u8), Some(&mut size));
        let _ = RegCloseKey(key);
        if r.is_ok() { from_wide(&buf) } else { String::new() }
    }
}

fn id_list(filter_id: &str, flags: u32) -> Vec<String> {
    let w = wide(filter_id);
    unsafe {
        let mut len = 0u32;
        if CM_Get_Device_ID_List_SizeW(&mut len, PCWSTR(w.as_ptr()), flags) != CR_SUCCESS || len == 0 {
            return vec![];
        }
        let mut buf = vec![0u16; len as usize];
        if CM_Get_Device_ID_ListW(PCWSTR(w.as_ptr()), &mut buf, flags) != CR_SUCCESS {
            return vec![];
        }
        multi_sz(&buf)
    }
}

fn interface_list(guid: &GUID, device_id: &str) -> Vec<String> {
    let w = wide(device_id);
    unsafe {
        let mut len = 0u32;
        if CM_Get_Device_Interface_List_SizeW(&mut len, guid, PCWSTR(w.as_ptr()), CM_GET_DEVICE_INTERFACE_LIST_PRESENT)
            != CR_SUCCESS
            || len <= 1
        {
            return vec![];
        }
        let mut buf = vec![0u16; len as usize];
        if CM_Get_Device_Interface_ListW(guid, PCWSTR(w.as_ptr()), &mut buf, CM_GET_DEVICE_INTERFACE_LIST_PRESENT)
            != CR_SUCCESS
        {
            return vec![];
        }
        multi_sz(&buf)
    }
}

/// Volumes that belong to a disk / CD-ROM devnode (via removal relations).
fn volumes_of(disk_id: &str) -> Vec<Volume> {
    let mut out = Vec::new();
    for vol_id in id_list(disk_id, CM_GETIDLIST_FILTER_REMOVALRELATIONS) {
        if !vol_id.to_uppercase().starts_with("STORAGE\\VOLUME") {
            continue;
        }
        for iface in interface_list(&GUID_DEVINTERFACE_VOLUME, &vol_id) {
            if let Some(v) = volume_info(&iface) {
                out.push(v);
            }
        }
    }
    out
}

fn volume_info(interface_path: &str) -> Option<Volume> {
    let mount = wide(&format!("{interface_path}\\"));
    let mut name = [0u16; 64];
    unsafe {
        GetVolumeNameForVolumeMountPointW(PCWSTR(mount.as_ptr()), &mut name).ok()?;
    }
    let vol_name = from_wide(&name);
    let wn = wide(&vol_name);
    let mut paths = [0u16; 1024];
    let mut len = 0u32;
    let mount_points = unsafe {
        if GetVolumePathNamesForVolumeNameW(PCWSTR(wn.as_ptr()), Some(&mut paths), &mut len).is_ok() {
            multi_sz(&paths[..(len as usize).min(paths.len())])
        } else {
            vec![]
        }
    };
    let mut label = [0u16; 261];
    let mut fs = [0u16; 261];
    let (mut free, mut total) = (0u64, 0u64);
    unsafe {
        let _ = GetVolumeInformationW(PCWSTR(wn.as_ptr()), Some(&mut label), None, None, None, Some(&mut fs));
        let _ = GetDiskFreeSpaceExW(PCWSTR(wn.as_ptr()), None, Some(&mut total), Some(&mut free));
    }
    Some(Volume {
        mount_points,
        label: from_wide(&label),
        file_system: from_wide(&fs),
        total_bytes: total,
        free_bytes: free,
        volume_name: vol_name,
    })
}

/// Reads everything Device Manager knows about a devnode.
/// `depth` limits how deep child devnodes are followed.
pub fn read(dn: u32, depth: u32) -> DevInfo {
    let (st, prob) = status(dn);
    let id = instance_id(dn);
    let class = prop_string(dn, &DEVPKEY_Device_Class);
    let mut info = DevInfo {
        friendly_name: prop_string(dn, &DEVPKEY_Device_FriendlyName),
        description: prop_string(dn, &DEVPKEY_Device_DeviceDesc),
        bus_reported_desc: prop_string(dn, &DEVPKEY_Device_BusReportedDeviceDesc),
        manufacturer: prop_string(dn, &DEVPKEY_Device_Manufacturer),
        service: prop_string(dn, &DEVPKEY_Device_Service),
        class_guid: prop_string(dn, &DEVPKEY_Device_ClassGuid),
        driver_key: prop_string(dn, &DEVPKEY_Device_Driver),
        driver_version: prop_string(dn, &DEVPKEY_Device_DriverVersion),
        driver_date: prop_string(dn, &DEVPKEY_Device_DriverDate),
        driver_provider: prop_string(dn, &DEVPKEY_Device_DriverProvider),
        driver_inf: prop_string(dn, &DEVPKEY_Device_DriverInfPath),
        location_info: prop_string(dn, &DEVPKEY_Device_LocationInfo),
        location_paths: prop_list(dn, &DEVPKEY_Device_LocationPaths),
        hardware_ids: prop_list(dn, &DEVPKEY_Device_HardwareIds),
        compatible_ids: prop_list(dn, &DEVPKEY_Device_CompatibleIds),
        container_id: prop_string(dn, &DEVPKEY_Device_ContainerId),
        pdo_name: prop_string(dn, &DEVPKEY_Device_PDOName),
        upper_filters: prop_list(dn, &DEVPKEY_Device_UpperFilters),
        lower_filters: prop_list(dn, &DEVPKEY_Device_LowerFilters),
        enumerator: prop_string(dn, &DEVPKEY_Device_EnumeratorName),
        problem_code: if st & DN_HAS_PROBLEM.0 != 0 { prob } else { prop_u32(dn, &DEVPKEY_Device_ProblemCode).unwrap_or(0) },
        status_flags: st,
        power_state: String::new(),
        instance_id: id.clone(),
        class: class.clone(),
        ..Default::default()
    };
    if class.eq_ignore_ascii_case("Ports") {
        info.com_port = com_port(dn);
    }
    if class.eq_ignore_ascii_case("DiskDrive") || class.eq_ignore_ascii_case("CDROM") {
        info.volumes = volumes_of(&id);
    }
    if depth > 0 {
        info.children = children(dn).into_iter().map(|c| read(c, depth - 1)).collect();
    }
    info
}

/// Maps driver keys (`{36fc9e60-...}\0012`) to devnodes for every present device.
pub fn driver_key_map() -> HashMap<String, u32> {
    let mut map = HashMap::new();
    unsafe {
        let Ok(h) = SetupDiGetClassDevsW(None, PCWSTR::null(), None, DIGCF_ALLCLASSES | DIGCF_PRESENT) else {
            return map;
        };
        let mut i = 0;
        loop {
            let mut di = SP_DEVINFO_DATA { cbSize: std::mem::size_of::<SP_DEVINFO_DATA>() as u32, ..Default::default() };
            if SetupDiEnumDeviceInfo(h, i, &mut di).is_err() {
                break;
            }
            i += 1;
            let mut buf = [0u8; 512];
            let mut req = 0u32;
            if SetupDiGetDeviceRegistryPropertyW(h, &di, SPDRP_DRIVER, None, Some(&mut buf), Some(&mut req)).is_ok() {
                let key = from_wide(&to_u16(&buf[..(req as usize).min(buf.len())]));
                if !key.is_empty() {
                    map.insert(key.to_uppercase(), di.DevInst);
                }
            }
        }
        let _ = SetupDiDestroyDeviceInfoList(h);
    }
    map
}
