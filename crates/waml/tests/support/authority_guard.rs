use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use syn::visit::{self, Visit};
use syn::{
    ExprCall, ExprMethodCall, ExprPath, ExprStruct, File, FnArg, ImplItem, Item, ItemFn, ItemImpl,
    ItemMod, ItemUse, ReturnType, Signature, Type, UseTree,
};

const DELETED_ROOT_MODULES: &[&str] = &["grammar", "parse", "serialize", "syntax"];
const HIGH_LEVEL_GRAMMAR_TYPES: &[&str] = &[
    "Anchored",
    "DeclaredLayoutStatement",
    "LayoutStatement",
    "MemberGroup",
    "Operand",
    "ParsedAttribute",
    "ParsedMember",
    "ParsedRelationship",
];
const MODEL_TYPES: &[&str] = &[
    "ClassDiagram",
    "DocumentDto",
    "Model",
    "Projection",
    "UmlModel",
];
const LEXICAL_METHODS: &[&str] = &[
    "chars",
    "find",
    "lines",
    "match_indices",
    "split",
    "split_ascii_whitespace",
    "split_once",
    "split_terminator",
    "split_whitespace",
    "strip_prefix",
    "strip_suffix",
    "trim",
    "trim_end",
    "trim_matches",
    "trim_start",
];
const SERIALIZATION_CALLS: &[&str] = &[
    "render",
    "serialize",
    "to_markdown",
    "to_source",
    "to_string",
];
const REPARSE_CALLS: &[&str] = &[
    "analyze",
    "from_source",
    "parse",
    "parse_document",
    "prepare_candidate",
];
const RETIRED_CALLS: &[&str] = &[
    "build_model",
    "build_model_from_source",
    "parse_document",
    "project_okf",
    "serialize_document",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Violation {
    pub path: String,
    pub reason: String,
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path, self.reason)
    }
}

#[derive(Clone, Debug)]
struct FunctionSummary {
    path: String,
    name: String,
    calls: BTreeSet<String>,
    direct_shadow: bool,
    model_source_reparse: bool,
    retired_calls: BTreeSet<String>,
}

#[derive(Default)]
struct BodyFacts {
    calls: BTreeSet<String>,
    lexical_methods: BTreeSet<String>,
    serialization_calls: BTreeSet<String>,
    reparse_calls: BTreeSet<String>,
    retired_calls: BTreeSet<String>,
    grammar_constructions: BTreeSet<String>,
}

impl<'ast> Visit<'ast> for BodyFacts {
    fn visit_expr_call(&mut self, node: &'ast ExprCall) {
        if let syn::Expr::Path(ExprPath { path, .. }) = &*node.func {
            if let Some(name) = path
                .segments
                .last()
                .map(|segment| segment.ident.to_string())
            {
                self.record_call(name, false);
            }
        }
        visit::visit_expr_call(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast ExprMethodCall) {
        let name = node.method.to_string();
        self.record_call(name, true);
        visit::visit_expr_method_call(self, node);
    }

    fn visit_expr_struct(&mut self, node: &'ast ExprStruct) {
        if let Some(name) = node
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string())
        {
            if HIGH_LEVEL_GRAMMAR_TYPES.contains(&name.as_str()) {
                self.grammar_constructions.insert(name);
            }
        }
        visit::visit_expr_struct(self, node);
    }
}

impl BodyFacts {
    fn record_call(&mut self, name: String, method: bool) {
        self.calls.insert(name.clone());
        if LEXICAL_METHODS.contains(&name.as_str()) {
            self.lexical_methods.insert(name.clone());
        }
        if SERIALIZATION_CALLS.contains(&name.as_str()) {
            self.serialization_calls.insert(name.clone());
        }
        if !method && REPARSE_CALLS.contains(&name.as_str()) {
            self.reparse_calls.insert(name.clone());
        }
        if RETIRED_CALLS.contains(&name.as_str()) {
            self.retired_calls.insert(name);
        }
    }
}

pub fn analyze_workspace(workspace: &Path) -> io::Result<Vec<Violation>> {
    let crates = workspace.join("crates");
    let mut paths = Vec::new();
    for entry in fs::read_dir(&crates)? {
        let source = entry?.path().join("src");
        if source.is_dir() {
            rust_sources(&source, &mut paths)?;
        }
    }
    paths.sort();

    let sources = paths
        .into_iter()
        .map(|path| {
            let relative = slash_path(path.strip_prefix(workspace).unwrap_or(&path));
            fs::read_to_string(path).map(|source| (relative, source))
        })
        .collect::<io::Result<Vec<_>>>()?;
    Ok(analyze_sources(
        sources
            .iter()
            .map(|(path, source)| (path.as_str(), source.as_str())),
    ))
}

