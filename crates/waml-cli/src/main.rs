use std::path::{Path, PathBuf};

use clap::{Args, Parser, Subcommand, ValueEnum};
use waml::analysis::{prepare_candidate, PreviousAnalyses};
use waml::bundle_envelope::{
    encode_bundle_envelope, is_ts_export_name, render_bundle_json, render_bundle_ts,
};
use waml::edit::{EditBatch, EditContext};
use waml::multiplicity::Multiplicity;
use waml::uml::FieldEdit;

use crate::ops_dto::{to_batch, OpDto};

mod commands;
mod io;
mod lsp;
mod ops_dto;
mod serve;
mod site;
pub mod upgrade;
mod web_artifact;

#[derive(Parser)]
#[command(name = "waml", about = "Tools for WAML documents")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Upgrade documents to the current WAML format.
    Upgrade {
        /// File or directory to upgrade.
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Report required changes without writing them.
        #[arg(long)]
        check: bool,
    },
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
    /// Print a shareable link that carries the whole bundle in its URL fragment.
    ///
    /// The web editor reconstructs the model from the fragment, so the link is
    /// self-contained: nothing is uploaded anywhere, and browsers never send a
    /// fragment to the server.
    Share {
        /// Files or directories to pack into the link.
        paths: Vec<PathBuf>,
        /// Read a single document/bundle from stdin instead.
        #[arg(long)]
        stdin: bool,
        /// Where the web editor is hosted. Omit to print just the fragment.
        #[arg(long, default_value = "https://redoz.github.io/waml/")]
        base_url: String,
        /// Print only the `w1.` fragment, with no URL around it.
        #[arg(long)]
        fragment_only: bool,
    },
    /// Run the WAML language server (stdio LSP).
    Lsp {
        /// Use stdio transport (the only supported transport in Phase 1).
        ///
        /// `overrides_with_self` lets the flag be passed more than once without
        /// error: the LSP client appends its own `--stdio` (TransportKind.stdio)
        /// on top of any we pass, and a repeated flag must not abort the server.
        #[arg(long, overrides_with = "stdio")]
        stdio: bool,
    },
    /// Serve the web editor and an ops API over a local directory.
    ///
    /// Binds loopback by default and mints a fresh access token per run; the
    /// token is printed once, in the URL, and is required on every request.
    /// Loopback is not access control — see the token flag docs.
    Serve {
        /// Directory to serve and edit.
        #[arg(default_value = ".")]
        dir: PathBuf,
        /// Port to bind. `0` picks an ephemeral port and prints it.
        #[arg(long, default_value_t = 8099)]
        port: u16,
        /// Bind 0.0.0.0 instead of 127.0.0.1. Exposes the API to your network.
        #[arg(long)]
        bind_all: bool,
        /// Serve only the API, without the embedded web editor.
        #[arg(long)]
        api_only: bool,
        /// Do not open a browser on start.
        #[arg(long)]
        no_open: bool,
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
    /// Rebuild deterministic directory index documents.
    Index {
        /// One directory that contains the Markdown bundle.
        path: PathBuf,
        /// Do not write; exit non-zero when an index differs.
        #[arg(long)]
        check: bool,
    },
    /// Create, rename, update, or remove a node.
    Node {
        #[command(subcommand)]
        action: NodeCmd,
        #[command(flatten)]
        common: Common,
    },
    /// Add, update, or remove an attribute on a node.
    Attr {
        #[command(subcommand)]
        action: AttrCmd,
        #[command(flatten)]
        common: Common,
    },
    /// Add or remove an enum literal value on a node.
    Value {
        #[command(subcommand)]
        action: ValueCmd,
        #[command(flatten)]
        common: Common,
    },
    /// Add, update, or remove a relationship.
    Rel {
        #[command(subcommand)]
        action: RelCmd,
        #[command(flatten)]
        common: Common,
    },
    /// Apply a batch of ops from an NDJSON op-log.
    Apply {
        /// NDJSON op-log file, or `-` for stdin.
        ops: String,
        #[command(flatten)]
        common: Common,
    },
    /// Show a resolved classifier.
    Show {
        slug: String,
        #[command(flatten)]
        query: QueryArgs,
    },
    /// List referrers of a classifier.
    Refs {
        slug: String,
        #[command(flatten)]
        query: QueryArgs,
    },
    /// List classifiers, optionally filtered by type.
    List {
        #[arg(long)]
        r#type: Option<String>,
        #[command(flatten)]
        query: QueryArgs,
    },
    /// Bundle a directory of `.md` files into a single JSON or TS artifact.
    Bundle {
        /// Directory to recursively collect `*.md` from.
        dir: PathBuf,
        /// Output format.
        #[arg(long, value_enum, default_value_t = BundleFormat::Json)]
        format: BundleFormat,
        /// Write to this file instead of stdout.
        #[arg(long)]
        out: Option<PathBuf>,
        /// Exported const name; required when `--format ts`.
        #[arg(long)]
        export_name: Option<String>,
    },
    /// Export a model in a self-contained form.
    Export {
        #[command(subcommand)]
        target: ExportCommand,
    },
}

