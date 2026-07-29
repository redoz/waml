//! Task 21's architecture test is structural first:
//! Cargo metadata selects workspace packages and production targets, Rust module
//! declarations select source files, and crate/module visibility seals the raw
//! parser. The AST pass is deliberately residual. It rejects raw-to-grammar
//! signatures, visible model-to-text surfaces, and model-to-authority-reparse
//! reachability; it does not pretend to be rustc type inference or macro
//! expansion. Literal `include!` files are followed. Dynamic/generated includes
//! and local macros that mention protected grammar types are rejected
//! conservatively because their expanded call graph cannot be audited here.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;
use syn::visit::{self, Visit};
use syn::{
    Attribute, ExprCall, ExprMethodCall, ExprPath, FnArg, ImplItem, Item, ItemImpl, ItemMacro,
    ItemMod, ItemUse, Lit, Local, Meta, ReturnType, Signature, Type, UseTree,
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
    serialize: bool,
    reparse: bool,
}

impl Capabilities {
    fn include(&mut self, other: Capabilities) -> bool {
        let before = *self;
        self.serialize |= other.serialize;
        self.reparse |= other.reparse;
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
    visible_model_text: bool,
}

#[derive(Default)]
struct BodyFacts {
    calls: Vec<CallSite>,
    local: Capabilities,
    retired_calls: BTreeSet<String>,
    receiver_types: BTreeMap<String, String>,
    callable_paths: BTreeMap<String, Vec<String>>,
    saw_method_call: bool,
}

impl<'ast> Visit<'ast> for BodyFacts {
    fn visit_expr_call(&mut self, node: &'ast ExprCall) {
        if let syn::Expr::Path(ExprPath { path, .. }) = &*node.func {
            let mut segments = path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect::<Vec<_>>();
            if segments.len() == 1 {
                if let Some(target) = self.callable_paths.get(&segments[0]) {
                    segments = target.clone();
                }
            }
            self.record_capability(&segments);
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
        let receiver_type = match &*node.receiver {
            syn::Expr::Path(path) => path
                .path
                .segments
                .last()
                .and_then(|segment| self.receiver_types.get(&segment.ident.to_string()))
                .cloned(),
            _ => None,
        };
        self.saw_method_call = true;
        self.local.serialize |= receiver_type
            .as_ref()
            .is_some_and(|receiver| MODEL_TYPES.contains(&receiver.as_str()));
        self.calls.push(CallSite {
            segments: vec![name],
            method: true,
            receiver_type,
        });
        visit::visit_expr_method_call(self, node);
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
        if let (Some(binding), Some(ty)) = (binding.as_ref(), explicit.or(inferred)) {
            self.receiver_types.insert(binding.clone(), ty);
        }
        if let (Some(binding), Some(init)) = (binding, &node.init) {
            if let syn::Expr::Path(path) = &*init.expr {
                self.callable_paths.insert(
                    binding,
                    path.path
                        .segments
                        .iter()
                        .map(|segment| segment.ident.to_string())
                        .collect(),
                );
            }
        }
        visit::visit_local(self, node);
    }
}

impl BodyFacts {
    fn record_capability(&mut self, segments: &[String]) {
        self.local.reparse |= is_reparse_path(segments);
        if let Some(name) = segments.last() {
            if RETIRED_CALLS.contains(&name.as_str()) {
                self.retired_calls.insert(name.to_owned());
            }
        }
    }
}

fn cargo_metadata(workspace: &Path) -> io::Result<Value> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = Command::new(cargo)
        .args([
            "metadata",
            "--format-version",
            "1",
            "--no-deps",
            "--manifest-path",
        ])
        .arg(workspace.join("Cargo.toml"))
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "cargo metadata failed for {}: {}",
            workspace.display(),
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|error| io::Error::other(format!("invalid cargo metadata JSON: {error}")))
}

