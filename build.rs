//! Windows only: embed the application icon and version information in
//! `oxyconvert.exe` (shown by Windows Explorer, the taskbar and installer).
//! On other targets this does nothing.

fn main() {
    println!("cargo:rerun-if-changed=packaging/icons/oxyconvert.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
    }
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        if std::path::Path::new("packaging/icons/oxyconvert.ico").exists() {
            res.set_icon("packaging/icons/oxyconvert.ico");
        }
        res.set("ProductName", "OxyConvert")
            .set("FileDescription", "OxyConvert Universal Addon Studio")
            .set("LegalCopyright", "The OxyLabs Contributors. MIT OR Apache-2.0");
        if let Err(e) = res.compile() {
            println!("cargo:warning=could not embed Windows icon/version resource: {e}");
        }
    }
}