#[derive(Subcommand)]
enum ExportCommand {
    /// Write a static site: the web editor plus this model's `bundle.waml`.
    ///
    /// The result is plain files with no server behind them -- open
    /// `index.html` over HTTP and the editor boots the bundle, editable and
    /// re-exportable from the browser.
    Site {
        /// Directory to recursively collect `*.md` from.
        dir: PathBuf,
        /// Where to write the site.
        #[arg(long, default_value = "./site")]
        out: PathBuf,
        /// Delete everything in a non-empty output directory, then write the site.
        #[arg(long)]
        force: bool,
    },
}

/// Flags shared by all mutating (node/attr/value/rel) subcommands.
/// `global = true` so these may appear either before or after the nested
/// action subcommand (e.g. both `attr --dir D add ...` and `attr add ... --dir D`).
#[derive(Args)]
struct Common {
    /// Bundle root; recursively collects *.md. Default: current directory.
    #[arg(long, default_value = ".", global = true)]
    dir: PathBuf,
    #[arg(long, global = true)]
    dry_run: bool,
    #[arg(long, global = true)]
    stdout: bool,
    #[arg(long, global = true)]
    emit: bool,
    #[arg(long, value_enum, default_value_t = Format::Human, global = true)]
    format: Format,
}

/// Flags shared by the read-only query subcommands (show/refs/list).
#[derive(Args)]
struct QueryArgs {
    /// Bundle root; recursively collects *.md. Default: current directory.
    #[arg(long, default_value = ".")]
    dir: PathBuf,
    #[arg(long, value_enum, default_value_t = Format::Human)]
    format: Format,
}

#[derive(Subcommand)]
enum NodeCmd {
    New {
        slug: String,
        #[arg(long)]
        r#type: String,
        #[arg(long)]
        title: String,
        #[arg(long, value_delimiter = ',')]
        stereotype: Vec<String>,
        #[arg(long)]
        desc: Option<String>,
        #[arg(long)]
        r#abstract: bool,
    },
    Rename {
        from: String,
        to: String,
    },
    Set {
        slug: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        desc: Option<String>,
        #[arg(long, value_delimiter = ',')]
        stereotype: Option<Vec<String>>,
        #[arg(long)]
        r#abstract: Option<bool>,
        #[arg(long)]
        r#type: Option<String>,
    },
    Rm {
        slug: String,
        #[arg(long)]
        cascade: bool,
    },
}

#[derive(Subcommand)]
enum AttrCmd {
    Add {
        node: String,
        name: String,
        r#type: String,
        #[arg(long)]
        mult: Option<String>,
        #[arg(long)]
        vis: Option<String>,
    },
    Set {
        node: String,
        name: String,
        #[arg(long)]
        r#type: Option<String>,
        #[arg(long)]
        mult: Option<String>,
        #[arg(long, conflicts_with = "mult")]
        clear_mult: bool,
        #[arg(long)]
        vis: Option<String>,
        #[arg(long)]
        rename: Option<String>,
    },
    Rm {
        node: String,
        name: String,
    },
}

#[derive(Subcommand)]
enum ValueCmd {
    Add { node: String, literal: String },
    Rm { node: String, literal: String },
}

#[derive(Subcommand)]
enum RelCmd {
    Add {
        source: String,
        verb: String,
        target: String,
        #[arg(long)]
        ends: Option<String>,
        #[arg(long = "as")]
        as_label: Option<String>,
        #[arg(long)]
        as_ref: Option<String>,
    },
    Set {
        source: String,
        #[arg(long)]
        verb: Option<String>,
        #[arg(long)]
        target: Option<String>,
        #[arg(long = "as")]
        as_sel: Option<String>,
        #[arg(long)]
        ends: Option<String>,
        #[arg(long = "set-as")]
        set_label: Option<String>,
        #[arg(long)]
        set_as_ref: Option<String>,
    },
    Rm {
        source: String,
        #[arg(long)]
        verb: Option<String>,
        #[arg(long)]
        target: Option<String>,
        #[arg(long = "as")]
        as_sel: Option<String>,
    },
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, ValueEnum)]
enum Format {
    Human,
    Json,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, ValueEnum)]
enum BundleFormat {
    Json,
    Ts,
}