pub fn analyze_sources<'a>(
    sources: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> Vec<Violation> {
    let mut violations = Vec::new();
    let mut summaries = Vec::new();

    for (path, source) in sources {
        let parsed = match syn::parse_file(source) {
            Ok(parsed) => parsed,
            Err(error) => {
                violations.push(Violation {
                    path: path.to_owned(),
                    reason: format!("production Rust source failed AST parsing: {error}"),
                });
                continue;
            }
        };
        analyze_file(path, &parsed, &mut summaries, &mut violations);
    }

    let name_counts = summaries
        .iter()
        .fold(BTreeMap::new(), |mut counts, summary| {
            *counts.entry(summary.name.clone()).or_insert(0usize) += 1;
            counts
        });
    let mut reachable = summaries
        .iter()
        .filter(|summary| {
            summary.direct_shadow
                || summary.model_source_reparse
                || !summary.retired_calls.is_empty()
        })
        .map(|summary| (summary.path.clone(), summary.name.clone()))
        .collect::<BTreeSet<_>>();
    loop {
        let newly_reachable = summaries
            .iter()
            .filter(|summary| {
                !reachable.contains(&(summary.path.clone(), summary.name.clone()))
                    && summary.calls.iter().any(|call| {
                        reachable.contains(&(summary.path.clone(), call.clone()))
                            || (name_counts.get(call) == Some(&1)
                                && reachable.iter().any(|(_, name)| name == call))
                    })
            })
            .map(|summary| (summary.path.clone(), summary.name.clone()))
            .collect::<Vec<_>>();
        if newly_reachable.is_empty() {
            break;
        }
        reachable.extend(newly_reachable);
    }

    for summary in &summaries {
        if summary.direct_shadow {
            violations.push(Violation {
                path: summary.path.clone(),
                reason: format!(
                    "`{}` defines raw-text grammar outside the syntax authority",
                    summary.name
                ),
            });
        }
        if summary.model_source_reparse {
            violations.push(Violation {
                path: summary.path.clone(),
                reason: format!("`{}` performs a model-to-source reparse", summary.name),
            });
        }
        for retired in &summary.retired_calls {
            violations.push(Violation {
                path: summary.path.clone(),
                reason: format!("`{}` calls deleted authority `{retired}`", summary.name),
            });
        }
        if !summary.direct_shadow
            && !summary.model_source_reparse
            && summary.retired_calls.is_empty()
        {
            if let Some(target) = summary.calls.iter().find(|call| {
                reachable.contains(&(summary.path.clone(), (*call).clone()))
                    || (name_counts.get(*call) == Some(&1)
                        && reachable.iter().any(|(_, name)| name == *call))
            }) {
                violations.push(Violation {
                    path: summary.path.clone(),
                    reason: format!("`{}` reaches shadow authority `{target}`", summary.name),
                });
            }
        }
    }

    violations.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.reason.cmp(&right.reason))
    });
    violations.dedup();
    violations
}

fn analyze_file(
    path: &str,
    file: &File,
    summaries: &mut Vec<FunctionSummary>,
    violations: &mut Vec<Violation>,
) {
    let authoritative = is_authoritative_path(path);
    for item in &file.items {
        match item {
            Item::Mod(item_mod) => {
                inspect_module(path, item_mod, violations);
                inspect_nested_items(
                    path,
                    &item_mod.content,
                    authoritative,
                    summaries,
                    violations,
                );
            }
            Item::Use(item_use) => inspect_use(path, item_use, violations),
            Item::Fn(item_fn) => {
                summarize_function(path, &item_fn.sig, &item_fn.block, authoritative, summaries)
            }
            Item::Impl(item_impl) => {
                summarize_impl(path, item_impl, authoritative, summaries, violations)
            }
            _ => {}
        }
    }
}

fn inspect_nested_items(
    path: &str,
    content: &Option<(syn::token::Brace, Vec<Item>)>,
    authoritative: bool,
    summaries: &mut Vec<FunctionSummary>,
    violations: &mut Vec<Violation>,
) {
    let Some((_, items)) = content else {
        return;
    };
    for item in items {
        match item {
            Item::Mod(item_mod) => {
                inspect_module(path, item_mod, violations);
                inspect_nested_items(
                    path,
                    &item_mod.content,
                    authoritative,
                    summaries,
                    violations,
                );
            }
            Item::Use(item_use) => inspect_use(path, item_use, violations),
            Item::Fn(item_fn) => {
                summarize_function(path, &item_fn.sig, &item_fn.block, authoritative, summaries)
            }
            Item::Impl(item_impl) => {
                summarize_impl(path, item_impl, authoritative, summaries, violations)
            }
            _ => {}
        }
    }
}

fn summarize_impl(
    path: &str,
    item_impl: &ItemImpl,
    authoritative: bool,
    summaries: &mut Vec<FunctionSummary>,
    _violations: &mut Vec<Violation>,
) {
    for item in &item_impl.items {
        if let ImplItem::Fn(method) = item {
            summarize_function(path, &method.sig, &method.block, authoritative, summaries);
        }
    }
}

