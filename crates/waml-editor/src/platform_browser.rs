use makepad_widgets::*;

pub trait ExternalUrlAdapter {
    fn open(&mut self, cx: &mut Cx, url: &str) -> Result<(), String>;
}

pub struct PlatformBrowser;

#[cfg(target_arch = "wasm32")]
impl ExternalUrlAdapter for PlatformBrowser {
    fn open(&mut self, cx: &mut Cx, url: &str) -> Result<(), String> {
        cx.open_url(url, OpenUrlInPlace::No);
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl ExternalUrlAdapter for PlatformBrowser {
    fn open(&mut self, _cx: &mut Cx, url: &str) -> Result<(), String> {
        #[cfg(target_os = "windows")]
        let mut command = {
            let mut command = std::process::Command::new("rundll32.exe");
            command.args(["url.dll,FileProtocolHandler", url]);
            command
        };
        #[cfg(target_os = "macos")]
        let mut command = {
            let mut command = std::process::Command::new("open");
            command.arg(url);
            command
        };
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        let mut command = {
            let mut command = std::process::Command::new("xdg-open");
            command.arg(url);
            command
        };

        command
            .spawn()
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
}