fn main() {
    let cli = Cli::parse();
    let code = match cli.command {
        Command::Upgrade { path, check } => run_upgrade(&path, check),
        Command::Check {
            paths,
            stdin,
            format,
        } => {
            let bundle = match io::read_analysis_bundle(&paths, stdin) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("waml: {e}");
                    std::process::exit(2);
                }
            };
            let prepared = match waml::validate::prepare(&bundle.files) {
                Ok(prepared) => prepared,
                Err(error) => {
                    eprintln!("waml: {error}");
                    std::process::exit(2);
                }
            };
            let diags = waml::validate::diagnostics(&prepared, &bundle.display_paths);
            let out = match format {
                Format::Human => commands::render_human(&diags),
                Format::Json => commands::render_json(&diags),
            };
            println!("{out}");
            commands::check_exit_code(&diags)
        }
        Command::Share {
            paths,
            stdin,
            base_url,
            fragment_only,
        } => {
            let bundle = match io::read_bundle_rooted(&paths, stdin) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("waml: {e}");
                    std::process::exit(2);
                }
            };
            let fragment = waml::share::encode(&bundle);
            if fragment_only {
                println!("{fragment}");
            } else {
                // One `#` exactly, whether or not the base URL ends in a slash.
                println!("{}#{fragment}", base_url.trim_end_matches('#'));
            }
            0
        }
        Command::Fmt {
            paths,
            check,
            stdout,
        } => {
            let bundle = match io::read_physical_bundle(&paths) {
                Ok(bundle) => bundle,
                Err(e) => {
                    eprintln!("waml: {e}");
                    std::process::exit(2);
                }
            };
            let plan = match waml::fmt::plan_fmt(&bundle.files) {
                Ok(plan) => plan,
                Err(error) => {
                    eprintln!("waml: {error}");
                    std::process::exit(2);
                }
            };
            let mut exit = 0;
            for r in &plan {
                if r.skipped {
                    let display = bundle
                        .display_paths
                        .get(&r.path)
                        .map(String::as_str)
                        .unwrap_or(&r.path);
                    eprintln!("waml: skipped {display} (has errors; run `waml check`)");
                    exit = 1;
                    continue;
                }
                if !stdout && check && r.changed {
                    let display = bundle
                        .display_paths
                        .get(&r.path)
                        .map(String::as_str)
                        .unwrap_or(&r.path);
                    eprintln!("waml: {display} is not formatted");
                    exit = 1;
                }
            }
            let stdout_files: Vec<_> = plan
                .iter()
                .filter(|result| !result.skipped)
                .map(|result| (result.path.clone(), result.formatted.clone()))
                .collect();
            if stdout {
                match stdout_files.as_slice() {
                    [] => {}
                    [(_, text)] => print!("{text}"),
                    files => match encode_bundle_envelope(files) {
                        Ok(blob) => print!("{blob}"),
                        Err(error) => {
                            eprintln!("waml: {error}");
                            exit = 2;
                        }
                    },
                }
            }
            if !stdout && !check {
                let formatted = plan
                    .iter()
                    .map(|result| (result.path.clone(), result.formatted.clone()))
                    .collect::<Vec<_>>();
                if let Err(error) = io::write_back(&bundle.root, &bundle.files, &formatted) {
                    eprintln!("waml: {error}");
                    std::process::exit(2);
                }
                for result in plan.iter().filter(|result| result.changed) {
                    let display = bundle
                        .display_paths
                        .get(&result.path)
                        .map(String::as_str)
                        .unwrap_or(&result.path);
                    println!("waml: formatted {display}");
                }
            }
            exit
        }
        Command::Index { path, check } => run_index(&path, check),
        Command::Lsp { stdio: _ } => lsp::run(),
        Command::Serve {
            dir,
            port,
            bind_all,
            api_only,
            no_open,
        } => serve::run(serve::ServeArgs {
            dir,
            port,
            bind_all,
            api_only,
            no_open,
        }),
        Command::Node { action, common } => run_mutation(&common, node_dto(action)),
        Command::Attr { action, common } => run_mutation(&common, attr_dto(action)),
        Command::Value { action, common } => run_mutation(&common, value_dto(action)),
        Command::Rel { action, common } => run_mutation(&common, rel_dto(action)),
        Command::Apply { ops, common } => run_apply(&ops, &common),
        Command::Show { slug, query } => run_show(&slug, &query),
        Command::Refs { slug, query } => run_refs(&slug, &query),
        Command::List { r#type, query } => run_list(&r#type, &query),
        Command::Bundle {
            dir,
            format,
            out,
            export_name,
        } => run_bundle(&dir, format, out, export_name),
        Command::Export {
            target: ExportCommand::Site { dir, out, force },
        } => run_export_site(&dir, &out, force),
    };
    std::process::exit(code);
}

