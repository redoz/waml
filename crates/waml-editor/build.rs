// Embeds the waml logo as the Windows executable + window icon.
//
// `winresource` compiles `resources/icon.ico` into the exe as the default icon
// resource, so Explorer, the taskbar, and the Alt-Tab switcher all show the
// waml mark instead of Makepad's stock glyph. No-op on every other platform.
fn main() {
    embed_icon();
    configure_windows_stack();
}

#[cfg(windows)]
fn configure_windows_stack() {
    // The native editor solves diagrams on the Makepad UI thread. The exact
    // Browser and Tooling use-case documents overflow MSVC's 1 MiB executable
    // stack while the obstacle graph and bounded A* searches are live, even
    // after the large solve phases are kept out of line. An 8 MiB reserve is
    // the Windows Rust toolchain's conventional executable-stack size and was
    // verified by launching all three real documents through run.ps1. This
    // changes virtual address reservation only; committed pages still grow on
    // demand. Other targets retain their platform defaults.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        println!("cargo:rustc-link-arg-bin=waml-editor=/STACK:8388608");
    }
}

#[cfg(not(windows))]
fn configure_windows_stack() {}

// `winresource` is a `cfg(windows)` build-dependency, so it only exists on a
// Windows *host*. Gate the reference to it with the same host cfg or the crate
// won't compile on Linux/macOS.
#[cfg(windows)]
fn embed_icon() {
    // A build script is compiled for the *host*, so `cfg(windows)` stays true
    // when cross-compiling from Windows to e.g. wasm32 -- and `rc.exe` then
    // refuses the job ("Can only compile resource file when target_env is
    // \"gnu\" or \"msvc\""). Check the *target* separately.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    // Relative to this crate's manifest dir; the .ico lives at the repo root.
    res.set_icon("../../resources/icon.ico");
    res.compile().expect("embed windows app icon");
}

#[cfg(not(windows))]
fn embed_icon() {}