fn dependency_violations(metadata: &Value, workspace: &Path) -> Vec<Violation> {
    let members = metadata
        .get("workspace_members")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<BTreeSet<_>>();
    let workspace_names = metadata
        .get("packages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|package| {
            package
                .get("id")
                .and_then(Value::as_str)
                .is_some_and(|id| members.contains(id))
        })
        .filter_map(|package| package.get("name").and_then(Value::as_str))
        .collect::<BTreeSet<_>>();
    let mut violations = Vec::new();

    for package in metadata
        .get("packages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(id) = package.get("id").and_then(Value::as_str) else {
            continue;
        };
        if !members.contains(id) {
            continue;
        }
        let name = package
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("<unknown>");
        let manifest = package
            .get("manifest_path")
            .and_then(Value::as_str)
            .map(PathBuf::from)
            .unwrap_or_else(|| workspace.join("Cargo.toml"));
        let path = workspace_relative(workspace, &manifest);
        for dependency in package
            .get("dependencies")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if dependency.get("kind").and_then(Value::as_str) == Some("dev") {
                continue;
            }
            let Some(dependency_name) = dependency.get("name").and_then(Value::as_str) else {
                continue;
            };
            if !workspace_names.contains(dependency_name) {
                continue;
            }
            let permitted = match name {
                "waml-syntax" => false,
                "waml" => dependency_name == "waml-syntax",
                "waml-ops-dto" => dependency_name == "waml",
                "waml-cli" => matches!(dependency_name, "waml" | "waml-ops-dto"),
                "waml-editor" => dependency_name == "waml",
                _ => dependency_name != "waml-syntax",
            };
            if !permitted {
                violations.push(Violation {
                    path: path.clone(),
                    reason: format!(
                        "forbidden workspace dependency direction `{name} -> {dependency_name}`"
                    ),
                });
            }
        }
    }
    violations
}

fn is_production_target(target: &Value) -> bool {
    target
        .get("kind")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .any(|kind| {
            matches!(
                kind,
                "lib"
                    | "bin"
                    | "example"
                    | "custom-build"
                    | "proc-macro"
                    | "staticlib"
                    | "cdylib"
                    | "dylib"
            )
        })
}

#[allow(clippy::too_many_arguments)]
fn collect_module_source(
    workspace: &Path,
    source_path: &Path,
    module: Vec<String>,
    module_dir: PathBuf,
    units: &mut Vec<SourceUnit>,
    seen: &mut BTreeSet<(PathBuf, Vec<String>)>,
    violations: &mut Vec<Violation>,
) -> io::Result<()> {
    let canonical = fs::canonicalize(source_path)?;
    if !seen.insert((canonical, module.clone())) {
        return Ok(());
    }
    let source = fs::read_to_string(source_path)?;
    let path = workspace_relative(workspace, source_path);
    let parsed = match syn::parse_file(&source) {
        Ok(parsed) => parsed,
        Err(error) => {
            units.push(SourceUnit {
                path,
                source,
                module,
            });
            violations.push(Violation {
                path: workspace_relative(workspace, source_path),
                reason: format!("production Rust source failed AST parsing: {error}"),
            });
            return Ok(());
        }
    };
    units.push(SourceUnit {
        path,
        source,
        module: module.clone(),
    });
    collect_item_sources(
        workspace,
        source_path,
        &parsed.items,
        &module,
        &module_dir,
        units,
        seen,
        violations,
    )
}