fn run_upgrade(path: &Path, check: bool) -> i32 {
    let paths = [path.to_path_buf()];
    let bundle = match io::read_physical_bundle(&paths) {
        Ok(bundle) => bundle,
        Err(error) => {
            eprintln!("waml: {error}");
            return 2;
        }
    };
    let plan = match upgrade::plan_upgrade(&bundle.files) {
        Ok(plan) => plan,
        Err(error) => {
            eprintln!("waml: upgrade failed: {}", render_upgrade_error(&error));
            return 1;
        }
    };

    if check {
        print_upgrade_reports(&plan.applied);
        return i32::from(!plan.applied.is_empty());
    }
    if plan.applied.is_empty() {
        return 0;
    }

    let touched = match io::write_back(&bundle.root, &bundle.files, &plan.files) {
        Ok(touched) => touched,
        Err(error) => {
            eprintln!("waml: could not write upgrade: {error}");
            return 1;
        }
    };
    print_upgrade_reports(&plan.applied);
    for warning in touched.iter().filter(|entry| entry.starts_with("warning:")) {
        eprintln!("{warning}");
    }
    0
}

fn render_upgrade_error(error: &upgrade::UpgradeError) -> String {
    use upgrade::UpgradeError;
    use waml::upgrade::UpgradeInspectionError;

    match error {
        UpgradeError::InvalidSource(message) => {
            format!("the source bundle is not valid: {message}")
        }
        UpgradeError::Inspection(UpgradeInspectionError::AmbiguousLegacyDiagram {
            path,
            incompatible_members,
        }) => format!(
            "cannot select the diagram type for {path}: it contains use-case and classifier members ({})",
            incompatible_members.join(", ")
        ),
        UpgradeError::Inspection(UpgradeInspectionError::InvalidLegacyBundle(_)) => {
            "the bundle does not pass strict validation".to_string()
        }
        UpgradeError::Rewrite { path, .. } => {
            format!("cannot rewrite the diagram type in {path}")
        }
        UpgradeError::Preparation(message) => {
            format!("cannot prepare the upgraded bundle: {message}")
        }
        UpgradeError::InvalidCandidate(_) => {
            "the upgraded bundle does not pass strict validation".to_string()
        }
    }
}

fn print_upgrade_reports(applied: &[upgrade::AppliedMigration]) {
    for report in applied {
        println!("{}: {} - {}", report.path, report.id, report.description);
    }
}

fn run_index(path: &Path, check: bool) -> i32 {
    match std::fs::metadata(path) {
        Ok(metadata) if metadata.is_dir() => {}
        Ok(_) => {
            eprintln!("waml: index path must be a directory: {}", path.display());
            return 2;
        }
        Err(error) => {
            eprintln!("waml: {error}");
            return 2;
        }
    }
    let bundle = match io::read_physical_bundle(std::slice::from_ref(&path.to_path_buf())) {
        Ok(bundle) => bundle,
        Err(error) => {
            eprintln!("waml: {error}");
            return 2;
        }
    };
    let changes = match commands::plan_indexes(&bundle.files) {
        Ok(changes) => changes,
        Err(error) => {
            eprintln!("waml: {error}");
            return 2;
        }
    };
    if check {
        for change in &changes {
            let path = match change {
                commands::IndexChange::Upsert { path, .. }
                | commands::IndexChange::Remove { path } => path,
            };
            let display = bundle
                .display_paths
                .get(path)
                .cloned()
                .unwrap_or_else(|| bundle.root.join(path).display().to_string());
            eprintln!("waml: {display}: generated index is stale");
        }
        return i32::from(!changes.is_empty());
    }
    match io::write_indexes(&bundle.root, &changes) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("waml: {error}");
            2
        }
    }
}

fn node_dto(a: NodeCmd) -> OpDto {
    match a {
        NodeCmd::New {
            slug,
            r#type,
            title,
            stereotype,
            desc,
            r#abstract,
        } => OpDto::NodeNew {
            v: 1,
            slug,
            dir: String::new(),
            ty: r#type,
            title,
            stereotype,
            desc,
            abstract_: r#abstract,
        },
        NodeCmd::Rename { from, to } => OpDto::NodeRename { v: 1, from, to },
        NodeCmd::Set {
            slug,
            title,
            desc,
            stereotype,
            r#abstract,
            r#type,
        } => OpDto::NodeSet {
            v: 1,
            slug,
            title,
            desc,
            stereotype,
            abstract_: r#abstract,
            ty: r#type,
        },
        NodeCmd::Rm { slug, cascade } => OpDto::NodeRm {
            v: 1,
            slug,
            cascade,
        },
    }
}

