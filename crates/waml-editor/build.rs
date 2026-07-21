// Embeds the waml logo as the Windows executable + window icon.
//
// `winresource` compiles `resources/icon.ico` into the exe as the default icon
// resource, so Explorer, the taskbar, and the Alt-Tab switcher all show the
// waml mark instead of Makepad's stock glyph. No-op on every other platform.
fn main() {
    embed_icon();
}

// `winresource` is a `cfg(windows)` build-dependency, so it only exists on a
// Windows *host*. Gate the reference to it with the same host cfg or the crate
// won't compile on Linux/macOS.
#[cfg(windows)]
fn embed_icon() {
    let mut res = winresource::WindowsResource::new();
    // Relative to this crate's manifest dir; the .ico lives at the repo root.
    res.set_icon("../../resources/icon.ico");
    res.compile().expect("embed windows app icon");
}

#[cfg(not(windows))]
fn embed_icon() {}
