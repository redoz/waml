use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::ops_dto::OpDto;

mod commands;
mod io;
mod lsp;
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
    /// Run the UAML language server (stdio LSP).
    Lsp {
        /// Use stdio transport (the only supported transport in Phase 1).
        ///
        /// `overrides_with_self` lets the flag be passed more than once without
        /// error: the LSP client appends its own `--stdio` (TransportKind.stdio)
        /// on top of any we pass, and a repeated flag must not abort the server.
        #[arg(long, overrides_with = "stdio")]
        stdio: bool,
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
        Command::Lsp { stdio: _ } => lsp::run(),
        Command::Node { action, common } => run_mutation(&common, node_dto(action)),
        Command::Attr { action, common } => run_mutation(&common, attr_dto(action)),
        Command::Value { action, common } => run_mutation(&common, value_dto(action)),
        Command::Rel { action, common } => run_mutation(&common, rel_dto(action)),
        Command::Apply { ops, common } => run_apply(&ops, &common),
        Command::Show { slug, query } => run_show(&slug, &query),
        Command::Refs { slug, query } => run_refs(&slug, &query),
        Command::List { r#type, query } => run_list(&r#type, &query),
    };
    std::process::exit(code);
}

fn node_dto(a: NodeCmd) -> OpDto {
    match a {
        NodeCmd::New { slug, r#type, title, stereotype, desc, r#abstract } => {
            OpDto::NodeNew { v: 1, slug, ty: r#type, title, stereotype, desc, abstract_: r#abstract }
        }
        NodeCmd::Rename { from, to } => OpDto::NodeRename { v: 1, from, to },
        NodeCmd::Set { slug, title, desc, stereotype, r#abstract, r#type } => {
            OpDto::NodeSet { v: 1, slug, title, desc, stereotype, abstract_: r#abstract, ty: r#type }
        }
        NodeCmd::Rm { slug, cascade } => OpDto::NodeRm { v: 1, slug, cascade },
    }
}

fn attr_dto(a: AttrCmd) -> OpDto {
    match a {
        AttrCmd::Add { node, name, r#type, mult, vis } => {
            OpDto::AttrAdd { v: 1, node, name, ty: r#type, mult, vis }
        }
        AttrCmd::Set { node, name, r#type, mult, vis, rename } => {
            OpDto::AttrSet { v: 1, node, name, ty: r#type, mult, vis, rename }
        }
        AttrCmd::Rm { node, name } => OpDto::AttrRm { v: 1, node, name },
    }
}

fn value_dto(a: ValueCmd) -> OpDto {
    match a {
        ValueCmd::Add { node, literal } => OpDto::ValueAdd { v: 1, node, literal },
        ValueCmd::Rm { node, literal } => OpDto::ValueRm { v: 1, node, literal },
    }
}

fn rel_dto(a: RelCmd) -> OpDto {
    match a {
        RelCmd::Add { source, verb, target, ends, as_label, as_ref } => {
            OpDto::RelAdd { v: 1, source, kind: verb, target, as_label, as_ref, ends }
        }
        RelCmd::Set { source, verb, target, as_sel, ends, set_label, set_as_ref } => {
            OpDto::RelSet { v: 1, source, kind: verb, target, as_sel, ends, set_label, set_as_ref }
        }
        RelCmd::Rm { source, verb, target, as_sel } => OpDto::RelRm { v: 1, source, kind: verb, target, as_sel },
    }
}

fn to_blob(bundle: &[(String, String)]) -> String {
    bundle.iter().map(|(p, c)| format!("<!-- {p} -->\n{c}")).collect::<Vec<_>>().join("\n")
}

fn run_mutation(common: &Common, dto: OpDto) -> i32 {
    if [common.emit, common.stdout, common.dry_run].iter().filter(|x| **x).count() > 1 {
        eprintln!("uaml: --emit, --stdout, --dry-run mutually exclusive");
        return 2;
    }
    if common.emit {
        return match serde_json::to_string(&dto) {
            Ok(line) => {
                println!("{line}");
                0
            }
            Err(e) => {
                eprintln!("uaml: {e}");
                2
            }
        };
    }
    let op = match dto.to_op() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("uaml: {e}");
            return 1;
        }
    };
    run_batch(common, vec![op])
}

fn run_batch(common: &Common, ops: Vec<uaml::ops::Op>) -> i32 {
    let bundle = match io::read_files(std::slice::from_ref(&common.dir)) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("uaml: {e}");
            return 2;
        }
    };
    match uaml::ops::apply(&bundle, &ops) {
        Ok(new) => {
            if common.stdout {
                print!("{}", to_blob(&new));
                0
            } else if common.dry_run {
                print!("{}", commands::render_diff(&bundle, &new));
                0
            } else {
                match io::write_back(&bundle, &new) {
                    Ok(touched) => {
                        for t in touched {
                            println!("uaml: {t}");
                        }
                        0
                    }
                    Err(e) => {
                        eprintln!("uaml: {e}");
                        2
                    }
                }
            }
        }
        Err(e) => {
            let sel = e.selector.as_ref().map(|s| format!(" [{s}]")).unwrap_or_default();
            eprintln!("uaml: op {}: {}{sel}", e.index, e.reason);
            1
        }
    }
}

