//! Build script: embed the NovaKey application icon into the exe.
//!
//! The icon (assets/NovaKey.ico) is generated from Asset/NewLogo.png — the same
//! branded logo the macOS app uses. It becomes the Explorer file icon and is
//! loaded at runtime (resource id 1) for the window and the "Vietnamese OFF"
//! tray icon. assets/NovaKey_V.ico (resource id 2) is the "Vietnamese ON"
//! variant, used for the tray icon while Vietnamese input is enabled.

fn main() {
    println!("cargo:rerun-if-changed=assets/NovaKey.ico");
    println!("cargo:rerun-if-changed=assets/NovaKey_V.ico");
    println!("cargo:rerun-if-changed=build.rs");

    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        let mut res = winresource::WindowsResource::new();
        // Explicit id 1: lowest ordinal, so Explorer picks it as the app icon.
        res.set_icon_with_id("assets/NovaKey.ico", "1");
        // Id 2: the "Vietnamese ON" variant used for the tray icon when enabled.
        res.set_icon_with_id("assets/NovaKey_V.ico", "2");
        res.set("ProductName", "NovaKey");
        res.set("FileDescription", "NovaKey Vietnamese IME");
        if let Err(e) = res.compile() {
            // Don't hard-fail the build if the resource compiler is unavailable
            // (e.g. building on CI without the Windows SDK) — the app still runs,
            // just with the default icon.
            println!("cargo:warning=icon resource embedding skipped: {e}");
        }
    }
}
