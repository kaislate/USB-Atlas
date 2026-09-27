//! Hot-plug notifications via CM_Register_Notification (no window needed).

use std::ffi::c_void;
use std::sync::mpsc::Sender;

use windows::Win32::Devices::DeviceAndDriverInstallation::*;
use windows::Win32::Devices::Usb::{GUID_DEVINTERFACE_USB_DEVICE, GUID_DEVINTERFACE_USB_HUB};

pub struct Watcher {
    handles: Vec<HCMNOTIFICATION>,
    _ctx: Box<Sink>,
}

struct Sink {
    tx: Sender<()>,
    wake: Box<dyn Fn() + Send + Sync>,
}

/// Device-instance events arrive for every device in the system; only USB
/// related instances (`USB\...`, `USBSTOR\...`) should trigger a rescan.
unsafe fn is_relevant(
    action: CM_NOTIFY_ACTION,
    data: *const CM_NOTIFY_EVENT_DATA,
    size: u32,
) -> bool {
    if data.is_null() || (*data).FilterType != CM_NOTIFY_FILTER_TYPE_DEVICEINSTANCE {
        return true;
    }
    if action != CM_NOTIFY_ACTION_DEVICEINSTANCESTARTED
        && action != CM_NOTIFY_ACTION_DEVICEINSTANCEREMOVED
    {
        return false;
    }
    // InstanceId (NUL-terminated UTF-16) starts at offset 8.
    let bytes = std::slice::from_raw_parts(data as *const u8, size as usize);
    let id = super::util::wide_at(bytes, 8).to_uppercase();
    id.starts_with("USB")
}

unsafe extern "system" fn callback(
    _h: HCMNOTIFICATION,
    context: *const c_void,
    action: CM_NOTIFY_ACTION,
    data: *const CM_NOTIFY_EVENT_DATA,
    size: u32,
) -> u32 {
    if !is_relevant(action, data, size) {
        return 0;
    }
    if let Some(sink) = (context as *const Sink).as_ref() {
        let _ = sink.tx.send(());
        (sink.wake)();
    }
    0
}

impl Watcher {
    /// Every USB device / hub arrival or removal sends `()` on `tx` and calls `wake`.
    pub fn start(tx: Sender<()>, wake: impl Fn() + Send + Sync + 'static) -> Option<Self> {
        let ctx = Box::new(Sink {
            tx,
            wake: Box::new(wake),
        });
        let mut handles = Vec::new();
        for guid in [GUID_DEVINTERFACE_USB_DEVICE, GUID_DEVINTERFACE_USB_HUB] {
            let mut filter = CM_NOTIFY_FILTER {
                cbSize: std::mem::size_of::<CM_NOTIFY_FILTER>() as u32,
                FilterType: CM_NOTIFY_FILTER_TYPE_DEVICEINTERFACE,
                ..Default::default()
            };
            filter.u.DeviceInterface.ClassGuid = guid;
            let mut h = HCMNOTIFICATION::default();
            let cr = unsafe {
                CM_Register_Notification(
                    &filter,
                    Some(&*ctx as *const Sink as *const c_void),
                    Some(callback),
                    &mut h,
                )
            };
            if cr == CR_SUCCESS {
                handles.push(h);
            }
        }
        // Device instance events also catch problem-code changes / restarts.
        let mut filter = CM_NOTIFY_FILTER {
            cbSize: std::mem::size_of::<CM_NOTIFY_FILTER>() as u32,
            FilterType: CM_NOTIFY_FILTER_TYPE_DEVICEINSTANCE,
            Flags: CM_NOTIFY_FILTER_FLAG_ALL_DEVICE_INSTANCES,
            ..Default::default()
        };
        filter.Reserved = 0;
        let mut h = HCMNOTIFICATION::default();
        if unsafe {
            CM_Register_Notification(
                &filter,
                Some(&*ctx as *const Sink as *const c_void),
                Some(callback),
                &mut h,
            )
        } == CR_SUCCESS
        {
            handles.push(h);
        }
        if handles.is_empty() {
            return None;
        }
        Some(Self { handles, _ctx: ctx })
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        for h in self.handles.drain(..) {
            unsafe {
                let _ = CM_Unregister_Notification(h);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn registers_notifications() {
        let (tx, _rx) = std::sync::mpsc::channel();
        let w = super::Watcher::start(tx, || {});
        assert!(
            w.is_some_and(|w| w.handles.len() == 3),
            "all three CM notification filters should register"
        );
    }
}