fn attr_dto(a: AttrCmd) -> OpDto {
    match a {
        AttrCmd::Add {
            node,
            name,
            r#type,
            mult,
            vis,
        } => OpDto::AttrAdd {
            v: 1,
            node,
            name,
            ty: r#type,
            mult,
            vis,
        },
        AttrCmd::Set {
            node,
            name,
            r#type,
            mult,
            clear_mult,
            vis,
            rename,
        } => OpDto::AttrSet {
            v: 1,
            node,
            name,
            ty: r#type,
            mult: if clear_mult {
                FieldEdit::Clear
            } else {
                mult.map_or(FieldEdit::Unchanged, FieldEdit::Set)
            },
            vis,
            rename,
        },
        AttrCmd::Rm { node, name } => OpDto::AttrRm { v: 1, node, name },
    }
}

fn value_dto(a: ValueCmd) -> OpDto {
    match a {
        ValueCmd::Add { node, literal } => OpDto::ValueAdd {
            v: 1,
            node,
            literal,
        },
        ValueCmd::Rm { node, literal } => OpDto::ValueRm {
            v: 1,
            node,
            literal,
        },
    }
}

fn rel_dto(a: RelCmd) -> OpDto {
    match a {
        RelCmd::Add {
            source,
            verb,
            target,
            ends,
            as_label,
            as_ref,
        } => OpDto::RelAdd {
            v: 1,
            source,
            kind: verb,
            target,
            as_label,
            as_ref,
            ends,
        },
        RelCmd::Set {
            source,
            verb,
            target,
            as_sel,
            ends,
            set_label,
            set_as_ref,
        } => OpDto::RelSet {
            v: 1,
            source,
            kind: verb,
            target,
            as_sel,
            ends,
            set_label,
            set_as_ref,
        },
        RelCmd::Rm {
            source,
            verb,
            target,
            as_sel,
        } => OpDto::RelRm {
            v: 1,
            source,
            kind: verb,
            target,
            as_sel,
        },
    }
}

fn run_mutation(common: &Common, dto: OpDto) -> i32 {
    if [common.emit, common.stdout, common.dry_run]
        .iter()
        .filter(|x| **x)
        .count()
        > 1
    {
        eprintln!("waml: --emit, --stdout, --dry-run mutually exclusive");
        return 2;
    }
    if common.emit {
        return match serde_json::to_string(&dto) {
            Ok(line) => {
                println!("{line}");
                0
            }
            Err(e) => {
                eprintln!("waml: {e}");
                2
            }
        };
    }
    let batch = match to_batch(std::slice::from_ref(&dto)) {
        Ok(batch) => batch,
        Err(e) => {
            eprintln!("waml: {e}");
            return 1;
        }
    };
    run_batch(common, batch)
}

fn run_batch(common: &Common, batch: waml::edit::Batch) -> i32 {
    let pairs = match io::read_bundle_rooted(std::slice::from_ref(&common.dir), false) {
        Ok(pairs) => pairs,
        Err(e) => {
            eprintln!("waml: {e}");
            return 2;
        }
    };
    let source = match waml::source::SourceBundle::try_from_pairs(pairs) {
        Ok(source) => source,
        Err(e) => {
            eprintln!("waml: {e}");
            return 2;
        }
    };
    let prepared = match prepare_candidate(source, None, 0) {
        Ok(prepared) => prepared,
        Err(e) => {
            eprintln!("waml: {e}");
            return 2;
        }
    };
    match batch.lower(EditContext {
        source: prepared.source(),
        okf_analysis: prepared.okf(),
        session_revision: prepared.revision(),
        uml: prepared.uml(),
    }) {
        Ok(changed) => {
            let validated = match prepare_candidate(
                changed,
                Some(PreviousAnalyses {
                    okf: prepared.okf(),
                    uml: prepared.uml(),
                }),
                1,
            ) {
                Ok(validated) => validated,
                Err(error) => {
                    eprintln!("waml: {error}");
                    return 1;
                }
            };
            let old = prepared.source().to_pairs();
            let new = validated.source().to_pairs();
            if common.stdout {
                match encode_bundle_envelope(&new) {
                    Ok(blob) => {
                        print!("{blob}");
                        0
                    }
                    Err(error) => {
                        eprintln!("waml: {error}");
                        2
                    }
                }
            } else if common.dry_run {
                print!("{}", commands::render_diff(&old, &new));
                0
            } else {
                match io::write_back(&common.dir, &old, &new) {
                    Ok(touched) => {
                        for t in touched {
                            println!("waml: {t}");
                        }
                        0
                    }
                    Err(e) => {
                        eprintln!("waml: {e}");
                        2
                    }
                }
            }
        }
        Err(e) => {
            let sel = e
                .selector
                .as_ref()
                .map(|s| format!(" [{s}]"))
                .unwrap_or_default();
            eprintln!("waml: op {}: {}{sel}", e.index, e.reason);
            1
        }
    }
}

