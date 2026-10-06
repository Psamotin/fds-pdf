//! Windows only: embed the app icon and version info (VERSIONINFO) into `printcraft.exe`, so it
//! shows in Explorer, the taskbar, the Start menu and Alt-Tab.
//!
//! On every other target this does nothing. A missing resource compiler is a warning, so a
//! cross-compile from macOS or Linux still links, unless `PRINTCRAFT_REQUIRE_WINRES=1` turns it
//! into an error (for release builds).

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../assets/fds-pdf/fds-pdf.ico");
    println!("cargo:rerun-if-env-changed=PRINTCRAFT_REQUIRE_WINRES");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    // The upstream brand icon is never embedded into the corporate build.
    // The resource compiler supplies version information even before an FDS icon is approved.
    let icon = "../../assets/fds-pdf/fds-pdf.ico";
    if std::path::Path::new(icon).is_file() {
        res.set_icon(icon);
    }
    res.set("ProductName", "ФДС ПДФ")
        .set("FileDescription", "ФДС ПДФ — работа с PDF")
        .set("ProductVersion", env!("CARGO_PKG_VERSION"))
        .set("FileVersion", env!("CARGO_PKG_VERSION"))
        .set("LegalCopyright", "Copyright (c) the PrintCraft contributors. MIT OR Apache-2.0.")
        .set("OriginalFilename", "FDS-PDF.exe")
        .set("InternalName", "printcraft");
    if let Err(e) = res.compile() {
        if std::env::var_os("PRINTCRAFT_REQUIRE_WINRES").is_some() {
            println!("cargo::error=embedding Windows resources failed: {e}");
            return;
        }
        println!("cargo:warning=printcraft.exe built without icon/version resources: {e}");
    }
}
