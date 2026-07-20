// Embeds the waml logo as the Windows executable + window icon.
//
// `winresource` compiles `resources/icon.ico` into the exe as the default icon
// resource, so Explorer, the taskbar, and the Alt-Tab switcher all show the
// waml mark instead of Makepad's stock glyph. No-op on every other platform.
fn main() {
    // build.rs is compiled for the host, so key off the *target* OS, not host
    // cfg -- keeps cross-compiles honest.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        // Relative to this crate's manifest dir; the .ico lives at the repo root.
        res.set_icon("../../resources/icon.ico");
        res.compile().expect("embed windows app icon");
    }
}