fn run_apply(ops_src: &str, common: &Common) -> i32 {
    let lines = match io::read_ndjson(ops_src) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("waml: {e}");
            return 2;
        }
    };
    let mut dtos = Vec::new();
    for (n, line) in &lines {
        let dto: OpDto = match serde_json::from_str(line) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("waml: line {n}: {e}");
                return 1;
            }
        };
        dtos.push((*n, dto));
    }
    let raw = dtos.into_iter().map(|(_, dto)| dto).collect::<Vec<_>>();
    let batch = match to_batch(&raw) {
        Ok(batch) => batch,
        Err(error) => {
            let rendered = error
                .strip_prefix("op ")
                .and_then(|rest| rest.split_once(':'))
                .and_then(|(index, reason)| {
                    index
                        .parse::<usize>()
                        .ok()
                        .and_then(|index| lines.get(index))
                        .map(|(line, _)| format!("line {line}:{reason}"))
                })
                .unwrap_or(error);
            eprintln!("waml: {rendered}");
            return 1;
        }
    };
    run_batch(common, batch)
}

fn run_show(slug: &str, q: &QueryArgs) -> i32 {
    let bundle = match io::read_bundle_rooted(std::slice::from_ref(&q.dir), false) {
        Ok(bundle) => bundle,
        Err(e) => {
            eprintln!("waml: {e}");
            return 2;
        }
    };
    let prepared = match waml::validate::prepare(&bundle) {
        Ok(prepared) => prepared,
        Err(error) => {
            eprintln!("waml: {error}");
            return 2;
        }
    };
    let model = &prepared.uml().projection;
    let Some(node) = model.node(slug) else {
        eprintln!("waml: no classifier '{slug}'");
        return 1;
    };
    match q.format {
        Format::Human => {
            println!(
                "{} ({})",
                node.concept.title.as_deref().unwrap_or("Untitled"),
                node.ty.as_str()
            );
            for a in &node.attributes {
                println!(
                    "  - {}: {} {{{}}}",
                    a.name,
                    a.ty.name,
                    a.multiplicity
                        .as_ref()
                        .map(Multiplicity::as_str)
                        .unwrap_or("1")
                );
            }
            for v in &node.values {
                println!("  = {v}");
            }
            for e in model
                .edges
                .iter()
                .filter(|e| e.source == slug || e.target == slug)
            {
                println!("  {} {} {}", e.source, e.kind.as_str(), e.target);
            }
            0
        }
        Format::Json => {
            let refs = prepared.referrers(slug);
            let dto = serde_json::json!({
                "slug": slug, "title": node.concept.title.as_deref().unwrap_or("Untitled"), "type": node.ty.as_str(),
                "attributes": node.attributes.iter().map(|a| serde_json::json!({
                    "name": a.name, "type": a.ty.name, "ref": a.ty.ref_, "multiplicity": a.multiplicity.as_ref().map(Multiplicity::as_str)
                })).collect::<Vec<_>>(),
                "values": node.values,
                "referrers": refs,
            });
            println!(
                "{}",
                serde_json::to_string_pretty(&dto).unwrap_or_else(|_| "{}".into())
            );
            0
        }
    }
}

fn run_refs(slug: &str, q: &QueryArgs) -> i32 {
    let bundle = match io::read_bundle_rooted(std::slice::from_ref(&q.dir), false) {
        Ok(bundle) => bundle,
        Err(e) => {
            eprintln!("waml: {e}");
            return 2;
        }
    };
    let prepared = match waml::validate::prepare(&bundle) {
        Ok(prepared) => prepared,
        Err(error) => {
            eprintln!("waml: {error}");
            return 2;
        }
    };
    let refs = prepared.referrers(slug);
    match q.format {
        Format::Human => {
            if refs.is_empty() {
                println!("no referrers of '{slug}'");
            } else {
                for r in refs {
                    println!("{r}");
                }
            }
        }
        Format::Json => println!(
            "{}",
            serde_json::to_string(&refs).unwrap_or_else(|_| "[]".into())
        ),
    }
    0
}

fn run_bundle(
    dir: &PathBuf,
    format: BundleFormat,
    out: Option<PathBuf>,
    export_name: Option<String>,
) -> i32 {
    let export_name = match (format, export_name) {
        (BundleFormat::Ts, None) => {
            eprintln!("waml: --export-name required when --format ts");
            return 2;
        }
        // The name is interpolated verbatim into `export const {name}`;
        // anything but an identifier would become injected TypeScript.
        (BundleFormat::Ts, Some(name)) if !is_ts_export_name(&name) => {
            eprintln!("waml: --export-name must be a valid identifier, got {name:?}");
            return 2;
        }
        (_, name) => name,
    };
    let bundle = match io::read_files(std::slice::from_ref(dir)) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("waml: {e}");
            return 2;
        }
    };
    let rendered = match format {
        BundleFormat::Json => render_bundle_json(&bundle),
        BundleFormat::Ts => render_bundle_ts(&bundle, &export_name.unwrap()),
    };
    match out {
        Some(path) => {
            if let Err(e) = std::fs::write(&path, rendered) {
                eprintln!("waml: failed to write {}: {e}", path.display());
                return 2;
            }
        }
        None => println!("{rendered}"),
    }
    0
}

