use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use syn::visit::{self, Visit};
use syn::{
    Attribute, ExprCall, ExprMethodCall, ExprPath, ExprStruct, FnArg, ImplItem, Item, ItemImpl,
    ItemMod, ItemUse, Local, ReturnType, Signature, Type, UseTree,
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
const GRAMMAR_TYPE_MARKERS: &[&str] = &[
    "Anchored",
    "Layout",
    "Operand",
    "ParsedAttribute",
    "ParsedMember",
    "ParsedRelationship",
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

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct FunctionId {
    module: Vec<String>,
    owner: Option<String>,
    name: String,
}

impl FunctionId {
    fn display(&self) -> String {
        let mut path = self.module.join("::");
        if let Some(owner) = &self.owner {
            path.push_str("::<");
            path.push_str(owner);
            path.push('>');
        }
        path.push_str("::");
        path.push_str(&self.name);
        path
    }
}

#[derive(Clone, Debug)]
struct CallSite {
    segments: Vec<String>,
    method: bool,
    receiver_type: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Capabilities {
    lexical: bool,
    serialize: bool,
    reparse: bool,
    grammar_construct: bool,
}

impl Capabilities {
    fn include(&mut self, other: Capabilities) -> bool {
        let before = *self;
        self.lexical |= other.lexical;
        self.serialize |= other.serialize;
        self.reparse |= other.reparse;
        self.grammar_construct |= other.grammar_construct;
        *self != before
    }
}

#[derive(Clone, Debug)]
struct FunctionSummary {
    path: String,
    id: FunctionId,
    calls: Vec<CallSite>,
    local: Capabilities,
    raw_input: bool,
    model_input: bool,
    output_paths: Vec<Vec<String>>,
    retired_calls: BTreeSet<String>,
    allowlisted_leaf_codec: bool,
}

#[derive(Default)]
struct BodyFacts {
    calls: Vec<CallSite>,
    local: Capabilities,
    retired_calls: BTreeSet<String>,
    receiver_types: BTreeMap<String, String>,
}

impl<'ast> Visit<'ast> for BodyFacts {
    fn visit_expr_call(&mut self, node: &'ast ExprCall) {
        if let syn::Expr::Path(ExprPath { path, .. }) = &*node.func {
            let segments = path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect::<Vec<_>>();
            if let Some(name) = segments.last() {
                let free_reparse = segments.len() == 1 || name != "parse";
                self.record_capability(name, false, free_reparse);
            }
            self.calls.push(CallSite {
                segments,
                method: false,
                receiver_type: None,
            });
        }
        visit::visit_expr_call(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast ExprMethodCall) {
        let name = node.method.to_string();
        self.record_capability(&name, true, false);
        let receiver_type = match &*node.receiver {
            syn::Expr::Path(path) => path
                .path
                .segments
                .last()
                .and_then(|segment| self.receiver_types.get(&segment.ident.to_string()))
                .cloned(),
            _ => None,
        };
        self.calls.push(CallSite {
            segments: vec![name],
            method: true,
            receiver_type,
        });
        visit::visit_expr_method_call(self, node);
    }

    fn visit_expr_struct(&mut self, node: &'ast ExprStruct) {
        if node.path.segments.last().is_some_and(|segment| {
            HIGH_LEVEL_GRAMMAR_TYPES.contains(&segment.ident.to_string().as_str())
        }) {
            self.local.grammar_construct = true;
        }
        visit::visit_expr_struct(self, node);
    }

    fn visit_local(&mut self, node: &'ast Local) {
        let (binding, explicit) = match &node.pat {
            syn::Pat::Ident(pattern) => (Some(pattern.ident.to_string()), None),
            syn::Pat::Type(pattern) => (
                match &*pattern.pat {
                    syn::Pat::Ident(pattern) => Some(pattern.ident.to_string()),
                    _ => None,
                },
                type_names(&pattern.ty).last().cloned(),
            ),
            _ => (None, None),
        };
        let inferred = node
            .init
            .as_ref()
            .and_then(|init| expression_type_name(&init.expr));
        if let (Some(binding), Some(ty)) = (binding, explicit.or(inferred)) {
            self.receiver_types.insert(binding, ty);
        }
        visit::visit_local(self, node);
    }
}

impl BodyFacts {
    fn record_capability(&mut self, name: &str, method: bool, free_reparse: bool) {
        self.local.lexical |= LEXICAL_METHODS.contains(&name);
        self.local.serialize |= SERIALIZATION_CALLS.contains(&name);
        self.local.reparse |= !method && free_reparse && REPARSE_CALLS.contains(&name);
        if RETIRED_CALLS.contains(&name) {
            self.retired_calls.insert(name.to_owned());
        }
    }
}

pub fn analyze_workspace(workspace: &Path) -> io::Result<Vec<Violation>> {
    let crates = workspace.join("crates");
    let mut paths = Vec::new();
    for entry in fs::read_dir(&crates)? {
        let crate_root = entry?.path();
        if !crate_root.is_dir() {
            continue;
        }
        for production_dir in ["src", "examples"] {
            let root = crate_root.join(production_dir);
            if root.is_dir() {
                rust_sources(&root, &mut paths)?;
            }
        }
        let build = crate_root.join("build.rs");
        if build.is_file() {
            paths.push(build);
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
    let mut imports = BTreeMap::new();
    let mut aliases = BTreeMap::new();

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
        let module = module_for_file(path);
        analyze_items(
            path,
            &parsed.items,
            &module,
            &mut summaries,
            &mut imports,
            &mut aliases,
            &mut violations,
        );
    }

    let edges = resolve_edges(&summaries, &imports);
    let mut capabilities = summaries
        .iter()
        .map(|summary| summary.local)
        .collect::<Vec<_>>();
    loop {
        let previous = capabilities.clone();
        let mut changed = false;
        for (caller, targets) in edges.iter().enumerate() {
            for target in targets {
                changed |= capabilities[caller].include(previous[*target]);
            }
        }
        if !changed {
            break;
        }
    }

    let direct_shadow = summaries
        .iter()
        .enumerate()
        .filter(|(index, summary)| {
            !summary.allowlisted_leaf_codec
                && summary.raw_input
                && is_domain_output(summary, &aliases, &imports)
                && capabilities[*index].lexical
        })
        .map(|(index, _)| index)
        .collect::<BTreeSet<_>>();
    let model_reparse = summaries
        .iter()
        .enumerate()
        .filter(|(index, summary)| {
            summary.model_input && capabilities[*index].serialize && capabilities[*index].reparse
        })
        .map(|(index, _)| index)
        .collect::<BTreeSet<_>>();
    let retired = summaries
        .iter()
        .enumerate()
        .filter(|(_, summary)| !summary.retired_calls.is_empty())
        .map(|(index, _)| index)
        .collect::<BTreeSet<_>>();
    let seeds = direct_shadow
        .union(&model_reparse)
        .copied()
        .chain(retired.iter().copied())
        .collect::<BTreeSet<_>>();
    let reaches_shadow = reverse_reachable(&edges, &seeds);

    for (index, summary) in summaries.iter().enumerate() {
        if direct_shadow.contains(&index) {
            violations.push(Violation {
                path: summary.path.clone(),
                reason: format!(
                    "`{}` defines raw-text grammar outside the syntax authority",
                    summary.id.display()
                ),
            });
        }
        if model_reparse.contains(&index) {
            violations.push(Violation {
                path: summary.path.clone(),
                reason: format!(
                    "`{}` performs a model-to-source reparse",
                    summary.id.display()
                ),
            });
        }
        for retired_call in &summary.retired_calls {
            violations.push(Violation {
                path: summary.path.clone(),
                reason: format!(
                    "`{}` calls deleted authority `{retired_call}`",
                    summary.id.display()
                ),
            });
        }
        if !seeds.contains(&index) && reaches_shadow.contains(&index) {
            let target = edges[index]
                .iter()
                .find(|target| reaches_shadow.contains(target))
                .map(|target| summaries[*target].id.display())
                .unwrap_or_else(|| "<shadow authority>".into());
            violations.push(Violation {
                path: summary.path.clone(),
                reason: format!(
                    "`{}` reaches shadow authority `{target}`",
                    summary.id.display()
                ),
            });
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

fn analyze_items(
    path: &str,
    items: &[Item],
    module: &[String],
    summaries: &mut Vec<FunctionSummary>,
    imports: &mut BTreeMap<Vec<String>, BTreeMap<String, Vec<String>>>,
    aliases: &mut BTreeMap<(Vec<String>, String), Vec<Vec<String>>>,
    violations: &mut Vec<Violation>,
) {
    let authoritative = is_authoritative_path(path);
    for item in items {
        match item {
            Item::Mod(item_mod) if !is_test_only(&item_mod.attrs) => {
                inspect_module(path, item_mod, violations);
                if let Some((_, nested)) = &item_mod.content {
                    let mut nested_module = module.to_vec();
                    nested_module.push(item_mod.ident.to_string());
                    analyze_items(
                        path,
                        nested,
                        &nested_module,
                        summaries,
                        imports,
                        aliases,
                        violations,
                    );
                }
            }
            Item::Use(item_use) if !is_test_only(&item_use.attrs) => {
                inspect_use(path, item_use, violations);
                collect_use_bindings(
                    Vec::new(),
                    &item_use.tree,
                    imports.entry(module.to_vec()).or_default(),
                );
            }
            Item::Type(item_type) if !is_test_only(&item_type.attrs) => {
                aliases.insert(
                    (module.to_vec(), item_type.ident.to_string()),
                    type_paths(&item_type.ty),
                );
            }
            Item::Fn(item_fn) if !is_test_only(&item_fn.attrs) => summarize_function(
                path,
                module,
                None,
                &item_fn.sig,
                &item_fn.block,
                authoritative,
                summaries,
            ),
            Item::Impl(item_impl) if !is_test_only(&item_impl.attrs) => {
                summarize_impl(path, module, item_impl, authoritative, summaries)
            }
            _ => {}
        }
    }
}

fn summarize_impl(
    path: &str,
    module: &[String],
    item_impl: &ItemImpl,
    authoritative: bool,
    summaries: &mut Vec<FunctionSummary>,
) {
    let owner = type_names(&item_impl.self_ty)
        .last()
        .cloned()
        .unwrap_or_else(|| "_".into());
    for item in &item_impl.items {
        if let ImplItem::Fn(method) = item {
            if is_test_only(&method.attrs) {
                continue;
            }
            summarize_function(
                path,
                module,
                Some(owner.clone()),
                &method.sig,
                &method.block,
                authoritative,
                summaries,
            );
        }
    }
}

fn summarize_function(
    path: &str,
    module: &[String],
    owner: Option<String>,
    signature: &Signature,
    block: &syn::Block,
    authoritative: bool,
    summaries: &mut Vec<FunctionSummary>,
) {
    if authoritative {
        return;
    }
    let mut facts = BodyFacts::default();
    for argument in &signature.inputs {
        match argument {
            FnArg::Receiver(_) => {
                if let Some(owner) = &owner {
                    facts.receiver_types.insert("self".into(), owner.clone());
                }
            }
            FnArg::Typed(argument) => {
                if let syn::Pat::Ident(pattern) = &*argument.pat {
                    if let Some(name) = type_names(&argument.ty).last() {
                        facts
                            .receiver_types
                            .insert(pattern.ident.to_string(), name.clone());
                    }
                }
            }
        }
    }
    facts.visit_block(block);

    let raw_input = signature.inputs.iter().any(|argument| match argument {
        FnArg::Receiver(_) => false,
        FnArg::Typed(argument) => type_contains_any(&argument.ty, &["str", "String"]),
    });
    let model_input = signature.inputs.iter().any(|argument| match argument {
        FnArg::Receiver(_) => false,
        FnArg::Typed(argument) => type_contains_any(&argument.ty, MODEL_TYPES),
    });
    let output_paths = match &signature.output {
        ReturnType::Default => Vec::new(),
        ReturnType::Type(_, output) => type_paths(output),
    };
    let name = signature.ident.to_string();
    let id = FunctionId {
        module: module.to_vec(),
        owner,
        name: name.clone(),
    };

    summaries.push(FunctionSummary {
        path: path.to_owned(),
        id: id.clone(),
        calls: facts.calls,
        local: facts.local,
        raw_input,
        model_input,
        output_paths,
        retired_calls: facts.retired_calls,
        allowlisted_leaf_codec: is_allowlisted_leaf_codec(&id),
    });
}

fn resolve_edges(
    summaries: &[FunctionSummary],
    imports: &BTreeMap<Vec<String>, BTreeMap<String, Vec<String>>>,
) -> Vec<BTreeSet<usize>> {
    summaries
        .iter()
        .map(|caller| {
            caller
                .calls
                .iter()
                .flat_map(|call| resolve_call(caller, call, summaries, imports))
                .collect()
        })
        .collect()
}

fn resolve_call(
    caller: &FunctionSummary,
    call: &CallSite,
    summaries: &[FunctionSummary],
    imports: &BTreeMap<Vec<String>, BTreeMap<String, Vec<String>>>,
) -> Vec<usize> {
    let Some(name) = call.segments.last() else {
        return Vec::new();
    };
    if call.segments.len() == 1 && !call.method {
        if let Some(target) = imports
            .get(&caller.id.module)
            .and_then(|bindings| bindings.get(name))
        {
            return resolve_call(
                caller,
                &CallSite {
                    segments: target.clone(),
                    method: false,
                    receiver_type: None,
                },
                summaries,
                imports,
            );
        }
    }
    let associated_owner = (!call.method && call.segments.len() >= 2)
        .then(|| call.segments[call.segments.len() - 2].clone())
        .filter(|owner| owner.chars().next().is_some_and(char::is_uppercase));
    let compatible = |candidate: &&FunctionSummary| {
        candidate.id.name == *name
            && if call.method {
                call.receiver_type
                    .as_ref()
                    .is_some_and(|owner| candidate.id.owner.as_ref() == Some(owner))
            } else if let Some(owner) = &associated_owner {
                candidate.id.owner.as_ref() == Some(owner)
            } else {
                candidate.id.owner.is_none()
            }
    };
    let named = summaries
        .iter()
        .enumerate()
        .filter(|(_, candidate)| compatible(candidate))
        .collect::<Vec<_>>();

    if call.segments.len() == 1 {
        let local = named
            .iter()
            .filter(|(_, candidate)| candidate.id.module == caller.id.module)
            .map(|(index, _)| *index)
            .collect::<Vec<_>>();
        if local.len() == 1 {
            return local;
        }
        if named.len() == 1 {
            return vec![named[0].0];
        }
        return Vec::new();
    }

    if associated_owner.is_some() {
        let local = named
            .iter()
            .filter(|(_, candidate)| candidate.id.module == caller.id.module)
            .map(|(index, _)| *index)
            .collect::<Vec<_>>();
        return if local.len() == 1 {
            local
        } else if named.len() == 1 {
            vec![named[0].0]
        } else {
            Vec::new()
        };
    }

    let mut expected = caller.id.module.clone();
    let mut segments = call.segments.as_slice();
    match segments.first().map(String::as_str) {
        Some("crate") => {
            expected.truncate(1);
            segments = &segments[1..];
        }
        Some("self") => segments = &segments[1..],
        Some("super") => {
            expected.pop();
            segments = &segments[1..];
        }
        _ => expected.truncate(1),
    }
    expected.extend(segments[..segments.len().saturating_sub(1)].iter().cloned());
    named
        .into_iter()
        .filter(|(_, candidate)| candidate.id.module == expected)
        .map(|(index, _)| index)
        .collect()
}

fn reverse_reachable(edges: &[BTreeSet<usize>], seeds: &BTreeSet<usize>) -> BTreeSet<usize> {
    let mut reachable = seeds.clone();
    loop {
        let before = reachable.len();
        for (caller, targets) in edges.iter().enumerate() {
            if targets.iter().any(|target| reachable.contains(target)) {
                reachable.insert(caller);
            }
        }
        if reachable.len() == before {
            return reachable;
        }
    }
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

fn collect_use_bindings(
    prefix: Vec<String>,
    tree: &UseTree,
    out: &mut BTreeMap<String, Vec<String>>,
) {
    match tree {
        UseTree::Path(path) => {
            let mut next = prefix;
            next.push(path.ident.to_string());
            collect_use_bindings(next, &path.tree, out);
        }
        UseTree::Name(name) => {
            let mut target = prefix;
            target.push(name.ident.to_string());
            out.insert(name.ident.to_string(), target);
        }
        UseTree::Rename(rename) => {
            let mut target = prefix;
            target.push(rename.ident.to_string());
            out.insert(rename.rename.to_string(), target);
        }
        UseTree::Group(group) => {
            for item in &group.items {
                collect_use_bindings(prefix.clone(), item, out);
            }
        }
        UseTree::Glob(_) => {}
    }
}

fn is_domain_output(
    summary: &FunctionSummary,
    aliases: &BTreeMap<(Vec<String>, String), Vec<Vec<String>>>,
    imports: &BTreeMap<Vec<String>, BTreeMap<String, Vec<String>>>,
) -> bool {
    let mut pending = summary
        .output_paths
        .iter()
        .cloned()
        .map(|path| (summary.id.module.clone(), path))
        .collect::<Vec<_>>();
    let mut seen = BTreeSet::new();
    while let Some((context, path)) = pending.pop() {
        let Some(name) = path.last().cloned() else {
            continue;
        };
        if HIGH_LEVEL_GRAMMAR_TYPES.contains(&name.as_str())
            || GRAMMAR_TYPE_MARKERS
                .iter()
                .any(|marker| name.contains(marker))
        {
            return true;
        }
        let (module, alias) = resolve_type_path(&context, &path, imports);
        if !seen.insert((module.clone(), alias.clone())) {
            continue;
        }
        if let Some(targets) = aliases.get(&(module.clone(), alias)) {
            pending.extend(
                targets
                    .iter()
                    .cloned()
                    .map(|target| (module.clone(), target)),
            );
        }
    }
    false
}

fn resolve_type_path(
    context: &[String],
    path: &[String],
    imports: &BTreeMap<Vec<String>, BTreeMap<String, Vec<String>>>,
) -> (Vec<String>, String) {
    let name = path.last().cloned().unwrap_or_default();
    if path.len() == 1 {
        if let Some(imported) = imports
            .get(context)
            .and_then(|bindings| bindings.get(&name))
        {
            return resolve_type_path(context, imported, imports);
        }
        return (context.to_vec(), name);
    }
    let mut module = context.to_vec();
    let mut segments = path;
    match segments.first().map(String::as_str) {
        Some("crate") => {
            module.truncate(1);
            segments = &segments[1..];
        }
        Some("self") => segments = &segments[1..],
        Some("super") => {
            module.pop();
            segments = &segments[1..];
        }
        _ => module.truncate(1),
    }
    module.extend(segments[..segments.len().saturating_sub(1)].iter().cloned());
    (module, name)
}

fn expression_type_name(expression: &syn::Expr) -> Option<String> {
    match expression {
        syn::Expr::Struct(expression) => expression
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string()),
        syn::Expr::Path(expression) => expression
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string()),
        syn::Expr::Call(expression) => match &*expression.func {
            syn::Expr::Path(function) => {
                let segments = function.path.segments.iter().collect::<Vec<_>>();
                match segments.as_slice() {
                    [.., owner, method]
                        if method
                            .ident
                            .to_string()
                            .chars()
                            .next()
                            .is_some_and(char::is_lowercase) =>
                    {
                        Some(owner.ident.to_string())
                    }
                    [.., constructor] => Some(constructor.ident.to_string()),
                    [] => None,
                }
            }
            _ => None,
        },
        _ => None,
    }
}

fn is_test_only(attributes: &[Attribute]) -> bool {
    attributes.iter().any(|attribute| {
        attribute.path().is_ident("test")
            || (attribute.path().is_ident("cfg") && {
                attribute.meta.require_list().is_ok_and(|list| {
                    let predicate = list.tokens.to_string().replace(' ', "");
                    predicate == "test" || predicate.starts_with("all(test,")
                })
            })
    })
}

fn module_for_file(path: &str) -> Vec<String> {
    let segments = path.split('/').collect::<Vec<_>>();
    let crate_name = segments.get(1).copied().unwrap_or("fixture").to_owned();
    let mut module = vec![crate_name];
    let Some(src) = segments.iter().position(|segment| *segment == "src") else {
        let stem = Path::new(path)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("build");
        module.push(stem.to_owned());
        return module;
    };
    let tail = &segments[src + 1..];
    for (index, segment) in tail.iter().enumerate() {
        let last = index + 1 == tail.len();
        let value = segment.strip_suffix(".rs").unwrap_or(segment);
        if last && matches!(value, "lib" | "main" | "mod") {
            continue;
        }
        module.push(value.to_owned());
    }
    module
}

fn type_contains_any(ty: &Type, names: &[&str]) -> bool {
    type_names(ty)
        .iter()
        .any(|name| names.contains(&name.as_str()))
}

fn type_names(ty: &Type) -> Vec<String> {
    let mut names = Vec::new();
    visit_type_names(ty, &mut |name| names.push(name.to_owned()));
    names
}

fn type_paths(ty: &Type) -> Vec<Vec<String>> {
    let mut paths = Vec::new();
    visit_type_paths(ty, &mut paths);
    paths
}

fn visit_type_paths(ty: &Type, paths: &mut Vec<Vec<String>>) {
    match ty {
        Type::Array(array) => visit_type_paths(&array.elem, paths),
        Type::BareFn(function) => {
            for input in &function.inputs {
                visit_type_paths(&input.ty, paths);
            }
            if let ReturnType::Type(_, output) = &function.output {
                visit_type_paths(output, paths);
            }
        }
        Type::Group(group) => visit_type_paths(&group.elem, paths),
        Type::Paren(paren) => visit_type_paths(&paren.elem, paths),
        Type::Path(path) => {
            paths.push(
                path.path
                    .segments
                    .iter()
                    .map(|segment| segment.ident.to_string())
                    .collect(),
            );
            for segment in &path.path.segments {
                if let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments {
                    for argument in &arguments.args {
                        if let syn::GenericArgument::Type(argument) = argument {
                            visit_type_paths(argument, paths);
                        }
                    }
                }
            }
        }
        Type::Ptr(pointer) => visit_type_paths(&pointer.elem, paths),
        Type::Reference(reference) => visit_type_paths(&reference.elem, paths),
        Type::Slice(slice) => visit_type_paths(&slice.elem, paths),
        Type::Tuple(tuple) => {
            for element in &tuple.elems {
                visit_type_paths(element, paths);
            }
        }
        _ => {}
    }
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

fn is_allowlisted_leaf_codec(id: &FunctionId) -> bool {
    id.owner.is_none()
        && matches!(
            (id.module.as_slice(), id.name.as_str()),
            ([crate_name, module], "parse_value")
                if crate_name == "waml" && module == "frontmatter"
        )
        || id.owner.is_none()
            && matches!(
                (id.module.as_slice(), id.name.as_str()),
                ([crate_name, module], "parse_end" | "parse_ends")
                    if crate_name == "waml" && module == "model"
            )
        || id.owner.is_none()
            && matches!(
                (id.module.as_slice(), id.name.as_str()),
                ([crate_name, first, second], "parse_selector")
                    if crate_name == "waml" && first == "uml" && second == "selector"
            )
        || id.owner.is_none()
            && matches!(
                (id.module.as_slice(), id.name.as_str()),
                ([crate_name, first, second], "parse_link_ref" | "parse_link_in_text")
                    if crate_name == "waml" && first == "uml" && second == "analysis"
            )
        || id.owner.is_none()
            && matches!(
                (id.module.as_slice(), id.name.as_str()),
                ([crate_name, module], "parse_hex")
                    if crate_name == "waml-editor" && module == "cli"
            )
        || id.owner.is_none()
            && matches!(
                (id.module.as_slice(), id.name.as_str()),
                ([crate_name, module], "placement_candidate" | "placement_preview")
                    if crate_name == "waml-editor" && module == "scene"
            )
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
