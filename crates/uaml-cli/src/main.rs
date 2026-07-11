use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

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
        Command::Check { .. } => 0,
        Command::Fmt { .. } => 0,
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