/// Write the web editor and this directory's model out as a static site.
///
/// The editor comes from the binary itself, so a build without `embed-web`
/// has nothing to write and says so rather than producing a site that is only
/// half there.
fn run_export_site(dir: &PathBuf, out: &Path, force: bool) -> i32 {
    let artifact = match web_artifact::embedded_artifact() {
        Ok(artifact) => artifact,
        Err(error) => {
            eprintln!("waml: {error}");
            return 2;
        }
    };
    // Rooted, not `read_files`: a bundle path is relative to the model root,
    // so keying documents by the path as typed would put `C:/...` or `/home/...`
    // in the envelope and fail validation on the way out.
    let files = match io::read_bundle_rooted(std::slice::from_ref(dir), false) {
        Ok(files) => files,
        Err(error) => {
            eprintln!("waml: {error}");
            return 2;
        }
    };
    let bundle = match encode_bundle_envelope(&files) {
        Ok(bundle) => bundle,
        Err(error) => {
            eprintln!("waml: {error}");
            return 2;
        }
    };
    // Built from `bundle` itself, not from `files`: the site's boot path
    // hashes the bundle it DECODES, and a hash disagreement is swallowed
    // there (the asset is dropped and the index rebuilt locally), so the two
    // must come from the same bytes -- see
    // `build_search_index_asset_for_envelope`. Search on the exported site
    // still sees exactly what the export shipped, no more (spec
    // §Export-time index boundary rule).
    let search_index = match commands::build_search_index_asset_for_envelope(&bundle) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("waml: {error}");
            return 2;
        }
    };
    let site = match site::assemble_site(
        artifact,
        site::SiteSource::Static {
            bundle: bundle.into_bytes(),
            search_index,
        },
    ) {
        Ok(site) => site,
        Err(error) => {
            eprintln!("waml: {error}");
            return 2;
        }
    };
    if let Err(error) = site::write_site(out, &site, force) {
        eprintln!("waml: {error}");
        return 2;
    }
    println!("wrote {} files to {}", site.len(), out.display());
    0
}