fn run_apply(ops_src: &str, common: &Common) -> i32 {
    let lines = match io::read_ndjson(ops_src) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("uaml: {e}");
            return 2;
        }
    };
    let mut ops = Vec::new();
    for (n, line) in lines {
        let dto: OpDto = match serde_json::from_str(&line) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("uaml: line {n}: {e}");
                return 1;
            }
        };
        match dto.to_op() {
            Ok(o) => ops.push(o),
            Err(e) => {
                eprintln!("uaml: line {n}: {e}");
                return 1;
            }
        }
    }
    run_batch(common, ops)
}

fn run_show(slug: &str, q: &QueryArgs) -> i32 {
    let bundle = match io::read_files(std::slice::from_ref(&q.dir)) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("uaml: {e}");
            return 2;
        }
    };
    let model = uaml::parse::build_model(&bundle);
    let Some(node) = model.node(slug) else {
        eprintln!("uaml: no classifier '{slug}'");
        return 1;
    };
    match q.format {
        Format::Human => {
            println!("{} ({})", node.title, node.ty.as_str());
            for a in &node.attributes {
                println!("  - {}: {} {{{}}}", a.name, a.ty.name, a.multiplicity.as_str());
            }
            for v in &node.values {
                println!("  = {v}");
            }
            for e in model.edges.iter().filter(|e| e.source == slug || e.target == slug) {
                println!("  {} {} {}", e.source, e.kind.as_str(), e.target);
            }
            0
        }
        Format::Json => {
            let refs = uaml::ops::referrers(&bundle, slug);
            let dto = serde_json::json!({
                "slug": slug, "title": node.title, "type": node.ty.as_str(),
                "attributes": node.attributes.iter().map(|a| serde_json::json!({
                    "name": a.name, "type": a.ty.name, "ref": a.ty.ref_, "multiplicity": a.multiplicity.as_str()
                })).collect::<Vec<_>>(),
                "values": node.values,
                "referrers": refs,
            });
            println!("{}", serde_json::to_string_pretty(&dto).unwrap_or_else(|_| "{}".into()));
            0
        }
    }
}

fn run_refs(slug: &str, q: &QueryArgs) -> i32 {
    let bundle = match io::read_files(std::slice::from_ref(&q.dir)) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("uaml: {e}");
            return 2;
        }
    };
    let refs = uaml::ops::referrers(&bundle, slug);
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
        Format::Json => println!("{}", serde_json::to_string(&refs).unwrap_or_else(|_| "[]".into())),
    }
    0
}

fn run_list(ty: &Option<String>, q: &QueryArgs) -> i32 {
    let bundle = match io::read_files(std::slice::from_ref(&q.dir)) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("uaml: {e}");
            return 2;
        }
    };
    let model = uaml::parse::build_model(&bundle);
    for n in &model.nodes {
        if ty.as_deref().map(|t| t == n.ty.as_str()).unwrap_or(true) {
            match q.format {
                Format::Human => println!("{}\t{}\t{}", n.key, n.ty.as_str(), n.title),
                Format::Json => {
                    println!("{}", serde_json::json!({"slug": n.key, "type": n.ty.as_str(), "title": n.title}))
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
    fn parses_lsp_stdio_subcommand() {
        let cli = Cli::try_parse_from(["uaml", "lsp", "--stdio"]).unwrap();
        assert!(matches!(cli.command, Command::Lsp { stdio: true }));
    }

    #[test]
    fn parses_fmt_check() {
        let cli = Cli::try_parse_from(["uaml", "fmt", "--check", "docs/"]).unwrap();
        assert!(matches!(cli.command, Command::Fmt { check: true, .. }));
    }

    #[test]
    fn parses_attr_add() {
        let cli = Cli::try_parse_from(["uaml", "attr", "add", "order", "total", "Money", "--mult", "0..1"]).unwrap();
        assert!(matches!(cli.command, Command::Attr { .. }));
    }

    #[test]
    fn parses_rel_add_with_ends() {
        let cli = Cli::try_parse_from([
            "uaml", "rel", "add", "order", "composes", "order-line", "--ends", "1 to 1..* lines",
        ])
        .unwrap();
        assert!(matches!(cli.command, Command::Rel { .. }));
    }

    #[test]
    fn parses_apply_and_show() {
        assert!(matches!(Cli::try_parse_from(["uaml", "apply", "ops.ndjson"]).unwrap().command, Command::Apply { .. }));
        assert!(matches!(Cli::try_parse_from(["uaml", "show", "order"]).unwrap().command, Command::Show { .. }));
    }
}
