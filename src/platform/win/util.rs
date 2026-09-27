//! Small helpers around raw Win32 buffers.

use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, GENERIC_WRITE, HANDLE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows::Win32::System::IO::DeviceIoControl;

/// RAII wrapper that closes a handle on drop.
pub struct Handle(pub HANDLE);

impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn from_wide(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

/// Decodes a NUL-terminated UTF-16 string stored in a byte buffer.
pub fn wide_at(buf: &[u8], offset: usize) -> String {
    if offset >= buf.len() {
        return String::new();
    }
    let units: Vec<u16> = buf[offset..]
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    from_wide(&units)
}

/// Splits a REG_MULTI_SZ style UTF-16 list.
pub fn multi_sz(units: &[u16]) -> Vec<String> {
    units
        .split(|&c| c == 0)
        .filter(|s| !s.is_empty())
        .map(String::from_utf16_lossy)
        .collect()
}

pub fn u16_at(b: &[u8], o: usize) -> u16 {
    b.get(o..o + 2)
        .map(|s| u16::from_le_bytes([s[0], s[1]]))
        .unwrap_or(0)
}

pub fn u32_at(b: &[u8], o: usize) -> u32 {
    b.get(o..o + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
        .unwrap_or(0)
}

pub fn open_device(path: &str) -> windows::core::Result<Handle> {
    let w = wide(path);
    let h = unsafe {
        CreateFileW(
            PCWSTR(w.as_ptr()),
            GENERIC_WRITE.0,
            FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_FLAGS_AND_ATTRIBUTES(0),
            None,
        )?
    };
    Ok(Handle(h))
}

/// Issues an IOCTL where the same buffer is used for input and output.
/// `input` is copied to the start of a buffer of `size` bytes.
pub fn ioctl(h: &Handle, code: u32, input: &[u8], size: usize) -> windows::core::Result<Vec<u8>> {
    let mut buf = vec![0u8; size.max(input.len())];
    buf[..input.len()].copy_from_slice(input);
    let mut returned = 0u32;
    unsafe {
        DeviceIoControl(
            h.0,
            code,
            Some(buf.as_ptr() as *const _),
            buf.len() as u32,
            Some(buf.as_mut_ptr() as *mut _),
            buf.len() as u32,
            Some(&mut returned),
            None,
        )?;
    }
    buf.truncate(returned as usize);
    Ok(buf)
}

/// Two-pass query for the variable-length USB name structures. `name_offset`
/// is where the UTF-16 name starts, `len_offset` where `ActualLength` is.
pub fn ioctl_name(
    h: &Handle,
    code: u32,
    input: &[u8],
    len_offset: usize,
    name_offset: usize,
) -> Option<String> {
    let first = ioctl(h, code, input, name_offset + 2).ok()?;
    let actual = u32_at(&first, len_offset) as usize;
    if actual <= name_offset {
        return None;
    }
    let full = ioctl(h, code, input, actual).ok()?;
    let s = wide_at(&full, name_offset);
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// Converts a FILETIME (100 ns ticks since 1601) to `YYYY-MM-DD`.
pub fn filetime_to_date(ft: u64) -> String {
    let days = (ft / 10_000_000 / 86_400) as i64 - 134_774; // days since 1970-01-01
                                                            // Civil-from-days (Howard Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filetime_dates() {
        // 2006-06-21 00:00:00 UTC
        assert_eq!(filetime_to_date(127_953_216_000_000_000), "2006-06-21");
        // 1970-01-01
        assert_eq!(filetime_to_date(116_444_736_000_000_000), "1970-01-01");
    }

    #[test]
    fn multi_sz_splits() {
        let v: Vec<u16> = "a\0bc\0\0".encode_utf16().collect();
        assert_eq!(multi_sz(&v), vec!["a", "bc"]);
    }
}