fn summarize_function(
    path: &str,
    signature: &Signature,
    block: &syn::Block,
    authoritative: bool,
    summaries: &mut Vec<FunctionSummary>,
) {
    let mut facts = BodyFacts::default();
    facts.visit_block(block);

    let raw_input = signature.inputs.iter().any(|argument| match argument {
        FnArg::Receiver(_) => false,
        FnArg::Typed(argument) => type_contains_any(&argument.ty, &["str", "String"]),
    });
    let grammar_output = match &signature.output {
        ReturnType::Default => false,
        ReturnType::Type(_, output) => type_contains_any(output, HIGH_LEVEL_GRAMMAR_TYPES),
    };
    let model_input = signature.inputs.iter().any(|argument| match argument {
        FnArg::Receiver(_) => false,
        FnArg::Typed(argument) => type_contains_any(&argument.ty, MODEL_TYPES),
    });
    let direct_shadow = !authoritative
        && raw_input
        && grammar_output
        && (!facts.lexical_methods.is_empty() || !facts.grammar_constructions.is_empty());
    let model_source_reparse = !authoritative
        && model_input
        && !facts.serialization_calls.is_empty()
        && !facts.reparse_calls.is_empty();

    summaries.push(FunctionSummary {
        path: path.to_owned(),
        name: signature.ident.to_string(),
        calls: facts.calls,
        direct_shadow,
        model_source_reparse,
        retired_calls: facts.retired_calls,
    });
}

fn inspect_module(path: &str, item_mod: &ItemMod, violations: &mut Vec<Violation>) {
    if path == "crates/waml/src/lib.rs"
        && DELETED_ROOT_MODULES.contains(&item_mod.ident.to_string().as_str())
    {
        violations.push(Violation {
            path: path.to_owned(),
            reason: format!(
                "deleted authority path is declared as root module `{}`",
                item_mod.ident
            ),
        });
    }
}

fn inspect_use(path: &str, item_use: &ItemUse, violations: &mut Vec<Violation>) {
    let mut imports = Vec::new();
    flatten_use_tree(Vec::new(), &item_use.tree, &mut imports);
    for import in imports {
        let first = import.first().map(String::as_str);
        let external_waml = first == Some("waml")
            && import
                .get(1)
                .is_some_and(|segment| DELETED_ROOT_MODULES.contains(&segment.as_str()));
        let internal_deleted = path.starts_with("crates/waml/src/")
            && ((first == Some("crate")
                && import
                    .get(1)
                    .is_some_and(|segment| DELETED_ROOT_MODULES.contains(&segment.as_str())))
                || (path == "crates/waml/src/lib.rs"
                    && first.is_some_and(|segment| DELETED_ROOT_MODULES.contains(&segment))));
        if external_waml || internal_deleted {
            violations.push(Violation {
                path: path.to_owned(),
                reason: format!(
                    "deleted authority path is imported as `{}`",
                    import.join("::")
                ),
            });
        }
    }
}

fn flatten_use_tree(prefix: Vec<String>, tree: &UseTree, out: &mut Vec<Vec<String>>) {
    match tree {
        UseTree::Path(path) => {
            let mut next = prefix;
            next.push(path.ident.to_string());
            flatten_use_tree(next, &path.tree, out);
        }
        UseTree::Name(name) => {
            let mut path = prefix;
            path.push(name.ident.to_string());
            out.push(path);
        }
        UseTree::Rename(rename) => {
            let mut path = prefix;
            path.push(rename.ident.to_string());
            out.push(path);
        }
        UseTree::Glob(_) => out.push(prefix),
        UseTree::Group(group) => {
            for item in &group.items {
                flatten_use_tree(prefix.clone(), item, out);
            }
        }
    }
}

fn type_contains_any(ty: &Type, names: &[&str]) -> bool {
    let mut found = false;
    visit_type_names(ty, &mut |name| {
        found |= names.contains(&name);
    });
    found
}

fn visit_type_names(ty: &Type, visit: &mut impl FnMut(&str)) {
    match ty {
        Type::Array(array) => visit_type_names(&array.elem, visit),
        Type::BareFn(function) => {
            for input in &function.inputs {
                visit_type_names(&input.ty, visit);
            }
            if let ReturnType::Type(_, output) = &function.output {
                visit_type_names(output, visit);
            }
        }
        Type::Group(group) => visit_type_names(&group.elem, visit),
        Type::Paren(paren) => visit_type_names(&paren.elem, visit),
        Type::Path(path) => {
            for segment in &path.path.segments {
                visit(&segment.ident.to_string());
                if let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments {
                    for argument in &arguments.args {
                        if let syn::GenericArgument::Type(argument) = argument {
                            visit_type_names(argument, visit);
                        }
                    }
                }
            }
        }
        Type::Ptr(pointer) => visit_type_names(&pointer.elem, visit),
        Type::Reference(reference) => visit_type_names(&reference.elem, visit),
        Type::Slice(slice) => visit_type_names(&slice.elem, visit),
        Type::Tuple(tuple) => {
            for element in &tuple.elems {
                visit_type_names(element, visit);
            }
        }
        _ => {}
    }
}

fn is_authoritative_path(path: &str) -> bool {
    path.starts_with("crates/waml/src/uml/syntax/")
        || path == "crates/waml/src/uml/syntax.rs"
        || path.starts_with("crates/waml-syntax/src/")
}

fn rust_sources(root: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        if path.is_dir() {
            rust_sources(&path, out)?;
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            out.push(path);
        }
    }
    Ok(())
}

fn slash_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
