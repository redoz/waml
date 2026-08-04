//! `waml serve`: the web editor plus an ops API over one local directory.
//!
//! Laid out like `crate::lsp`: this module owns transport and process
//! lifetime, and delegates every semantic decision to `waml` proper.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub mod guard;
pub mod paths;
pub mod routes;
pub mod state;
pub mod ui;

use guard::{Guard, Token};
use routes::App;
use state::ServeState;

/// Everything `run` needs, decoupled from clap so tests can build one.
#[derive(Debug, Clone)]
pub struct ServeArgs {
    pub dir: PathBuf,
    pub port: u16,
    pub bind_all: bool,
    pub api_only: bool,
    pub no_open: bool,
}

/// Process exit code, matching the rest of the CLI: 0 ok, 2 I/O failure.
pub fn run(args: ServeArgs) -> i32 {
    let state = match ServeState::load(&args.dir) {
        Ok(state) => state,
        Err(err) => {
            eprintln!("waml serve: could not load {}: {err}", args.dir.display());
            return 2;
        }
    };

    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    runtime.block_on(async {
        let bind_ip = if args.bind_all {
            "0.0.0.0"
        } else {
            "127.0.0.1"
        };
        let listener = match tokio::net::TcpListener::bind((bind_ip, args.port)).await {
            Ok(listener) => listener,
            Err(err) => {
                eprintln!("waml serve: could not bind {bind_ip}:{}: {err}", args.port);
                return 2;
            }
        };
        let port = match listener.local_addr() {
            Ok(addr) => addr.port(),
            Err(err) => {
                eprintln!("waml serve: could not read the bound address: {err}");
                return 2;
            }
        };

        if args.bind_all {
            eprintln!(
                "waml serve: --bind-all exposes the API to your network on port {port}; \
                 the token is still required, but anyone on your network can present it"
            );
        }

        let token = Token::generate();
        let guard = Arc::new(Guard {
            token: token.clone(),
            port,
            bind_all: args.bind_all,
        });
        let app = App {
            state: Arc::new(Mutex::new(state)),
            guard,
        };

        let url = format!("http://127.0.0.1:{port}/?api=/api&token={}", token.as_str());
        println!("waml serve  {url}   (serving {})", args.dir.display());

        let ui_router = if args.api_only {
            eprintln!("waml serve: --api-only, the embedded web editor is not mounted");
            None
        } else {
            match crate::web_artifact::embedded_artifact() {
                Ok(artifact) => match ui::build(artifact) {
                    Ok(assets) => Some(ui::router(assets)),
                    Err(err) => {
                        eprintln!("waml serve: embedded web editor is malformed: {err}");
                        None
                    }
                },
                Err(err) => {
                    eprintln!("waml serve: {err}");
                    None
                }
            }
        };

        if !args.no_open {
            launch_browser(&url);
        }

        if let Err(err) = routes::serve_on(listener, app, ui_router).await {
            eprintln!("waml serve: server error: {err}");
            return 2;
        }
        0
    })
}

/// Launch the platform browser on `url`. A failed launch warns on stderr; it
/// does not fail the server (`serve` is still usable via a manual URL paste).
fn launch_browser(url: &str) {
    #[cfg(target_os = "windows")]
    let result = {
        // `cmd` re-parses its command line, so the URL must go through raw
        // and quoted: passed as a plain argument, cmd splits it at `&` and
        // the browser opens a tokenless URL (see `windows_start_line`).
        use std::os::windows::process::CommandExt;
        std::process::Command::new("cmd")
            .arg("/C")
            .raw_arg(windows_start_line(url))
            .status()
    };
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open").arg(url).status();
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let result = std::process::Command::new("xdg-open").arg(url).status();
    if let Err(err) = result {
        eprintln!("waml serve: could not open a browser automatically: {err}");
    }
}

/// The `start` line handed raw to `cmd /C`. The URL is double-quoted so cmd
/// treats `&` (the query separator carrying the token) as literal text, and
/// the leading `""` is `start`'s window-title slot, which would otherwise
/// swallow the quoted URL.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn windows_start_line(url: &str) -> String {
    format!("start \"\" \"{url}\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_windows_start_line_quotes_the_url_so_cmd_keeps_the_token() {
        let url = "http://127.0.0.1:8080/?api=/api&token=abc123";
        let line = windows_start_line(url);
        assert_eq!(
            line,
            "start \"\" \"http://127.0.0.1:8080/?api=/api&token=abc123\""
        );
        // The `&` must sit inside double quotes or cmd splits the line there.
        let quoted = line.rfind('"').unwrap();
        let amp = line.find('&').unwrap();
        assert!(amp < quoted, "token separator must be inside the quotes");
    }
}