#[allow(clippy::too_many_arguments)]
fn collect_item_sources(
    workspace: &Path,
    containing_source: &Path,
    items: &[Item],
    module: &[String],
    module_dir: &Path,
    units: &mut Vec<SourceUnit>,
    seen: &mut BTreeSet<(PathBuf, Vec<String>)>,
    violations: &mut Vec<Violation>,
) -> io::Result<()> {
    for item in items {
        match item {
            Item::Mod(item_mod) if !is_test_only(&item_mod.attrs) => {
                let mut nested_module = module.to_vec();
                nested_module.push(item_mod.ident.to_string());
                if let Some((_, nested)) = &item_mod.content {
                    collect_item_sources(
                        workspace,
                        containing_source,
                        nested,
                        &nested_module,
                        &module_dir.join(item_mod.ident.to_string()),
                        units,
                        seen,
                        violations,
                    )?;
                } else {
                    let resolved = module_source_path(containing_source, module_dir, item_mod);
                    match resolved {
                        Some(path) => {
                            let parent = path.parent().unwrap_or(module_dir);
                            let next_dir = if path.file_name().is_some_and(|name| name == "mod.rs")
                            {
                                parent.to_path_buf()
                            } else {
                                parent.join(item_mod.ident.to_string())
                            };
                            collect_module_source(
                                workspace,
                                &path,
                                nested_module,
                                next_dir,
                                units,
                                seen,
                                violations,
                            )?;
                        }
                        None => violations.push(Violation {
                            path: workspace_relative(workspace, containing_source),
                            reason: format!(
                                "declared production module `{}` has no auditable source",
                                item_mod.ident
                            ),
                        }),
                    }
                }
            }
            Item::Macro(item_macro)
                if item_macro.mac.path.is_ident("include") && !is_test_only(&item_macro.attrs) =>
            {
                let literal = syn::parse2::<syn::LitStr>(item_macro.mac.tokens.clone()).ok();
                if let Some(literal) = literal {
                    let path = containing_source
                        .parent()
                        .unwrap_or(workspace)
                        .join(literal.value());
                    collect_module_source(
                        workspace,
                        &path,
                        module.to_vec(),
                        module_dir.to_path_buf(),
                        units,
                        seen,
                        violations,
                    )?;
                } else {
                    violations.push(Violation {
                        path: workspace_relative(workspace, containing_source),
                        reason: "generated Rust include is not statically auditable; use a literal include! path or keep generated code outside production authority boundaries".into(),
                    });
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn module_source_path(
    containing_source: &Path,
    module_dir: &Path,
    item_mod: &ItemMod,
) -> Option<PathBuf> {
    for attribute in &item_mod.attrs {
        if !attribute.path().is_ident("path") {
            continue;
        }
        if let Meta::NameValue(value) = &attribute.meta {
            if let syn::Expr::Lit(expression) = &value.value {
                if let Lit::Str(path) = &expression.lit {
                    return Some(
                        containing_source
                            .parent()
                            .unwrap_or(module_dir)
                            .join(path.value()),
                    );
                }
            }
        }
    }
    let name = item_mod.ident.to_string();
    let flat = module_dir.join(format!("{name}.rs"));
    if flat.is_file() {
        return Some(flat);
    }
    let nested = module_dir.join(name).join("mod.rs");
    nested.is_file().then_some(nested)
}

fn workspace_relative(workspace: &Path, path: &Path) -> String {
    let normalized_workspace = fs::canonicalize(workspace).unwrap_or_else(|_| workspace.to_owned());
    let normalized_path = fs::canonicalize(path).unwrap_or_else(|_| path.to_owned());
    if let Ok(relative) = normalized_path.strip_prefix(&normalized_workspace) {
        return slash_path(relative);
    }
    if let Some(parent) = normalized_workspace.parent() {
        if let Ok(relative) = normalized_path.strip_prefix(parent) {
            return format!("../{}", slash_path(relative));
        }
    }
    slash_path(&normalized_path)
}

pub fn analyze_workspace(workspace: &Path) -> io::Result<Vec<Violation>> {
    let metadata = cargo_metadata(workspace)?;
    let mut violations = dependency_violations(&metadata, workspace);
    let mut units = Vec::new();
    let mut seen = BTreeSet::new();

    let members = metadata
        .get("workspace_members")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<BTreeSet<_>>();
    for package in metadata
        .get("packages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(package_id) = package.get("id").and_then(Value::as_str) else {
            continue;
        };
        if !members.contains(package_id) {
            continue;
        }
        let crate_name = package
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("fixture")
            .replace('-', "_");
        for target in package
            .get("targets")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if !is_production_target(target) {
                continue;
            }
            let Some(source) = target.get("src_path").and_then(Value::as_str) else {
                continue;
            };
            let source = PathBuf::from(source);
            let mut module = vec![crate_name.clone()];
            let target_name = target
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .replace('-', "_");
            let kinds = target
                .get("kind")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>();
            if !kinds
                .iter()
                .any(|kind| matches!(*kind, "lib" | "proc-macro"))
            {
                module.push(target_name);
            }
            collect_module_source(
                workspace,
                &source,
                module,
                source.parent().unwrap_or(workspace).to_path_buf(),
                &mut units,
                &mut seen,
                &mut violations,
            )?;
        }
    }

    violations.extend(analyze_source_units(&units));
    violations.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.reason.cmp(&right.reason))
    });
    violations.dedup();
    Ok(violations)
}

