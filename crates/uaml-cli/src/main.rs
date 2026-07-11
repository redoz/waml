use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

mod commands;
mod io;
// Wire layer for a future `--emit`/apply CLI surface (Task 8 only adds the DTO + its
// own round-trip tests; nothing in this binary calls it yet).
#[allow(dead_code)]
mod ops_dto;

#[derive(Parser)]
#[command(name = "uaml", about = "Tools for UAML documents")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Parse and validate documents, reporting diagnostics.
    Check {
        /// Files or directories to check.
        paths: Vec<PathBuf>,
        /// Read a single document/bundle from stdin instead.
        #[arg(long)]
        stdin: bool,
        /// Output format.
        #[arg(long, value_enum, default_value_t = Format::Human)]
        format: Format,
    },
    /// Rewrite documents in canonical form.
    Fmt {
        /// Files or directories to format.
        paths: Vec<PathBuf>,
        /// Do not write; exit non-zero if any file is not already formatted.
        #[arg(long)]
        check: bool,
        /// Write the formatted result to stdout instead of the file.
        #[arg(long)]
        stdout: bool,
    },
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, ValueEnum)]
enum Format {
    Human,
    Json,
}

fn main() {
    let cli = Cli::parse();
    let code = match cli.command {
        Command::Check { paths, stdin, format } => {
            let bundle = match io::read_bundle(&paths, stdin) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("uaml: {e}");
                    std::process::exit(2);
                }
            };
            let diags = uaml::validate::validate(&bundle);
            let out = match format {
                Format::Human => commands::render_human(&diags),
                Format::Json => commands::render_json(&diags),
            };
            println!("{out}");
            commands::check_exit_code(&diags)
        }
        Command::Fmt { paths, check, stdout } => {
            let files = match io::read_files(&paths) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("uaml: {e}");
                    std::process::exit(2);
                }
            };
            let plan = commands::plan_fmt(&files);
            let mut exit = 0;
            for r in &plan {
                if r.skipped {
                    eprintln!("uaml: skipped {} (has errors; run `uaml check`)", r.path);
                    exit = 1;
                    continue;
                }
                if stdout {
                    println!("{}", r.formatted);
                } else if check {
                    if r.changed {
                        eprintln!("uaml: {} is not formatted", r.path);
                        exit = 1;
                    }
                } else if r.changed {
                    if let Err(e) = std::fs::write(&r.path, &r.formatted) {
                        eprintln!("uaml: failed to write {}: {e}", r.path);
                        std::process::exit(2);
                    }
                    println!("uaml: formatted {}", r.path);
                }
            }
            exit
        }
    };
    std::process::exit(code);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_check_with_json_flag() {
        let cli = Cli::try_parse_from(["uaml", "check", "a.md", "--format", "json"]).unwrap();
        match cli.command {
            Command::Check { paths, format, stdin } => {
                assert_eq!(paths.len(), 1);
                assert_eq!(format, Format::Json);
                assert!(!stdin);
            }
            _ => panic!("expected check"),
        }
    }

    #[test]
    fn parses_fmt_check() {
        let cli = Cli::try_parse_from(["uaml", "fmt", "--check", "docs/"]).unwrap();
        assert!(matches!(cli.command, Command::Fmt { check: true, .. }));
    }
}