fn run_list(ty: &Option<String>, q: &QueryArgs) -> i32 {
    let bundle = match io::read_bundle_rooted(std::slice::from_ref(&q.dir), false) {
        Ok(bundle) => bundle,
        Err(e) => {
            eprintln!("waml: {e}");
            return 2;
        }
    };
    let prepared = match waml::validate::prepare(&bundle) {
        Ok(prepared) => prepared,
        Err(error) => {
            eprintln!("waml: {error}");
            return 2;
        }
    };
    let model = &prepared.uml().projection;
    for n in &model.nodes {
        if ty.as_deref().map(|t| t == n.ty.as_str()).unwrap_or(true) {
            match q.format {
                Format::Human => println!(
                    "{}\t{}\t{}",
                    n.key,
                    n.ty.as_str(),
                    n.concept.title.as_deref().unwrap_or("Untitled")
                ),
                Format::Json => {
                    println!(
                        "{}",
                        serde_json::json!({"slug": n.key, "type": n.ty.as_str(), "title": n.concept.title.as_deref().unwrap_or("Untitled")})
                    )
                }
            }
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_check_with_json_flag() {
        let cli = Cli::try_parse_from(["waml", "check", "a.md", "--format", "json"]).unwrap();
        match cli.command {
            Command::Check {
                paths,
                format,
                stdin,
            } => {
                assert_eq!(paths.len(), 1);
                assert_eq!(format, Format::Json);
                assert!(!stdin);
            }
            _ => panic!("expected check"),
        }
    }

    #[test]
    fn parses_lsp_stdio_subcommand() {
        let cli = Cli::try_parse_from(["waml", "lsp", "--stdio"]).unwrap();
        assert!(matches!(cli.command, Command::Lsp { stdio: true }));
    }

    #[test]
    fn parses_serve_defaults() {
        let cli = Cli::parse_from(["waml", "serve"]);
        match cli.command {
            Command::Serve {
                ref dir,
                port,
                bind_all,
                api_only,
                no_open,
            } => {
                assert_eq!(dir, std::path::Path::new("."));
                assert_eq!(port, 8099);
                assert!(!bind_all);
                assert!(!api_only);
                assert!(!no_open);
            }
            _ => panic!("expected Serve"),
        }
    }

    #[test]
    fn parses_serve_flags() {
        let cli = Cli::parse_from([
            "waml",
            "serve",
            "docs",
            "--port",
            "0",
            "--bind-all",
            "--api-only",
            "--no-open",
        ]);
        match cli.command {
            Command::Serve {
                ref dir,
                port,
                bind_all,
                api_only,
                no_open,
            } => {
                assert_eq!(dir, std::path::Path::new("docs"));
                assert_eq!(port, 0);
                assert!(bind_all && api_only && no_open);
            }
            _ => panic!("expected Serve"),
        }
    }

    #[test]
    fn parses_fmt_check() {
        let cli = Cli::try_parse_from(["waml", "fmt", "--check", "docs/"]).unwrap();
        assert!(matches!(cli.command, Command::Fmt { check: true, .. }));
    }

    #[test]
    fn parses_attr_add() {
        let cli = Cli::try_parse_from([
            "waml", "attr", "add", "order", "total", "Money", "--mult", "0..1",
        ])
        .unwrap();
        assert!(matches!(cli.command, Command::Attr { .. }));
    }

    #[test]
    fn type_only_attr_set_omits_multiplicity_intent() {
        let cli = Cli::try_parse_from(["waml", "attr", "set", "order", "total", "--type", "Cash"])
            .unwrap();
        let Command::Attr { action, .. } = cli.command else {
            panic!("expected attr command");
        };
        let value = serde_json::to_value(attr_dto(action)).unwrap();

        assert!(value.get("mult").is_none());
    }

    #[test]
    fn clear_mult_attr_set_emits_explicit_null() {
        let cli =
            Cli::try_parse_from(["waml", "attr", "set", "order", "total", "--clear-mult"]).unwrap();
        let Command::Attr { action, .. } = cli.command else {
            panic!("expected attr command");
        };
        let value = serde_json::to_value(attr_dto(action)).unwrap();

        assert!(value.get("mult").is_some_and(serde_json::Value::is_null));
    }

    #[test]
    fn parses_rel_add_with_ends() {
        let cli = Cli::try_parse_from([
            "waml",
            "rel",
            "add",
            "order",
            "composes",
            "order-line",
            "--ends",
            "1 to 1..* lines",
        ])
        .unwrap();
        assert!(matches!(cli.command, Command::Rel { .. }));
    }

    #[test]
    fn parses_apply_and_show() {
        assert!(matches!(
            Cli::try_parse_from(["waml", "apply", "ops.ndjson"])
                .unwrap()
                .command,
            Command::Apply { .. }
        ));
        assert!(matches!(
            Cli::try_parse_from(["waml", "show", "order"])
                .unwrap()
                .command,
            Command::Show { .. }
        ));
    }

    #[test]
    fn parses_bundle_with_ts_format_and_export_name() {
        let cli = Cli::try_parse_from([
            "waml",
            "bundle",
            "dir",
            "--format",
            "ts",
            "--export-name",
            "myBundle",
            "--out",
            "out.ts",
        ])
        .unwrap();
        match cli.command {
            Command::Bundle {
                dir,
                format,
                out,
                export_name,
            } => {
                assert_eq!(dir, PathBuf::from("dir"));
                assert_eq!(format, BundleFormat::Ts);
                assert_eq!(out, Some(PathBuf::from("out.ts")));
                assert_eq!(export_name.as_deref(), Some("myBundle"));
            }
            _ => panic!("expected bundle"),
        }
    }

    #[test]
    fn parses_export_site_with_out_and_force() {
        let cli =
            Cli::try_parse_from(["waml", "export", "site", "docs", "--out", "out", "--force"])
                .unwrap();
        match cli.command {
            Command::Export {
                target: ExportCommand::Site { dir, out, force },
            } => {
                assert_eq!(dir, PathBuf::from("docs"));
                assert_eq!(out, PathBuf::from("out"));
                assert!(force);
            }
            _ => panic!("expected export site"),
        }
    }

    #[test]
    fn parses_export_site_defaults_to_a_site_directory() {
        let cli = Cli::try_parse_from(["waml", "export", "site", "docs"]).unwrap();
        match cli.command {
            Command::Export {
                target: ExportCommand::Site { out, force, .. },
            } => {
                assert_eq!(out, PathBuf::from("./site"));
                assert!(!force);
            }
            _ => panic!("expected export site"),
        }
    }

    #[test]
    fn parses_bundle_default_format_is_json() {
        let cli = Cli::try_parse_from(["waml", "bundle", "dir"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::Bundle {
                format: BundleFormat::Json,
                ..
            }
        ));
    }
}