pub fn analyze_sources<'a>(
    sources: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> Vec<Violation> {
    let units = sources
        .into_iter()
        .map(|(path, source)| SourceUnit {
            path: path.to_owned(),
            source: source.to_owned(),
            module: module_for_file(path),
        })
        .collect::<Vec<_>>();
    analyze_source_units(&units)
}

#[derive(Clone, Debug)]
struct SourceUnit {
    path: String,
    source: String,
    module: Vec<String>,
}

fn analyze_source_units(units: &[SourceUnit]) -> Vec<Violation> {
    let mut violations = Vec::new();
    let mut summaries = Vec::new();
    let mut imports = BTreeMap::new();
    let mut aliases = BTreeMap::new();

    for unit in units {
        let parsed = match syn::parse_file(&unit.source) {
            Ok(parsed) => parsed,
            Err(error) => {
                violations.push(Violation {
                    path: unit.path.clone(),
                    reason: format!("production Rust source failed AST parsing: {error}"),
                });
                continue;
            }
        };
        analyze_items(
            &unit.path,
            &parsed.items,
            &unit.module,
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
    let visible_model_text = summaries
        .iter()
        .enumerate()
        .filter(|(_, summary)| summary.visible_model_text)
        .map(|(index, _)| index)
        .collect::<BTreeSet<_>>();
    let seeds = direct_shadow
        .union(&model_reparse)
        .copied()
        .chain(retired.iter().copied())
        .chain(visible_model_text.iter().copied())
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
        if visible_model_text.contains(&index) {
            violations.push(Violation {
                path: summary.path.clone(),
                reason: format!(
                    "`{}` exposes a visible model-to-source capability",
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
                !matches!(item_fn.vis, syn::Visibility::Inherited),
                authoritative,
                summaries,
            ),
            Item::Impl(item_impl) if !is_test_only(&item_impl.attrs) => {
                summarize_impl(path, module, item_impl, authoritative, summaries)
            }
            Item::Trait(item_trait) if !is_test_only(&item_trait.attrs) && !authoritative => {
                for trait_item in &item_trait.items {
                    if let syn::TraitItem::Fn(method) = trait_item {
                        if signature_is_raw_grammar_entry(&method.sig) {
                            violations.push(Violation {
                                path: path.to_owned(),
                                reason: format!(
                                    "`{}::{}::{}` declares raw-text grammar outside the syntax authority",
                                    module.join("::"),
                                    item_trait.ident,
                                    method.sig.ident
                                ),
                            });
                        }
                        if !matches!(item_trait.vis, syn::Visibility::Inherited)
                            && signature_is_model_to_text(&method.sig)
                        {
                            violations.push(Violation {
                                path: path.to_owned(),
                                reason: format!(
                                    "`{}::{}::{}` exposes a visible model-to-source capability",
                                    module.join("::"),
                                    item_trait.ident,
                                    method.sig.ident
                                ),
                            });
                        }
                    }
                }
            }
            Item::Macro(item_macro) if !authoritative && !is_test_only(&item_macro.attrs) => {
                inspect_opaque_macro(path, item_macro, violations);
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
                !matches!(method.vis, syn::Visibility::Inherited),
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
    visible: bool,
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
    facts.local.serialize |=
        signature_is_model_to_text(signature) || (model_input && facts.saw_method_call);
    let output_paths = match &signature.output {
        ReturnType::Default => Vec::new(),
        ReturnType::Type(_, output) => type_paths(output),
    };
    let visible_model_text = visible && signature_is_model_to_text(signature);
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
        visible_model_text,
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

fn signature_is_raw_grammar_entry(signature: &Signature) -> bool {
    let raw_input = signature.inputs.iter().any(|argument| match argument {
        FnArg::Receiver(_) => false,
        FnArg::Typed(argument) => type_contains_any(&argument.ty, &["str", "String"]),
    });
    let grammar_output = match &signature.output {
        ReturnType::Default => false,
        ReturnType::Type(_, output) => type_paths(output).iter().any(|path| {
            path.last()
                .is_some_and(|name| HIGH_LEVEL_GRAMMAR_TYPES.contains(&name.as_str()))
        }),
    };
    raw_input && grammar_output
}

fn signature_is_model_to_text(signature: &Signature) -> bool {
    let model_input = signature.inputs.iter().any(|argument| match argument {
        FnArg::Receiver(_) => false,
        FnArg::Typed(argument) => type_contains_any(&argument.ty, MODEL_TYPES),
    });
    let text_output = match &signature.output {
        ReturnType::Default => false,
        ReturnType::Type(_, output) => type_is_text_result(output),
    };
    model_input && text_output
}

fn type_is_text_result(ty: &Type) -> bool {
    match ty {
        Type::Group(group) => type_is_text_result(&group.elem),
        Type::Paren(paren) => type_is_text_result(&paren.elem),
        Type::Reference(reference) => type_is_text_result(&reference.elem),
        Type::Path(path) => {
            let Some(last) = path.path.segments.last() else {
                return false;
            };
            let name = last.ident.to_string();
            if matches!(name.as_str(), "str" | "String") {
                return true;
            }
            if !matches!(name.as_str(), "Arc" | "Box" | "Cow" | "Option" | "Result") {
                return false;
            }
            let syn::PathArguments::AngleBracketed(arguments) = &last.arguments else {
                return false;
            };
            arguments.args.iter().any(|argument| {
                matches!(argument, syn::GenericArgument::Type(inner) if type_is_text_result(inner))
            })
        }
        _ => false,
    }
}

fn is_reparse_path(segments: &[String]) -> bool {
    segments.last().is_some_and(|name| name == "parse_document")
        || segments.ends_with(&[
            "uml".to_owned(),
            "syntax".to_owned(),
            "parser".to_owned(),
            "parse".to_owned(),
        ])
        || segments.ends_with(&["syntax".to_owned(), "parser".to_owned(), "parse".to_owned()])
        || segments.ends_with(&["analysis".to_owned(), "prepare_candidate".to_owned()])
        || segments.ends_with(&["okf".to_owned(), "Bundle".to_owned(), "parse".to_owned()])
}

fn inspect_opaque_macro(path: &str, item_macro: &ItemMacro, violations: &mut Vec<Violation>) {
    if item_macro.mac.path.is_ident("include") {
        return;
    }
    let tokens = item_macro.mac.tokens.to_string();
    let grammar_type = HIGH_LEVEL_GRAMMAR_TYPES.iter().find(|name| {
        tokens
            .split(|character: char| !character.is_alphanumeric() && character != '_')
            .any(|token| token == **name)
    });
    if let Some(grammar_type) = grammar_type {
        violations.push(Violation {
            path: path.to_owned(),
            reason: format!(
                "opaque macro source references grammar type `{grammar_type}` outside the syntax authority"
            ),
        });
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
        if HIGH_LEVEL_GRAMMAR_TYPES.contains(&name.as_str()) {
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

fn slash_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
