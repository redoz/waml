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

        if args.api_only {
            eprintln!("waml serve: --api-only, the embedded web editor is not mounted");
        }

        if !args.no_open {
            launch_browser(&url);
        }

        if let Err(err) = routes::serve_on(listener, app).await {
            eprintln!("waml serve: server error: {err}");
            return 2;
        }
        0
    })
}

/// Launch the platform browser on `url`. A failed launch warns on stderr; it
/// does not fail the server (`serve` is still usable via a manual URL paste).
fn launch_browser(url: &str) {
    let result = if cfg!(target_os = "windows") {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .status()
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(url).status()
    } else {
        std::process::Command::new("xdg-open").arg(url).status()
    };
    if let Err(err) = result {
        eprintln!("waml serve: could not open a browser automatically: {err}");
    }
}
