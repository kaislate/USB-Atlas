//! Embeds the logo (.ico) and version information into USB_Atlas.exe.

#[allow(dead_code)]
mod logo {
    include!("src/logo_raster.rs");
}

fn main() {
    println!("cargo:rerun-if-changed=src/logo_raster.rs");
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let ico = out.join("usb_atlas.ico");
    std::fs::write(
        &ico,
        logo::ico_bytes(&[16, 20, 24, 32, 40, 48, 64, 128, 256]),
    )
    .expect("write icon");

    let mut res = winresource::WindowsResource::new();
    res.set_icon(ico.to_str().unwrap())
        .set("ProductName", "USB Atlas")
        .set("FileDescription", "USB Atlas – USB topology explorer")
        .set("CompanyName", "kaislate")
        .set(
            "LegalCopyright",
            "Copyright (c) 2026 kaislate. MIT License.",
        )
        .set("OriginalFilename", "USB_Atlas.exe");
    if let Err(e) = res.compile() {
        // Don't fail the build on machines without the Windows SDK resource compiler.
        println!("cargo:warning=could not embed the icon/version resource: {e}");
    }
}
