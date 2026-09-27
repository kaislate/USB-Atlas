# USB Atlas – design notes

## Goal

Re-implement everything useful in Uwe Sieber's USB Device Tree Viewer (V4.7.x),
then go further on aesthetics, interactivity and diagnostics.

## Language and toolkit decision

**Rust + egui/eframe (glow renderer)**, single static `.exe`.

| Option | Verdict |
|---|---|
| Rust + egui | ✔ Chosen. Direct, zero-cost access to Win32 (`windows` crate); immediate-mode UI makes rich custom painting (hex cross-highlighting, animated rows, port maps) cheap; one ~11 MB portable binary; no runtime. |
| Rust + Tauri (WebView2) | Great visuals, but two languages, IPC for every descriptor, WebView2 dependency, heavier. |
| Rust + Slint / iced | Viable; fewer ready-made widgets for dense technical UIs. |
| C# / WinUI 3 | Good native look, but P/Invoke-heavy for packed USB IOCTL structs; runtime/deployment overhead. |
| C++ / Win32 (like the original) | Maximum control, but slow to build a modern UI and memory-unsafe parsing of device-supplied data. |

Descriptor bytes come from untrusted devices, so parsing them in safe Rust
(bounds-checked `Reader`) is itself a feature.

Toolchain note: egui 0.36 requires Rust 1.95; the project pins **egui/eframe
0.35** so it builds with the installed Rust 1.94.

## Research summary

- **Feature inventory** of USBTreeView was built from the product page,
  history and the strings in the current executable (tree node types, icon
  states, every detail-pane block, menus, options, command line).
- **Win32 API reference** verified against `windows` 0.62.2 source with a
  compiled probe: struct sizes/offsets of the packed `usbioctl.h` structures,
  IOCTL codes (incl. the missing `IOCTL_USB_USER_REQUEST = 0x220438`), the
  usbview enumeration algorithm, devnode property access, eject / restart /
  cycle-port and `CM_Register_Notification`.

IOCTL buffers are parsed **by byte offset**, never cast to packed structs,
avoiding unaligned-reference and invalid-`bool` UB.

## Architecture

```
platform::scan() ──► Snapshot (serde) ──► tree::flatten ──► Flat (stable ids)
        ▲                  │                    │
 CM notifications          ├─► details::sections ──► UI cards / text / HTML
 (debounced 350 ms)        ├─► insights::analyze ──► callouts, filter, badges
                           └─► tree::diff(old,new) ──► activity log, toasts, ghosts
```

- The scanner runs on a background thread; the UI never blocks.
- The `Snapshot` model is the JSON file format, so saved reports can be
  reopened, dragged in, and compared against the live system.
- Node ids are `controller-instance-id/port-chain`, so selection, expansion,
  nicknames and diffing survive rescans.
- Descriptor decoders record the absolute byte offset of each field, which
  drives the two-way hover highlighting between decoded fields and the hex view.

## Deliberate omissions (vs. USBTreeView)

- Reading the MS OS 0xEE string descriptor (can hang some devices).
- Kernel-streaming / HID report-descriptor deep parsing, Bluetooth device list,
  disk read-speed test, writing FriendlyName to the registry (nicknames are
  stored locally instead), proprietary XML format (JSON + HTML instead).

## Testing

`cargo test` covers descriptor decoding (including malformed and truncated
data), usb.ids parsing, tree flattening, snapshot diffing, insights, reports,
HTML export, fuzzy matching and notification registration. The GUI was
exercised on real hardware (4 xHCI controllers, nested USB 2/3 hubs, UAS
drive, phones, audio/MIDI and a webcam).
