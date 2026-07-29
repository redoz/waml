//! Task 21's architecture test is structural first:
//! Cargo metadata selects workspace packages and production targets, Rust module
//! declarations select source files, and crate/module visibility seals the raw
//! parser. The AST pass is deliberately residual. It resolves aliases, target
//! crate roots, callable edges, and model-text taint to reject raw-to-typed-tree
//! authority, visible model-to-text surfaces, and model-to-authority-reparse
//! reachability; it does not pretend to be rustc type inference or general macro
//! expansion. Standalone literal `include!` files are followed. Dynamic or
//! context-dependent includes and non-allowlisted procedural macros in protected
//! code are rejected because their expanded call graph cannot be audited here.

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
    "ActivityNode",
    "AssocName",
    "Attribute",
    "BehaviorKind",
    "CardinalityVisibility",
    "ClassDiagram",
    "DeclaredAttribute",
    "DeclaredBundle",
    "DeclaredConcept",
    "DeclaredField",
    "DeclaredFlowNode",
    "DeclaredFlowTransition",
    "DeclaredInlineInstance",
    "DeclaredLayoutStatement",
    "DeclaredLifeline",
    "DeclaredMember",
    "DeclaredMemberGroup",
    "DeclaredMessage",
    "DeclaredRelationship",
    "DeclaredSequenceOperand",
    "DeclaredSlot",
    "DeclaredValue",
    "Diagram",
    "DiagramDisplay",
    "DiagramGroup",
    "DocumentDto",
    "Edge",
    "ElementType",
    "FlowDoc",
    "FlowEdge",
    "FlowEdgeKind",
    "FlowFlavor",
    "FlowNodeKind",
    "FragmentKind",
    "MessageVerb",
    "Model",
    "Node",
    "NoteAnchor",
    "Projection",
    "RelEnd",
    "RelationshipKind",
    "SeqChild",
    "SeqEdge",
    "SeqNode",
    "SequenceDoc",
    "Slot",
    "TypeRef",
    "UmlModel",
    "UmlMetaclass",
    "Visibility",
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
struct ModuleIdentity {
    crate_key: String,
    segments: Vec<String>,
    crate_root_len: usize,
}

impl ModuleIdentity {
    fn new(crate_key: String, segments: Vec<String>) -> Self {
        let crate_root_len = segments.len();
        Self {
            crate_key,
            segments,
            crate_root_len,
        }
    }

    fn child(&self, name: impl Into<String>) -> Self {
        let mut nested = self.clone();
        nested.segments.push(name.into());
        nested
    }

    fn reset_to_crate_root(&mut self) {
        self.segments.truncate(self.crate_root_len);
    }

    fn pop_module(&mut self) {
        if self.segments.len() > self.crate_root_len {
            self.segments.pop();
        }
    }

    fn extend(&mut self, segments: impl IntoIterator<Item = String>) {
        self.segments.extend(segments);
    }

    fn display(&self) -> String {
        self.segments.join("::")
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct TypeIdentity {
    module: ModuleIdentity,
    name: String,
}

impl TypeIdentity {
    fn new(module: ModuleIdentity, name: impl Into<String>) -> Self {
        Self {
            module,
            name: name.into(),
        }
    }
}

type Imports = BTreeMap<ModuleIdentity, BTreeMap<String, Vec<String>>>;
type Aliases = BTreeMap<(ModuleIdentity, String), Vec<Vec<String>>>;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct FunctionId {
    module: ModuleIdentity,
    owner: Option<String>,
    name: String,
}

impl FunctionId {
    fn display(&self) -> String {
        let mut path = self.module.display();
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
    receiver_type: Option<TypeIdentity>,
    indirect: bool,
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

#[derive(Clone)]
struct FunctionSummary {
    path: String,
    id: FunctionId,
    block: syn::Block,
    calls: Vec<CallSite>,
    local: Capabilities,
    input_types: Vec<ParameterSummary>,
    owner_paths: Vec<Vec<String>>,
    owner_type: Option<TypeIdentity>,
    has_receiver: bool,
    trait_type: Option<TypeIdentity>,
    allowlisted_standard_text_trait: bool,
    model_input: bool,
    output_type: Option<Type>,
    output_paths: Vec<Vec<String>>,
    body_paths: Vec<Vec<String>>,
    body_type_uses: Vec<TypeUse>,
    local_standard_roots: BTreeSet<String>,
    retired_calls: BTreeSet<String>,
    opaque_authority_macros: BTreeSet<String>,
    raw_authority_closure: bool,
    allowlisted_leaf_codec: bool,
    visible: bool,
}

#[derive(Clone, Debug)]
struct ParameterSummary {
    name: Option<String>,
    paths: Vec<Vec<String>>,
    mutable: bool,
}

#[derive(Clone, Debug)]
struct TypeUse {
    module: ModuleIdentity,
    paths: Vec<Vec<String>>,
}

type StructFields = BTreeMap<TypeIdentity, BTreeMap<String, TypeUse>>;
type MethodReturns = BTreeMap<(TypeIdentity, String), TypeUse>;
type TraitVisibility = BTreeMap<TypeIdentity, bool>;
type TraitDefaults = BTreeMap<TypeIdentity, Vec<(Signature, syn::Block)>>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ValueOrigin {
    Other,
    Model,
    ModelText,
    ProtectedSource,
}

struct BodyEnvironment<'env> {
    module: &'env ModuleIdentity,
    aliases: &'env Aliases,
    imports: &'env Imports,
    struct_fields: &'env StructFields,
    method_returns: &'env MethodReturns,
}

#[derive(Clone)]
struct BindingState {
    receiver_type: Option<TypeIdentity>,
    receiver_type_uses: Option<Vec<TypeUse>>,
    callable_path: Option<Vec<String>>,
    bound: bool,
    origin: Option<ValueOrigin>,
}

struct BodyFacts<'env> {
    env: BodyEnvironment<'env>,
    calls: Vec<CallSite>,
    local: Capabilities,
    retired_calls: BTreeSet<String>,
    receiver_types: BTreeMap<String, TypeIdentity>,
    receiver_type_uses: BTreeMap<String, Vec<TypeUse>>,
    callable_paths: BTreeMap<String, Vec<String>>,
    bindings: BTreeSet<String>,
    origins: BTreeMap<String, ValueOrigin>,
    binding_scopes: Vec<BTreeMap<String, BindingState>>,
    body_paths: Vec<Vec<String>>,
    body_type_uses: Vec<TypeUse>,
    opaque_authority_macros: BTreeSet<String>,
    raw_authority_closure: bool,
}

impl<'ast, 'env> Visit<'ast> for BodyFacts<'env> {
    fn visit_block(&mut self, block: &'ast syn::Block) {
        self.push_binding_scope();
        visit::visit_block(self, block);
        self.pop_binding_scope();
    }

    fn visit_expr_call(&mut self, node: &'ast ExprCall) {
        let (mut segments, mut indirect) = match unwrap_expression(&node.func) {
            syn::Expr::Path(ExprPath { path, .. }) => {
                self.body_paths.extend(paths_in_path(path));
                let segments = path
                    .segments
                    .iter()
                    .map(|segment| segment.ident.to_string())
                    .collect::<Vec<_>>();
                let indirect = segments.len() == 1 && self.bindings.contains(&segments[0]);
                (segments, indirect)
            }
            syn::Expr::Field(field) => (
                vec![match &field.member {
                    syn::Member::Named(name) => name.to_string(),
                    syn::Member::Unnamed(index) => index.index.to_string(),
                }],
                true,
            ),
            _ => (Vec::new(), true),
        };
        if segments.len() == 1 {
            if let Some(target) = self.callable_paths.get(&segments[0]) {
                segments = target.clone();
                indirect = true;
            }
        }
        if !segments.is_empty() || indirect {
            self.record_capability(&segments);
            self.calls.push(CallSite {
                segments,
                method: false,
                receiver_type: None,
                indirect,
            });
        }
        visit::visit_expr_call(self, node);
    }

    fn visit_expr_struct(&mut self, expression: &'ast syn::ExprStruct) {
        self.body_paths.extend(paths_in_path(&expression.path));
        visit::visit_expr_struct(self, expression);
    }

    fn visit_expr_method_call(&mut self, node: &'ast ExprMethodCall) {
        let name = node.method.to_string();
        let receiver_type = self.expression_type_identity(&node.receiver);
        let receiver_origin = self.expression_origin(&node.receiver);
        self.local.serialize |=
            receiver_origin == ValueOrigin::Model && is_model_text_method(&name);
        self.calls.push(CallSite {
            segments: vec![name],
            method: true,
            receiver_type,
            indirect: true,
        });
        visit::visit_expr_method_call(self, node);
    }

    fn visit_local(&mut self, node: &'ast Local) {
        let bindings = pattern_binding_names(&node.pat);
        let explicit_paths = match &node.pat {
            syn::Pat::Type(pattern) => type_paths(&pattern.ty),
            _ => Vec::new(),
        };
        self.body_paths.extend(explicit_paths.iter().cloned());

        // A Rust `let` binding is not in scope in its initializer or in the
        // diverging branch of `let-else`; traverse both before deriving the
        // initializer's evidence or shadowing any previous binding.
        visit::visit_local(self, node);

        let explicit_type_uses = if explicit_paths.is_empty() {
            Vec::new()
        } else {
            vec![TypeUse {
                module: self.env.module.clone(),
                paths: explicit_paths.clone(),
            }]
        };
        let inferred_type_uses = node
            .init
            .as_ref()
            .map(|init| self.expression_type_uses(&init.expr))
            .unwrap_or_default();
        let explicit = receiver_type_identity(
            self.env.module,
            &explicit_paths,
            self.env.aliases,
            self.env.imports,
        );
        let inferred = node
            .init
            .as_ref()
            .and_then(|init| self.expression_type_identity(&init.expr));
        let receiver_type = explicit.or(inferred);
        let type_uses = if explicit_type_uses.is_empty() {
            inferred_type_uses
        } else {
            explicit_type_uses
        };
        let origin = (!bindings.is_empty()).then(|| {
            node.init
                .as_ref()
                .map(|init| self.expression_origin(&init.expr))
                .filter(|origin| *origin != ValueOrigin::Other)
                .unwrap_or_else(|| {
                    origin_for_type(
                        self.env.module,
                        &explicit_paths,
                        self.env.aliases,
                        self.env.imports,
                    )
                })
        });
        let callable_path = if let ([binding], Some(init)) = (bindings.as_slice(), &node.init) {
            if let syn::Expr::Path(path) = &*init.expr {
                Some((
                    binding.clone(),
                    path.path
                        .segments
                        .iter()
                        .map(|segment| segment.ident.to_string())
                        .collect(),
                ))
            } else {
                None
            }
        } else {
            None
        };

        for binding in &bindings {
            self.record_binding_declaration(binding);
            self.receiver_types.remove(binding);
            self.receiver_type_uses.remove(binding);
            self.callable_paths.remove(binding);
            self.origins.remove(binding);
            self.bindings.insert(binding.clone());
        }
        if let Some(ty) = receiver_type {
            for binding in &bindings {
                self.receiver_types.insert(binding.clone(), ty.clone());
            }
        }
        if !type_uses.is_empty() {
            for binding in &bindings {
                self.receiver_type_uses
                    .insert(binding.clone(), type_uses.clone());
            }
        }
        if let Some(origin) = origin {
            self.local.serialize |= origin == ValueOrigin::ModelText;
            for binding in &bindings {
                self.origins.insert(binding.clone(), origin);
            }
        }
        if let Some((binding, path)) = callable_path {
            self.callable_paths.insert(binding, path);
        }
    }

    fn visit_expr_cast(&mut self, expression: &'ast syn::ExprCast) {
        self.body_paths.extend(type_paths(&expression.ty));
        visit::visit_expr_cast(self, expression);
    }

    fn visit_expr_assign(&mut self, assignment: &'ast syn::ExprAssign) {
        self.record_assignment_place_type(&assignment.left);
        visit::visit_expr_assign(self, assignment);

        let assigned_bindings = assignment_local_binding_names(&assignment.left, &self.bindings);
        let type_uses = self.expression_type_uses(&assignment.right);
        let identity = self.expression_type_identity(&assignment.right);
        let origin = self.expression_origin(&assignment.right);
        let callable_path = if assigned_bindings.len() == 1 {
            match unwrap_expression(&assignment.right) {
                syn::Expr::Path(path) => Some(
                    path.path
                        .segments
                        .iter()
                        .map(|segment| segment.ident.to_string())
                        .collect::<Vec<_>>(),
                ),
                _ => None,
            }
        } else {
            None
        };
        for binding in assigned_bindings {
            if !type_uses.is_empty() {
                self.receiver_type_uses
                    .entry(binding.clone())
                    .or_default()
                    .extend(type_uses.iter().cloned());
            }
            if let Some(identity) = &identity {
                self.receiver_types
                    .insert(binding.clone(), identity.clone());
            } else {
                self.receiver_types.remove(&binding);
            }
            if let Some(callable_path) = &callable_path {
                self.callable_paths
                    .insert(binding.clone(), callable_path.clone());
            } else {
                self.callable_paths.remove(&binding);
            }
            self.origins.insert(binding, origin);
        }
    }

    fn visit_expr_for_loop(&mut self, expression: &'ast syn::ExprForLoop) {
        for attribute in &expression.attrs {
            self.visit_attribute(attribute);
        }
        self.visit_expr(&expression.expr);
        self.push_binding_scope();
        self.bind_pattern_from_expression(&expression.pat, &expression.expr);
        self.visit_block(&expression.body);
        self.pop_binding_scope();
    }

    fn visit_expr_match(&mut self, expression: &'ast syn::ExprMatch) {
        for attribute in &expression.attrs {
            self.visit_attribute(attribute);
        }
        self.visit_expr(&expression.expr);
        for arm in &expression.arms {
            self.push_binding_scope();
            self.bind_pattern_from_expression(&arm.pat, &expression.expr);
            if let Some((_, guard)) = &arm.guard {
                self.visit_expr(guard);
            }
            self.visit_expr(&arm.body);
            self.pop_binding_scope();
        }
    }

    fn visit_expr_if(&mut self, expression: &'ast syn::ExprIf) {
        let syn::Expr::Let(condition) = unwrap_expression(&expression.cond) else {
            visit::visit_expr_if(self, expression);
            return;
        };
        for attribute in &expression.attrs {
            self.visit_attribute(attribute);
        }
        self.visit_expr(&condition.expr);
        self.push_binding_scope();
        self.bind_pattern_from_expression(&condition.pat, &condition.expr);
        self.visit_block(&expression.then_branch);
        self.pop_binding_scope();
        if let Some((_, alternative)) = &expression.else_branch {
            self.visit_expr(alternative);
        }
    }

    fn visit_expr_while(&mut self, expression: &'ast syn::ExprWhile) {
        let syn::Expr::Let(condition) = unwrap_expression(&expression.cond) else {
            visit::visit_expr_while(self, expression);
            return;
        };
        for attribute in &expression.attrs {
            self.visit_attribute(attribute);
        }
        self.visit_expr(&condition.expr);
        self.push_binding_scope();
        self.bind_pattern_from_expression(&condition.pat, &condition.expr);
        self.visit_block(&expression.body);
        self.pop_binding_scope();
    }

    fn visit_macro(&mut self, item_macro: &'ast syn::Macro) {
        let name = macro_path(item_macro);
        if !is_harmless_body_macro(&name) {
            let identifiers = macro_identifiers(item_macro);
            let protected_binding = identifiers.iter().any(|identifier| {
                self.origins.get(identifier).is_some_and(|origin| {
                    matches!(
                        origin,
                        ValueOrigin::ModelText | ValueOrigin::ProtectedSource
                    )
                })
            });
            let protected_shape = identifiers.contains("SourceText")
                && identifiers.contains("SyntaxTree")
                && identifiers.contains("UmlLanguage");
            if protected_binding || protected_shape {
                self.opaque_authority_macros.insert(name);
            }
        }
        visit::visit_macro(self, item_macro);
    }

    fn visit_expr_closure(&mut self, closure: &'ast syn::ExprClosure) {
        self.raw_authority_closure |= closure_is_raw_grammar_entry(
            closure,
            self.env.module,
            self.env.aliases,
            self.env.imports,
        );
        self.push_binding_scope();
        for input in &closure.inputs {
            let bindings = pattern_binding_names(input);
            let paths = match input {
                syn::Pat::Type(pattern) => type_paths(&pattern.ty),
                _ => Vec::new(),
            };
            let type_uses = (!paths.is_empty()).then(|| {
                vec![TypeUse {
                    module: self.env.module.clone(),
                    paths: paths.clone(),
                }]
            });
            let identity =
                receiver_type_identity(self.env.module, &paths, self.env.aliases, self.env.imports);
            let origin =
                origin_for_type(self.env.module, &paths, self.env.aliases, self.env.imports);
            for binding in bindings {
                self.record_binding_declaration(&binding);
                self.bindings.insert(binding.clone());
                self.callable_paths.remove(&binding);
                if let Some(identity) = &identity {
                    self.receiver_types
                        .insert(binding.clone(), identity.clone());
                } else {
                    self.receiver_types.remove(&binding);
                }
                if let Some(type_uses) = &type_uses {
                    self.receiver_type_uses
                        .insert(binding.clone(), type_uses.clone());
                } else {
                    self.receiver_type_uses.remove(&binding);
                }
                self.origins.insert(binding, origin);
            }
        }
        visit::visit_expr_closure(self, closure);
        self.pop_binding_scope();
    }
}

fn pattern_binding_names(pattern: &syn::Pat) -> Vec<String> {
    #[derive(Default)]
    struct PatternBindings {
        names: BTreeSet<String>,
    }

    impl<'ast> Visit<'ast> for PatternBindings {
        fn visit_pat_ident(&mut self, pattern: &'ast syn::PatIdent) {
            self.names.insert(pattern.ident.to_string());
            visit::visit_pat_ident(self, pattern);
        }
    }

    let mut bindings = PatternBindings::default();
    bindings.visit_pat(pattern);
    bindings.names.into_iter().collect()
}

fn assignment_local_binding_names(
    expression: &syn::Expr,
    known_bindings: &BTreeSet<String>,
) -> Vec<String> {
    fn collect(
        expression: &syn::Expr,
        known_bindings: &BTreeSet<String>,
        names: &mut BTreeSet<String>,
    ) {
        match unwrap_expression(expression) {
            syn::Expr::Path(path) if path.path.segments.len() == 1 => {
                let name = path.path.segments[0].ident.to_string();
                if known_bindings.contains(&name) {
                    names.insert(name);
                }
            }
            syn::Expr::Tuple(tuple) => {
                for element in &tuple.elems {
                    collect(element, known_bindings, names);
                }
            }
            syn::Expr::Array(array) => {
                for element in &array.elems {
                    collect(element, known_bindings, names);
                }
            }
            syn::Expr::Struct(structure) => {
                for field in &structure.fields {
                    collect(&field.expr, known_bindings, names);
                }
            }
            _ => {}
        }
    }

    let mut names = BTreeSet::new();
    collect(expression, known_bindings, &mut names);
    names.into_iter().collect()
}

impl<'env> BodyFacts<'env> {
    fn new(env: BodyEnvironment<'env>) -> Self {
        Self {
            env,
            calls: Vec::new(),
            local: Capabilities::default(),
            retired_calls: BTreeSet::new(),
            receiver_types: BTreeMap::new(),
            receiver_type_uses: BTreeMap::new(),
            callable_paths: BTreeMap::new(),
            bindings: BTreeSet::new(),
            origins: BTreeMap::new(),
            binding_scopes: Vec::new(),
            body_paths: Vec::new(),
            body_type_uses: Vec::new(),
            opaque_authority_macros: BTreeSet::new(),
            raw_authority_closure: false,
        }
    }

    fn push_binding_scope(&mut self) {
        self.binding_scopes.push(BTreeMap::new());
    }

    fn record_binding_declaration(&mut self, binding: &str) {
        let state = BindingState {
            receiver_type: self.receiver_types.get(binding).cloned(),
            receiver_type_uses: self.receiver_type_uses.get(binding).cloned(),
            callable_path: self.callable_paths.get(binding).cloned(),
            bound: self.bindings.contains(binding),
            origin: self.origins.get(binding).copied(),
        };
        if let Some(scope) = self.binding_scopes.last_mut() {
            scope.entry(binding.to_owned()).or_insert(state);
        }
    }

    fn pop_binding_scope(&mut self) {
        let scope = self
            .binding_scopes
            .pop()
            .expect("binding scope is balanced");
        for (binding, state) in scope {
            match state.receiver_type {
                Some(value) => {
                    self.receiver_types.insert(binding.clone(), value);
                }
                None => {
                    self.receiver_types.remove(&binding);
                }
            }
            match state.receiver_type_uses {
                Some(value) => {
                    self.receiver_type_uses.insert(binding.clone(), value);
                }
                None => {
                    self.receiver_type_uses.remove(&binding);
                }
            }
            match state.callable_path {
                Some(value) => {
                    self.callable_paths.insert(binding.clone(), value);
                }
                None => {
                    self.callable_paths.remove(&binding);
                }
            }
            if state.bound {
                self.bindings.insert(binding.clone());
            } else {
                self.bindings.remove(&binding);
            }
            match state.origin {
                Some(value) => {
                    self.origins.insert(binding, value);
                }
                None => {
                    self.origins.remove(&binding);
                }
            }
        }
    }

    fn bind_pattern_from_expression(&mut self, pattern: &syn::Pat, expression: &syn::Expr) {
        let bindings = pattern_binding_names(pattern);
        let type_uses = self.expression_type_uses(expression);
        let identity = self.expression_type_identity(expression);
        let origin = self.expression_origin(expression);
        for binding in bindings {
            self.record_binding_declaration(&binding);
            self.bindings.insert(binding.clone());
            self.callable_paths.remove(&binding);
            if let Some(identity) = &identity {
                self.receiver_types
                    .insert(binding.clone(), identity.clone());
            } else {
                self.receiver_types.remove(&binding);
            }
            if type_uses.is_empty() {
                self.receiver_type_uses.remove(&binding);
            } else {
                self.receiver_type_uses
                    .insert(binding.clone(), type_uses.clone());
            }
            self.origins.insert(binding, origin);
        }
    }

    fn record_capability(&mut self, segments: &[String]) {
        self.local.reparse |= is_reparse_path(segments);
        if let Some(name) = segments.last() {
            if RETIRED_CALLS.contains(&name.as_str()) {
                self.retired_calls.insert(name.to_owned());
            }
        }
    }

    fn record_assignment_place_type(&mut self, expression: &syn::Expr) {
        self.record_assignment_place_type_inner(expression, false);
    }

    fn record_assignment_place_type_inner(&mut self, expression: &syn::Expr, through_alias: bool) {
        match unwrap_expression(expression) {
            syn::Expr::Path(_) if through_alias => {
                self.body_type_uses
                    .extend(self.expression_type_uses(expression));
            }
            syn::Expr::Field(field) => {
                let type_uses = self.expression_type_uses(expression);
                if type_uses.is_empty() {
                    self.record_assignment_place_type_inner(&field.base, through_alias);
                } else {
                    self.body_type_uses.extend(type_uses);
                }
            }
            syn::Expr::Index(index) => {
                let type_uses = self.expression_type_uses(expression);
                if type_uses.is_empty() {
                    self.record_assignment_place_type_inner(&index.expr, through_alias);
                } else {
                    self.body_type_uses.extend(type_uses);
                }
            }
            syn::Expr::Unary(unary) if matches!(unary.op, syn::UnOp::Deref(_)) => {
                self.record_assignment_place_type_inner(&unary.expr, true);
            }
            _ if through_alias => {
                self.body_type_uses
                    .extend(self.expression_type_uses(expression));
            }
            _ => {}
        }
    }

    fn expression_type_uses(&self, expression: &syn::Expr) -> Vec<TypeUse> {
        match unwrap_expression(expression) {
            syn::Expr::Path(expression) => {
                if expression.path.segments.len() == 1 {
                    let name = expression.path.segments[0].ident.to_string();
                    if let Some(types) = self.receiver_type_uses.get(&name) {
                        return types.clone();
                    }
                }
                vec![TypeUse {
                    module: self.env.module.clone(),
                    paths: paths_in_path(&expression.path),
                }]
            }
            syn::Expr::Field(expression) => {
                let field = match &expression.member {
                    syn::Member::Named(name) => name.to_string(),
                    syn::Member::Unnamed(index) => index.index.to_string(),
                };
                self.expression_type_identity(&expression.base)
                    .and_then(|owner| {
                        self.env
                            .struct_fields
                            .get(&owner)
                            .and_then(|fields| fields.get(&field))
                            .cloned()
                    })
                    .map(|ty| vec![ty])
                    .unwrap_or_else(|| self.expression_type_uses(&expression.base))
            }
            syn::Expr::MethodCall(expression) => self
                .expression_type_identity(&expression.receiver)
                .and_then(|owner| {
                    self.env
                        .method_returns
                        .get(&(owner, expression.method.to_string()))
                        .cloned()
                })
                .map(|ty| vec![ty])
                .unwrap_or_else(|| {
                    let mut types = self.expression_type_uses(&expression.receiver);
                    for argument in &expression.args {
                        types.extend(self.expression_type_uses(argument));
                    }
                    types
                }),
            syn::Expr::Struct(expression) => {
                let mut types = vec![TypeUse {
                    module: self.env.module.clone(),
                    paths: paths_in_path(&expression.path),
                }];
                for field in &expression.fields {
                    types.extend(self.expression_type_uses(&field.expr));
                }
                if let Some(rest) = &expression.rest {
                    types.extend(self.expression_type_uses(rest));
                }
                types
            }
            syn::Expr::Call(expression) => {
                let mut types = match unwrap_expression(&expression.func) {
                    syn::Expr::Path(function) => {
                        let mut segments = function
                            .path
                            .segments
                            .iter()
                            .map(|segment| segment.ident.to_string())
                            .collect::<Vec<_>>();
                        if segments.len() >= 2
                            && segments.last().is_some_and(|method| {
                                method.chars().next().is_some_and(char::is_lowercase)
                            })
                        {
                            segments.pop();
                        }
                        vec![TypeUse {
                            module: self.env.module.clone(),
                            paths: vec![segments],
                        }]
                    }
                    _ => self.expression_type_uses(&expression.func),
                };
                for argument in &expression.args {
                    types.extend(self.expression_type_uses(argument));
                }
                types
            }
            syn::Expr::Tuple(expression) => expression
                .elems
                .iter()
                .flat_map(|element| self.expression_type_uses(element))
                .collect(),
            syn::Expr::Array(expression) => expression
                .elems
                .iter()
                .flat_map(|element| self.expression_type_uses(element))
                .collect(),
            syn::Expr::Repeat(expression) => self.expression_type_uses(&expression.expr),
            syn::Expr::Index(expression) => self.expression_type_uses(&expression.expr),
            syn::Expr::Unary(expression) if matches!(expression.op, syn::UnOp::Deref(_)) => {
                self.expression_type_uses(&expression.expr)
            }
            syn::Expr::Block(expression) => self.block_type_uses(&expression.block),
            syn::Expr::If(expression) => {
                let mut branches = self.block_type_uses(&expression.then_branch);
                if let Some((_, alternative)) = &expression.else_branch {
                    branches.extend(self.expression_type_uses(alternative));
                }
                branches
            }
            syn::Expr::Match(expression) => expression
                .arms
                .iter()
                .flat_map(|arm| self.expression_type_uses(&arm.body))
                .collect(),
            syn::Expr::Try(expression) => self.expression_type_uses(&expression.expr),
            _ => self.descendant_expression_type_uses(expression),
        }
    }

    fn descendant_expression_type_uses(&self, expression: &syn::Expr) -> Vec<TypeUse> {
        struct DescendantTypeUses<'facts, 'env> {
            facts: &'facts BodyFacts<'env>,
            at_root: bool,
            types: Vec<TypeUse>,
        }

        impl<'ast, 'facts, 'env> Visit<'ast> for DescendantTypeUses<'facts, 'env> {
            fn visit_expr(&mut self, expression: &'ast syn::Expr) {
                if self.at_root {
                    self.at_root = false;
                    visit::visit_expr(self, expression);
                    return;
                }
                let types = self.facts.expression_type_uses(expression);
                if types.is_empty() {
                    visit::visit_expr(self, expression);
                } else {
                    self.types.extend(types);
                }
            }
        }

        let mut descendants = DescendantTypeUses {
            facts: self,
            at_root: true,
            types: Vec::new(),
        };
        descendants.visit_expr(expression);
        descendants.types
    }

    fn block_type_uses(&self, block: &syn::Block) -> Vec<TypeUse> {
        let Some(syn::Stmt::Expr(expression, None)) = block.stmts.last() else {
            return Vec::new();
        };
        self.expression_type_uses(expression)
    }

    fn expression_type_identity(&self, expression: &syn::Expr) -> Option<TypeIdentity> {
        match unwrap_expression(expression) {
            syn::Expr::Path(expression) => {
                if expression.path.segments.len() == 1 {
                    let name = expression.path.segments[0].ident.to_string();
                    if let Some(ty) = self.receiver_types.get(&name) {
                        return Some(ty.clone());
                    }
                }
                receiver_type_identity(
                    self.env.module,
                    &paths_in_path(&expression.path),
                    self.env.aliases,
                    self.env.imports,
                )
            }
            syn::Expr::Field(expression) => {
                let owner = self.expression_type_identity(&expression.base)?;
                let field = match &expression.member {
                    syn::Member::Named(name) => name.to_string(),
                    syn::Member::Unnamed(index) => index.index.to_string(),
                };
                let field_type = self.env.struct_fields.get(&owner)?.get(&field)?;
                receiver_type_identity(
                    &field_type.module,
                    &field_type.paths,
                    self.env.aliases,
                    self.env.imports,
                )
            }
            syn::Expr::MethodCall(expression) => {
                let owner = self.expression_type_identity(&expression.receiver)?;
                let output = self
                    .env
                    .method_returns
                    .get(&(owner, expression.method.to_string()))?;
                receiver_type_identity(
                    &output.module,
                    &output.paths,
                    self.env.aliases,
                    self.env.imports,
                )
            }
            syn::Expr::Struct(expression) => preferred_type_identity(
                self.env.module,
                &paths_in_path(&expression.path),
                self.env.aliases,
                self.env.imports,
            ),
            syn::Expr::Call(expression) => match unwrap_expression(&expression.func) {
                syn::Expr::Path(function) => {
                    let mut segments = function
                        .path
                        .segments
                        .iter()
                        .map(|segment| segment.ident.to_string())
                        .collect::<Vec<_>>();
                    if segments.len() >= 2
                        && segments.last().is_some_and(|method| {
                            method.chars().next().is_some_and(char::is_lowercase)
                        })
                    {
                        segments.pop();
                    }
                    receiver_type_identity(
                        self.env.module,
                        &[segments],
                        self.env.aliases,
                        self.env.imports,
                    )
                }
                _ => None,
            },
            syn::Expr::Unary(expression) if matches!(expression.op, syn::UnOp::Deref(_)) => {
                self.expression_type_identity(&expression.expr)
            }
            _ => None,
        }
    }

    fn expression_origin(&self, expression: &syn::Expr) -> ValueOrigin {
        match unwrap_expression(expression) {
            syn::Expr::Path(path) if path.path.segments.len() == 1 => self
                .origins
                .get(&path.path.segments[0].ident.to_string())
                .copied()
                .unwrap_or(ValueOrigin::Other),
            syn::Expr::Field(field) => match self.expression_origin(&field.base) {
                ValueOrigin::Model => ValueOrigin::Model,
                origin => origin,
            },
            syn::Expr::MethodCall(call) => {
                let receiver = self.expression_origin(&call.receiver);
                if (receiver == ValueOrigin::Model
                    && is_model_text_method(&call.method.to_string()))
                    || receiver == ValueOrigin::ModelText
                    || call
                        .args
                        .iter()
                        .any(|argument| self.expression_origin(argument) == ValueOrigin::ModelText)
                {
                    ValueOrigin::ModelText
                } else {
                    ValueOrigin::Other
                }
            }
            syn::Expr::Call(call) => {
                if call
                    .args
                    .iter()
                    .any(|argument| self.expression_origin(argument) == ValueOrigin::ModelText)
                {
                    ValueOrigin::ModelText
                } else {
                    ValueOrigin::Other
                }
            }
            syn::Expr::Macro(expression) => {
                let identifiers = macro_identifiers(&expression.mac);
                if identifiers.iter().any(|identifier| {
                    self.origins
                        .get(identifier)
                        .is_some_and(|origin| *origin == ValueOrigin::ModelText)
                }) {
                    ValueOrigin::ModelText
                } else {
                    ValueOrigin::Other
                }
            }
            syn::Expr::Binary(expression) => merge_origins([
                self.expression_origin(&expression.left),
                self.expression_origin(&expression.right),
            ]),
            syn::Expr::Array(expression) => merge_origins(
                expression
                    .elems
                    .iter()
                    .map(|item| self.expression_origin(item)),
            ),
            syn::Expr::Tuple(expression) => merge_origins(
                expression
                    .elems
                    .iter()
                    .map(|item| self.expression_origin(item)),
            ),
            syn::Expr::Struct(expression) => merge_origins(
                expression
                    .fields
                    .iter()
                    .map(|field| self.expression_origin(&field.expr)),
            ),
            syn::Expr::Await(expression) => self.expression_origin(&expression.base),
            syn::Expr::Index(expression) => self.expression_origin(&expression.expr),
            syn::Expr::Repeat(expression) => self.expression_origin(&expression.expr),
            syn::Expr::Try(expression) => self.expression_origin(&expression.expr),
            syn::Expr::Unary(expression) => self.expression_origin(&expression.expr),
            _ => ValueOrigin::Other,
        }
    }
}

fn unwrap_expression(mut expression: &syn::Expr) -> &syn::Expr {
    loop {
        expression = match expression {
            syn::Expr::Group(group) => &group.expr,
            syn::Expr::Paren(paren) => &paren.expr,
            syn::Expr::Reference(reference) => &reference.expr,
            _ => return expression,
        };
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum FlowOutcome {
    Fallthrough,
    Return,
    Break(Option<String>),
    Continue(Option<String>),
    Diverge,
}

fn fallthrough_flow() -> BTreeSet<FlowOutcome> {
    [FlowOutcome::Fallthrough].into_iter().collect()
}

fn sequence_flow(
    first: BTreeSet<FlowOutcome>,
    second: BTreeSet<FlowOutcome>,
) -> BTreeSet<FlowOutcome> {
    let mut outcomes = first
        .iter()
        .filter(|outcome| **outcome != FlowOutcome::Fallthrough)
        .cloned()
        .collect::<BTreeSet<_>>();
    if first.contains(&FlowOutcome::Fallthrough) {
        outcomes.extend(second);
    }
    outcomes
}

fn sequence_expression_flow<'a>(
    expressions: impl IntoIterator<Item = &'a syn::Expr>,
) -> BTreeSet<FlowOutcome> {
    expressions
        .into_iter()
        .fold(fallthrough_flow(), |flow, expression| {
            sequence_flow(flow, expression_flow(expression))
        })
}

fn block_flow(block: &syn::Block) -> BTreeSet<FlowOutcome> {
    block
        .stmts
        .iter()
        .fold(fallthrough_flow(), |flow, statement| {
            let statement_flow = match statement {
                syn::Stmt::Expr(expression, _) => expression_flow(expression),
                _ => fallthrough_flow(),
            };
            sequence_flow(flow, statement_flow)
        })
}

fn label_name(label: &Option<syn::Lifetime>) -> Option<String> {
    label.as_ref().map(|label| label.ident.to_string())
}

fn loop_flow(body: &syn::Block, label: Option<String>, can_skip: bool) -> BTreeSet<FlowOutcome> {
    let mut outcomes = can_skip
        .then(fallthrough_flow)
        .unwrap_or_else(BTreeSet::new);
    for outcome in block_flow(body) {
        match outcome {
            FlowOutcome::Break(target) if target.is_none() || target == label => {
                outcomes.insert(FlowOutcome::Fallthrough);
            }
            FlowOutcome::Continue(target) if target.is_none() || target == label => {
                outcomes.insert(if can_skip {
                    FlowOutcome::Fallthrough
                } else {
                    FlowOutcome::Diverge
                });
            }
            FlowOutcome::Fallthrough => {
                outcomes.insert(if can_skip {
                    FlowOutcome::Fallthrough
                } else {
                    FlowOutcome::Diverge
                });
            }
            outcome => {
                outcomes.insert(outcome);
            }
        }
    }
    outcomes
}

fn expression_flow(expression: &syn::Expr) -> BTreeSet<FlowOutcome> {
    match unwrap_expression(expression) {
        syn::Expr::Return(expression) => sequence_flow(
            expression
                .expr
                .as_deref()
                .map(expression_flow)
                .unwrap_or_else(fallthrough_flow),
            [FlowOutcome::Return].into_iter().collect(),
        ),
        syn::Expr::Break(expression) => sequence_flow(
            expression
                .expr
                .as_deref()
                .map(expression_flow)
                .unwrap_or_else(fallthrough_flow),
            [FlowOutcome::Break(label_name(&expression.label))]
                .into_iter()
                .collect(),
        ),
        syn::Expr::Continue(expression) => [FlowOutcome::Continue(label_name(&expression.label))]
            .into_iter()
            .collect(),
        syn::Expr::Block(expression) => {
            let flow = block_flow(&expression.block);
            let Some(label) = &expression.label else {
                return flow;
            };
            let label = label.name.ident.to_string();
            flow.into_iter()
                .map(|outcome| match outcome {
                    FlowOutcome::Break(Some(target)) if target == label => FlowOutcome::Fallthrough,
                    outcome => outcome,
                })
                .collect()
        }
        syn::Expr::Unsafe(expression) => block_flow(&expression.block),
        syn::Expr::Const(expression) => block_flow(&expression.block),
        syn::Expr::TryBlock(expression) => block_flow(&expression.block),
        syn::Expr::If(expression) => {
            let mut branches = block_flow(&expression.then_branch);
            branches.extend(
                expression
                    .else_branch
                    .as_ref()
                    .map(|(_, alternative)| expression_flow(alternative))
                    .unwrap_or_else(fallthrough_flow),
            );
            sequence_flow(expression_flow(&expression.cond), branches)
        }
        syn::Expr::Match(expression) => {
            let mut arms = BTreeSet::new();
            for arm in &expression.arms {
                arms.extend(expression_flow(&arm.body));
            }
            sequence_flow(expression_flow(&expression.expr), arms)
        }
        syn::Expr::Loop(expression) => loop_flow(
            &expression.body,
            expression
                .label
                .as_ref()
                .map(|label| label.name.ident.to_string()),
            false,
        ),
        syn::Expr::While(expression) => loop_flow(
            &expression.body,
            expression
                .label
                .as_ref()
                .map(|label| label.name.ident.to_string()),
            true,
        ),
        syn::Expr::ForLoop(expression) => loop_flow(
            &expression.body,
            expression
                .label
                .as_ref()
                .map(|label| label.name.ident.to_string()),
            true,
        ),
        syn::Expr::Binary(expression) => {
            let mut flow = sequence_flow(
                expression_flow(&expression.left),
                expression_flow(&expression.right),
            );
            if matches!(&expression.op, syn::BinOp::And(_) | syn::BinOp::Or(_)) {
                flow.insert(FlowOutcome::Fallthrough);
            }
            flow
        }
        syn::Expr::Array(expression) => sequence_expression_flow(&expression.elems),
        syn::Expr::Tuple(expression) => sequence_expression_flow(&expression.elems),
        syn::Expr::Struct(expression) => sequence_expression_flow(
            expression
                .fields
                .iter()
                .map(|field| &field.expr)
                .chain(expression.rest.as_deref()),
        ),
        syn::Expr::Call(expression) => sequence_expression_flow(
            std::iter::once(&*expression.func).chain(expression.args.iter()),
        ),
        syn::Expr::MethodCall(expression) => sequence_expression_flow(
            std::iter::once(&*expression.receiver).chain(expression.args.iter()),
        ),
        syn::Expr::Index(expression) => {
            sequence_expression_flow([&*expression.expr, &*expression.index])
        }
        syn::Expr::Repeat(expression) => {
            sequence_expression_flow([&*expression.expr, &*expression.len])
        }
        syn::Expr::Assign(expression) => {
            sequence_expression_flow([&*expression.left, &*expression.right])
        }
        syn::Expr::Field(expression) => expression_flow(&expression.base),
        syn::Expr::Await(expression) => expression_flow(&expression.base),
        syn::Expr::Try(expression) => expression_flow(&expression.expr),
        syn::Expr::Unary(expression) => expression_flow(&expression.expr),
        syn::Expr::Async(_) | syn::Expr::Closure(_) => fallthrough_flow(),
        _ => fallthrough_flow(),
    }
}

fn block_may_fall_through(block: &syn::Block) -> bool {
    block_flow(block).contains(&FlowOutcome::Fallthrough)
}

fn expression_may_fall_through(expression: &syn::Expr) -> bool {
    expression_flow(expression).contains(&FlowOutcome::Fallthrough)
}

fn pattern_is_obviously_irrefutable(pattern: &syn::Pat) -> bool {
    match pattern {
        syn::Pat::Wild(_) => true,
        syn::Pat::Ident(pattern) => pattern.subpat.is_none(),
        syn::Pat::Type(pattern) => pattern_is_obviously_irrefutable(&pattern.pat),
        syn::Pat::Reference(pattern) => pattern_is_obviously_irrefutable(&pattern.pat),
        syn::Pat::Paren(pattern) => pattern_is_obviously_irrefutable(&pattern.pat),
        syn::Pat::Tuple(pattern) => pattern.elems.iter().all(pattern_is_obviously_irrefutable),
        syn::Pat::Or(pattern) => pattern.cases.iter().any(pattern_is_obviously_irrefutable),
        _ => false,
    }
}

fn merge_origins(origins: impl IntoIterator<Item = ValueOrigin>) -> ValueOrigin {
    let mut result = ValueOrigin::Other;
    for origin in origins {
        result = match (result, origin) {
            (ValueOrigin::ModelText, _) | (_, ValueOrigin::ModelText) => ValueOrigin::ModelText,
            (ValueOrigin::Model, _) | (_, ValueOrigin::Model) => ValueOrigin::Model,
            (ValueOrigin::ProtectedSource, _) | (_, ValueOrigin::ProtectedSource) => {
                ValueOrigin::ProtectedSource
            }
            _ => ValueOrigin::Other,
        };
    }
    result
}

fn expression_key<T>(expression: &T) -> usize {
    expression as *const T as usize
}

fn is_model_text_method(name: &str) -> bool {
    matches!(
        name,
        "export" | "format" | "render" | "serialize" | "to_string" | "write_to_string"
    )
}

fn macro_identifiers(item_macro: &syn::Macro) -> BTreeSet<String> {
    item_macro
        .tokens
        .to_string()
        .split(|character: char| !character.is_alphanumeric() && character != '_')
        .filter(|token| !token.is_empty())
        .map(str::to_owned)
        .collect()
}

fn macro_path(item_macro: &syn::Macro) -> String {
    item_macro
        .path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

fn is_harmless_body_macro(name: &str) -> bool {
    matches!(
        name,
        "assert"
            | "assert_eq"
            | "assert_ne"
            | "cfg"
            | "column"
            | "concat"
            | "debug_assert"
            | "debug_assert_eq"
            | "debug_assert_ne"
            | "eprintln"
            | "env"
            | "file"
            | "format"
            | "format_args"
            | "include_bytes"
            | "include_str"
            | "line"
            | "matches"
            | "module_path"
            | "option_env"
            | "panic"
            | "println"
            | "stringify"
            | "todo"
            | "unimplemented"
            | "unreachable"
            | "vec"
            | "write"
            | "writeln"
    )
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
    module: ModuleIdentity,
    module_dir: PathBuf,
    units: &mut Vec<SourceUnit>,
    seen: &mut BTreeSet<(PathBuf, ModuleIdentity)>,
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
    )?;
    collect_body_includes(
        workspace,
        source_path,
        &parsed,
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
    module: &ModuleIdentity,
    module_dir: &Path,
    units: &mut Vec<SourceUnit>,
    seen: &mut BTreeSet<(PathBuf, ModuleIdentity)>,
    violations: &mut Vec<Violation>,
) -> io::Result<()> {
    for item in items {
        match item {
            Item::Mod(item_mod) if !is_test_only(&item_mod.attrs) => {
                let nested_module = module.child(item_mod.ident.to_string());
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
                        module.clone(),
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

#[derive(Default)]
struct BodyIncludeCollector {
    includes: Vec<syn::Macro>,
}

impl<'ast> Visit<'ast> for BodyIncludeCollector {
    fn visit_item_macro(&mut self, item_macro: &'ast ItemMacro) {
        if !item_macro.mac.path.is_ident("include") {
            visit::visit_item_macro(self, item_macro);
        }
    }

    fn visit_macro(&mut self, item_macro: &'ast syn::Macro) {
        if item_macro.path.is_ident("include") {
            self.includes.push(item_macro.clone());
        }
        visit::visit_macro(self, item_macro);
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_body_includes(
    workspace: &Path,
    containing_source: &Path,
    parsed: &syn::File,
    module: &ModuleIdentity,
    module_dir: &Path,
    units: &mut Vec<SourceUnit>,
    seen: &mut BTreeSet<(PathBuf, ModuleIdentity)>,
    violations: &mut Vec<Violation>,
) -> io::Result<()> {
    let mut collector = BodyIncludeCollector::default();
    collector.visit_file(parsed);
    for item_macro in collector.includes {
        let literal = syn::parse2::<syn::LitStr>(item_macro.tokens.clone()).ok();
        let Some(literal) = literal else {
            violations.push(Violation {
                path: workspace_relative(workspace, containing_source),
                reason: "generated Rust include in a function/statement/expression is not statically auditable; use a literal include! path".into(),
            });
            continue;
        };
        let source_path = containing_source
            .parent()
            .unwrap_or(workspace)
            .join(literal.value());
        let source = fs::read_to_string(&source_path)?;
        if syn::parse_file(&source).is_ok() {
            collect_module_source(
                workspace,
                &source_path,
                module.clone(),
                module_dir.to_path_buf(),
                units,
                seen,
                violations,
            )?;
            continue;
        }

        let canonical = fs::canonicalize(&source_path)?;
        if !seen.insert((canonical, module.clone())) {
            continue;
        }
        let wrapped = format!("fn __authority_body_include() {{\n{source}\n}}\n");
        let wrapped_file = match syn::parse_file(&wrapped) {
            Ok(parsed) => parsed,
            Err(error) => {
                violations.push(Violation {
                    path: workspace_relative(workspace, &source_path),
                    reason: format!(
                        "literal function-body include is not statically auditable as Rust statements or an expression: {error}"
                    ),
                });
                continue;
            }
        };
        violations.push(Violation {
            path: workspace_relative(workspace, &source_path),
            reason: "context-dependent function-body include cannot be analyzed with its enclosing bindings; keep production authority code in a standalone Rust item or module"
                .into(),
        });
        units.push(SourceUnit {
            path: workspace_relative(workspace, &source_path),
            source: wrapped,
            module: module.clone(),
        });
        collect_body_includes(
            workspace,
            &source_path,
            &wrapped_file,
            module,
            module_dir,
            units,
            seen,
            violations,
        )?;
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
            let mut display_root = vec![crate_name.clone()];
            if !kinds
                .iter()
                .any(|kind| matches!(*kind, "lib" | "proc-macro"))
            {
                display_root.push(target_name.clone());
            }
            let crate_key = format!(
                "{package_id}|{}|{target_name}|{}",
                kinds.join("+"),
                slash_path(&source)
            );
            let module = ModuleIdentity::new(crate_key, display_root);
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
    module: ModuleIdentity,
}

fn analyze_source_units(units: &[SourceUnit]) -> Vec<Violation> {
    let mut violations = Vec::new();
    let mut summaries = Vec::new();
    let mut imports = BTreeMap::new();
    let mut aliases = BTreeMap::new();
    let mut struct_fields = StructFields::new();
    let mut method_returns = MethodReturns::new();
    let mut trait_visibility = TraitVisibility::new();
    let mut trait_defaults = TraitDefaults::new();

    for unit in units {
        let Ok(parsed) = syn::parse_file(&unit.source) else {
            continue;
        };
        collect_bindings(&parsed.items, &unit.module, &mut imports, &mut aliases);
    }
    for unit in units {
        let Ok(parsed) = syn::parse_file(&unit.source) else {
            continue;
        };
        collect_type_members(
            &parsed.items,
            &unit.module,
            &aliases,
            &imports,
            &mut struct_fields,
            &mut method_returns,
            &mut trait_visibility,
            &mut trait_defaults,
        );
    }

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
        if module_can_access_raw_parser(&unit.module) {
            let local_macros = local_macro_names(&parsed);
            let mut attribute_visitor = AuthorityAttributeVisitor {
                path: &unit.path,
                violations: &mut violations,
                local_macros: &local_macros,
            };
            attribute_visitor.visit_file(&parsed);
        }
        analyze_items(
            &unit.path,
            &parsed.items,
            &unit.module,
            &mut summaries,
            &imports,
            &aliases,
            &struct_fields,
            &method_returns,
            &trait_visibility,
            &trait_defaults,
            &mut violations,
        );
    }

    let edges = resolve_edges(&summaries, &imports, &aliases);
    let unresolved_model_dispatch = find_unresolved_model_dispatch(
        &summaries,
        &imports,
        &aliases,
        &struct_fields,
        &method_returns,
    );
    let direct_shadow = summaries
        .iter()
        .enumerate()
        .filter(|(_, summary)| {
            !summary.allowlisted_leaf_codec
                && summary_is_raw_grammar_entry(summary, &aliases, &imports)
        })
        .map(|(index, _)| index)
        .collect::<BTreeSet<_>>();
    let closure_shadow = summaries
        .iter()
        .enumerate()
        .filter(|(_, summary)| summary.raw_authority_closure && !summary.allowlisted_leaf_codec)
        .map(|(index, _)| index)
        .collect::<BTreeSet<_>>();
    let mut capabilities = summaries
        .iter()
        .enumerate()
        .map(|(index, summary)| {
            let mut local = summary.local;
            local.serialize |= summary_is_model_to_text(summary, &aliases, &imports);
            local.reparse |= direct_shadow.contains(&index) || closure_shadow.contains(&index);
            local
        })
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

    let model_reparse = summaries
        .iter()
        .enumerate()
        .filter(|(index, summary)| {
            summary_has_model_input(summary, &aliases, &imports)
                && capabilities[*index].serialize
                && capabilities[*index].reparse
                && !is_authorized_model_reparse(summary)
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
        .filter(|(_, summary)| summary_exposes_visible_model_text(summary, &aliases, &imports))
        .map(|(index, _)| index)
        .collect::<BTreeSet<_>>();
    let opaque_macro_authority = summaries
        .iter()
        .enumerate()
        .filter(|(_, summary)| !summary.opaque_authority_macros.is_empty())
        .map(|(index, _)| index)
        .collect::<BTreeSet<_>>();
    let seeds = direct_shadow
        .union(&model_reparse)
        .copied()
        .chain(closure_shadow.iter().copied())
        .chain(retired.iter().copied())
        .chain(visible_model_text.iter().copied())
        .chain(unresolved_model_dispatch.iter().copied())
        .chain(opaque_macro_authority.iter().copied())
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
        if closure_shadow.contains(&index) {
            violations.push(Violation {
                path: summary.path.clone(),
                reason: format!(
                    "`{}` defines raw-text grammar in a local closure outside the syntax authority",
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
        if unresolved_model_dispatch.contains(&index) {
            violations.push(Violation {
                path: summary.path.clone(),
                reason: format!(
                    "`{}` sends protected model-to-text data through unresolved callable dispatch",
                    summary.id.display()
                ),
            });
        }
        for item_macro in &summary.opaque_authority_macros {
            violations.push(Violation {
                path: summary.path.clone(),
                reason: format!(
                    "`{}` passes protected authority data through opaque macro `{item_macro}`",
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

fn collect_bindings(
    items: &[Item],
    module: &ModuleIdentity,
    imports: &mut Imports,
    aliases: &mut Aliases,
) {
    for item in items {
        if is_test_only(item_attributes(item)) {
            continue;
        }
        match item {
            Item::Mod(item_mod) => {
                let name = item_mod.ident.to_string();
                imports
                    .entry(module.clone())
                    .or_default()
                    .entry(name.clone())
                    .or_insert_with(|| vec!["self".into(), name.clone()]);
                if let Some((_, nested)) = &item_mod.content {
                    let nested_module = module.child(name);
                    collect_bindings(nested, &nested_module, imports, aliases);
                }
            }
            Item::Use(item_use) => collect_use_bindings(
                Vec::new(),
                &item_use.tree,
                imports.entry(module.clone()).or_default(),
            ),
            Item::ExternCrate(item_extern) => {
                let binding = item_extern
                    .rename
                    .as_ref()
                    .map(|(_, rename)| rename.to_string())
                    .unwrap_or_else(|| item_extern.ident.to_string());
                imports
                    .entry(module.clone())
                    .or_default()
                    .entry(binding)
                    .or_insert_with(|| vec![item_extern.ident.to_string()]);
            }
            Item::Type(item_type) => {
                aliases.insert(
                    (module.clone(), item_type.ident.to_string()),
                    type_paths(&item_type.ty),
                );
            }
            _ => {}
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_type_members(
    items: &[Item],
    module: &ModuleIdentity,
    aliases: &Aliases,
    imports: &Imports,
    struct_fields: &mut StructFields,
    method_returns: &mut MethodReturns,
    trait_visibility: &mut TraitVisibility,
    trait_defaults: &mut TraitDefaults,
) {
    for item in items {
        if is_test_only(item_attributes(item)) {
            continue;
        }
        match item {
            Item::Mod(item_mod) => {
                if let Some((_, nested)) = &item_mod.content {
                    let nested_module = module.child(item_mod.ident.to_string());
                    collect_type_members(
                        nested,
                        &nested_module,
                        aliases,
                        imports,
                        struct_fields,
                        method_returns,
                        trait_visibility,
                        trait_defaults,
                    );
                }
            }
            Item::Struct(item_struct) => {
                let fields = struct_fields
                    .entry(TypeIdentity::new(
                        module.clone(),
                        item_struct.ident.to_string(),
                    ))
                    .or_default();
                for (index, field) in item_struct.fields.iter().enumerate() {
                    let name = field
                        .ident
                        .as_ref()
                        .map(ToString::to_string)
                        .unwrap_or_else(|| index.to_string());
                    fields.insert(
                        name,
                        TypeUse {
                            module: module.clone(),
                            paths: type_paths(&field.ty),
                        },
                    );
                }
            }
            Item::Enum(item_enum) => {
                struct_fields
                    .entry(TypeIdentity::new(
                        module.clone(),
                        item_enum.ident.to_string(),
                    ))
                    .or_default();
            }
            Item::Union(item_union) => {
                struct_fields
                    .entry(TypeIdentity::new(
                        module.clone(),
                        item_union.ident.to_string(),
                    ))
                    .or_default();
            }
            Item::Impl(item_impl) => {
                let owner = receiver_type_identity(
                    module,
                    &type_paths(&item_impl.self_ty),
                    aliases,
                    imports,
                )
                .unwrap_or_else(|| TypeIdentity::new(module.clone(), "_"));
                for item in &item_impl.items {
                    if let ImplItem::Fn(method) = item {
                        if let ReturnType::Type(_, output) = &method.sig.output {
                            method_returns.insert(
                                (owner.clone(), method.sig.ident.to_string()),
                                TypeUse {
                                    module: module.clone(),
                                    paths: type_paths(output),
                                },
                            );
                        }
                    }
                }
            }
            Item::Trait(item_trait) => {
                let owner = TypeIdentity::new(module.clone(), item_trait.ident.to_string());
                struct_fields.entry(owner.clone()).or_default();
                trait_visibility.insert(
                    owner.clone(),
                    !matches!(item_trait.vis, syn::Visibility::Inherited),
                );
                for item in &item_trait.items {
                    if let syn::TraitItem::Fn(method) = item {
                        if let Some(default) = &method.default {
                            trait_defaults
                                .entry(owner.clone())
                                .or_default()
                                .push((method.sig.clone(), default.clone()));
                        }
                        if let ReturnType::Type(_, output) = &method.sig.output {
                            method_returns.insert(
                                (owner.clone(), method.sig.ident.to_string()),
                                TypeUse {
                                    module: module.clone(),
                                    paths: type_paths(output),
                                },
                            );
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn analyze_items(
    path: &str,
    items: &[Item],
    module: &ModuleIdentity,
    summaries: &mut Vec<FunctionSummary>,
    imports: &Imports,
    aliases: &Aliases,
    struct_fields: &StructFields,
    method_returns: &MethodReturns,
    trait_visibility: &TraitVisibility,
    trait_defaults: &TraitDefaults,
    violations: &mut Vec<Violation>,
) {
    let authoritative = is_authoritative_path(path);
    for item in items {
        match item {
            Item::Mod(item_mod) if !is_test_only(&item_mod.attrs) => {
                inspect_module(path, item_mod, violations);
                if let Some((_, nested)) = &item_mod.content {
                    let nested_module = module.child(item_mod.ident.to_string());
                    analyze_items(
                        path,
                        nested,
                        &nested_module,
                        summaries,
                        imports,
                        aliases,
                        struct_fields,
                        method_returns,
                        trait_visibility,
                        trait_defaults,
                        violations,
                    );
                }
            }
            Item::Use(item_use) if !is_test_only(&item_use.attrs) => {
                inspect_use(path, item_use, violations);
            }
            Item::Fn(item_fn) if !is_test_only(&item_fn.attrs) => summarize_function(
                path,
                module,
                None,
                Vec::new(),
                None,
                false,
                &item_fn.sig,
                &item_fn.block,
                !matches!(item_fn.vis, syn::Visibility::Inherited),
                authoritative,
                aliases,
                imports,
                struct_fields,
                method_returns,
                summaries,
            ),
            Item::Impl(item_impl) if !is_test_only(&item_impl.attrs) => summarize_impl(
                path,
                module,
                item_impl,
                authoritative,
                aliases,
                imports,
                struct_fields,
                method_returns,
                trait_visibility,
                trait_defaults,
                summaries,
            ),
            Item::Trait(item_trait) if !is_test_only(&item_trait.attrs) && !authoritative => {
                for trait_item in &item_trait.items {
                    if let syn::TraitItem::Fn(method) = trait_item {
                        if let Some(default) = &method.default {
                            let owner =
                                TypeIdentity::new(module.clone(), item_trait.ident.to_string());
                            summarize_function(
                                path,
                                module,
                                Some(owner.clone()),
                                vec![vec![owner.name.clone()]],
                                None,
                                false,
                                &method.sig,
                                default,
                                !matches!(item_trait.vis, syn::Visibility::Inherited),
                                false,
                                aliases,
                                imports,
                                struct_fields,
                                method_returns,
                                summaries,
                            );
                        } else {
                            if signature_is_resolved_raw_grammar_entry(
                                &method.sig,
                                module,
                                aliases,
                                imports,
                            ) {
                                violations.push(Violation {
                                    path: path.to_owned(),
                                    reason: format!(
                                        "`{}::{}::{}` declares raw-text grammar outside the syntax authority",
                                        module.display(),
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
                                        module.display(),
                                        item_trait.ident,
                                        method.sig.ident
                                    ),
                                });
                            }
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

#[allow(clippy::too_many_arguments)]
fn summarize_impl(
    path: &str,
    module: &ModuleIdentity,
    item_impl: &ItemImpl,
    authoritative: bool,
    aliases: &Aliases,
    imports: &Imports,
    struct_fields: &StructFields,
    method_returns: &MethodReturns,
    trait_visibility: &TraitVisibility,
    trait_defaults: &TraitDefaults,
    summaries: &mut Vec<FunctionSummary>,
) {
    let owner_paths = type_paths(&item_impl.self_ty);
    let owner = receiver_type_identity(module, &owner_paths, aliases, imports)
        .unwrap_or_else(|| TypeIdentity::new(module.clone(), "_"));
    let trait_type = item_impl.trait_.as_ref().and_then(|(_, path, _)| {
        preferred_type_identity(module, &paths_in_path(path), aliases, imports)
    });
    let trait_visible = trait_type
        .as_ref()
        .map(|identity| trait_visibility.get(identity).copied().unwrap_or(true))
        .unwrap_or(false);
    let allowlisted_standard_text_trait =
        trait_type.as_ref().is_some_and(is_standard_to_string_trait);
    let implemented_methods = item_impl
        .items
        .iter()
        .filter_map(|item| match item {
            ImplItem::Fn(method) if !is_test_only(&method.attrs) => {
                Some(method.sig.ident.to_string())
            }
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    for item in &item_impl.items {
        if let ImplItem::Fn(method) = item {
            if is_test_only(&method.attrs) {
                continue;
            }
            summarize_function(
                path,
                module,
                Some(owner.clone()),
                owner_paths.clone(),
                trait_type.clone(),
                allowlisted_standard_text_trait,
                &method.sig,
                &method.block,
                !matches!(method.vis, syn::Visibility::Inherited) || trait_visible,
                authoritative,
                aliases,
                imports,
                struct_fields,
                method_returns,
                summaries,
            );
        }
    }
    if let Some(trait_identity) = &trait_type {
        for (signature, default) in trait_defaults
            .get(trait_identity)
            .into_iter()
            .flatten()
            .filter(|(signature, _)| !implemented_methods.contains(&signature.ident.to_string()))
        {
            summarize_function(
                path,
                module,
                Some(owner.clone()),
                owner_paths.clone(),
                trait_type.clone(),
                allowlisted_standard_text_trait,
                signature,
                default,
                trait_visible,
                authoritative,
                aliases,
                imports,
                struct_fields,
                method_returns,
                summaries,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn summarize_function(
    path: &str,
    module: &ModuleIdentity,
    owner: Option<TypeIdentity>,
    owner_paths: Vec<Vec<String>>,
    trait_type: Option<TypeIdentity>,
    allowlisted_standard_text_trait: bool,
    signature: &Signature,
    block: &syn::Block,
    visible: bool,
    authoritative: bool,
    aliases: &Aliases,
    imports: &Imports,
    struct_fields: &StructFields,
    method_returns: &MethodReturns,
    summaries: &mut Vec<FunctionSummary>,
) {
    if authoritative {
        return;
    }
    let mut facts = BodyFacts::new(BodyEnvironment {
        module,
        aliases,
        imports,
        struct_fields,
        method_returns,
    });
    for argument in &signature.inputs {
        match argument {
            FnArg::Receiver(_) => {
                if let Some(owner) = &owner {
                    facts.receiver_types.insert("self".into(), owner.clone());
                    facts.receiver_type_uses.insert(
                        "self".into(),
                        vec![TypeUse {
                            module: module.clone(),
                            paths: owner_paths.clone(),
                        }],
                    );
                    facts.bindings.insert("self".into());
                    let owner_names = resolved_type_names(module, &owner_paths, aliases, imports);
                    facts.origins.insert(
                        "self".into(),
                        if owner_names
                            .iter()
                            .any(|name| MODEL_TYPES.contains(&name.as_str()))
                        {
                            ValueOrigin::Model
                        } else {
                            ValueOrigin::Other
                        },
                    );
                }
            }
            FnArg::Typed(argument) => {
                let bindings = pattern_binding_names(&argument.pat);
                if !bindings.is_empty() {
                    let paths = type_paths(&argument.ty);
                    let identity = receiver_type_identity(module, &paths, aliases, imports);
                    let type_uses = vec![TypeUse {
                        module: module.clone(),
                        paths: paths.clone(),
                    }];
                    let origin = origin_for_type(module, &paths, aliases, imports);
                    for binding in bindings {
                        if let Some(identity) = &identity {
                            facts
                                .receiver_types
                                .insert(binding.clone(), identity.clone());
                        }
                        facts
                            .receiver_type_uses
                            .insert(binding.clone(), type_uses.clone());
                        facts.bindings.insert(binding.clone());
                        facts.origins.insert(binding, origin);
                    }
                }
            }
        }
    }
    facts.visit_block(block);

    let input_types = signature
        .inputs
        .iter()
        .filter_map(|argument| match argument {
            FnArg::Receiver(_) => None,
            FnArg::Typed(argument) => Some(ParameterSummary {
                name: match &*argument.pat {
                    syn::Pat::Ident(pattern) => Some(pattern.ident.to_string()),
                    syn::Pat::Type(pattern) => match &*pattern.pat {
                        syn::Pat::Ident(pattern) => Some(pattern.ident.to_string()),
                        _ => None,
                    },
                    _ => None,
                },
                paths: type_paths(&argument.ty),
                mutable: type_is_mutable_reference(&argument.ty),
            }),
        })
        .collect::<Vec<_>>();
    let model_input = signature.inputs.iter().any(|argument| match argument {
        FnArg::Receiver(_) => false,
        FnArg::Typed(argument) => type_contains_any(&argument.ty, MODEL_TYPES),
    });
    let output_paths = match &signature.output {
        ReturnType::Default => Vec::new(),
        ReturnType::Type(_, output) => type_paths(output),
    };
    let output_type = match &signature.output {
        ReturnType::Default => None,
        ReturnType::Type(_, output) => Some((**output).clone()),
    };
    let name = signature.ident.to_string();
    let id = FunctionId {
        module: module.clone(),
        owner: owner.as_ref().map(|identity| identity.name.clone()),
        name: name.clone(),
    };

    let mut local_standard_roots = block_local_standard_roots(block);
    local_standard_roots.extend(
        signature
            .generics
            .type_params()
            .map(|parameter| parameter.ident.to_string())
            .filter(|name| matches!(name.as_str(), "std" | "alloc" | "core" | "Vec")),
    );
    if owner_paths.iter().any(|path| path.as_slice() == ["Vec"]) {
        local_standard_roots.insert("Vec".into());
    }

    summaries.push(FunctionSummary {
        path: path.to_owned(),
        id: id.clone(),
        block: block.clone(),
        calls: facts.calls,
        local: facts.local,
        input_types,
        owner_paths,
        owner_type: owner,
        has_receiver: signature
            .inputs
            .iter()
            .any(|argument| matches!(argument, FnArg::Receiver(_))),
        trait_type,
        allowlisted_standard_text_trait,
        model_input,
        output_type,
        output_paths,
        body_paths: facts.body_paths,
        body_type_uses: facts.body_type_uses,
        local_standard_roots,
        retired_calls: facts.retired_calls,
        opaque_authority_macros: facts.opaque_authority_macros,
        raw_authority_closure: facts.raw_authority_closure,
        allowlisted_leaf_codec: is_allowlisted_leaf_codec(&id),
        visible,
    });
}

fn block_local_standard_roots(block: &syn::Block) -> BTreeSet<String> {
    #[derive(Default)]
    struct LocalRootBindings {
        roots: BTreeSet<String>,
    }

    impl LocalRootBindings {
        fn record(&mut self, name: impl Into<String>) {
            let name = name.into();
            if matches!(name.as_str(), "std" | "alloc" | "core" | "Vec") {
                self.roots.insert(name);
            }
        }
    }

    impl<'ast> Visit<'ast> for LocalRootBindings {
        fn visit_item_mod(&mut self, item_mod: &'ast ItemMod) {
            self.record(item_mod.ident.to_string());
        }

        fn visit_item_use(&mut self, item_use: &'ast ItemUse) {
            let mut bindings = BTreeMap::new();
            collect_use_bindings(Vec::new(), &item_use.tree, &mut bindings);
            for binding in bindings.into_keys() {
                if binding == "*" {
                    self.roots.insert("Vec".into());
                } else {
                    self.record(binding);
                }
            }
        }

        fn visit_item_extern_crate(&mut self, item_extern: &'ast syn::ItemExternCrate) {
            self.record(
                item_extern
                    .rename
                    .as_ref()
                    .map(|(_, rename)| rename.to_string())
                    .unwrap_or_else(|| item_extern.ident.to_string()),
            );
        }

        fn visit_item_struct(&mut self, item_struct: &'ast syn::ItemStruct) {
            self.record(item_struct.ident.to_string());
        }

        fn visit_item_enum(&mut self, item_enum: &'ast syn::ItemEnum) {
            self.record(item_enum.ident.to_string());
        }

        fn visit_item_union(&mut self, item_union: &'ast syn::ItemUnion) {
            self.record(item_union.ident.to_string());
        }

        fn visit_item_trait(&mut self, item_trait: &'ast syn::ItemTrait) {
            self.record(item_trait.ident.to_string());
        }

        fn visit_item_type(&mut self, item_type: &'ast syn::ItemType) {
            self.record(item_type.ident.to_string());
        }

        fn visit_item_fn(&mut self, _item_fn: &'ast syn::ItemFn) {}
    }

    let mut bindings = LocalRootBindings::default();
    bindings.visit_block(block);
    bindings.roots
}

fn resolve_edges(
    summaries: &[FunctionSummary],
    imports: &Imports,
    aliases: &Aliases,
) -> Vec<BTreeSet<usize>> {
    let mut edges = vec![BTreeSet::new(); summaries.len()];
    for (caller_index, caller) in summaries.iter().enumerate() {
        for call in &caller.calls {
            let targets = resolve_call(caller, call, summaries, imports, aliases);
            edges[caller_index].extend(targets);
        }
    }
    edges
}

fn resolve_call(
    caller: &FunctionSummary,
    call: &CallSite,
    summaries: &[FunctionSummary],
    imports: &Imports,
    aliases: &Aliases,
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
                    indirect: call.indirect,
                },
                summaries,
                imports,
                aliases,
            );
        }
    }
    let associated_owner = (!call.method && call.segments.len() >= 2)
        .then(|| call.segments[..call.segments.len() - 1].to_vec())
        .filter(|owner| {
            owner
                .last()
                .is_some_and(|name| name.chars().next().is_some_and(char::is_uppercase))
        })
        .and_then(|path| preferred_type_identity(&caller.id.module, &[path], aliases, imports));
    let compatible = |candidate: &&FunctionSummary| {
        candidate.id.name == *name
            && if call.method {
                call.receiver_type.as_ref().is_some_and(|owner| {
                    candidate.owner_type.as_ref() == Some(owner)
                        || candidate.trait_type.as_ref() == Some(owner)
                })
            } else if let Some(owner) = &associated_owner {
                candidate.owner_type.as_ref() == Some(owner)
            } else {
                candidate.owner_type.is_none()
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
            expected.reset_to_crate_root();
            segments = &segments[1..];
        }
        Some("self") => segments = &segments[1..],
        Some("super") => {
            expected.pop_module();
            segments = &segments[1..];
        }
        _ => expected.reset_to_crate_root(),
    }
    expected.extend(segments[..segments.len().saturating_sub(1)].iter().cloned());
    named
        .into_iter()
        .filter(|(_, candidate)| candidate.id.module == expected)
        .map(|(index, _)| index)
        .collect()
}

fn find_unresolved_model_dispatch(
    summaries: &[FunctionSummary],
    imports: &Imports,
    aliases: &Aliases,
    struct_fields: &StructFields,
    method_returns: &MethodReturns,
) -> BTreeSet<usize> {
    let model_text_returns = summaries
        .iter()
        .enumerate()
        .filter(|(_, summary)| summary_is_model_to_text(summary, aliases, imports))
        .map(|(index, _)| index)
        .collect::<BTreeSet<_>>();
    summaries
        .iter()
        .enumerate()
        .filter_map(|(index, caller)| {
            let mut facts = ReturnTaintFacts::new(
                BodyEnvironment {
                    module: &caller.id.module,
                    aliases,
                    imports,
                    struct_fields,
                    method_returns,
                },
                caller,
                summaries,
                &model_text_returns,
            );
            facts.visit_block(&caller.block);
            facts.unresolved_model_dispatch.then_some(index)
        })
        .collect()
}

struct ReturnTaintFacts<'env> {
    env: BodyEnvironment<'env>,
    caller: &'env FunctionSummary,
    summaries: &'env [FunctionSummary],
    model_text_returns: &'env BTreeSet<usize>,
    receiver_types: BTreeMap<String, TypeIdentity>,
    callable_paths: BTreeMap<String, Vec<String>>,
    callable_origins: BTreeMap<String, ValueOrigin>,
    bindings: BTreeSet<String>,
    origins: BTreeMap<String, ValueOrigin>,
    binding_scopes: Vec<BTreeMap<String, TaintBindingState>>,
    expression_origins: BTreeMap<usize, ValueOrigin>,
    break_origins: Vec<BreakOriginState>,
    return_origins: Vec<Vec<ValueOrigin>>,
    unresolved_model_dispatch: bool,
}

#[derive(Clone, Eq, PartialEq)]
struct TaintEnvironment {
    receiver_types: BTreeMap<String, TypeIdentity>,
    callable_paths: BTreeMap<String, Vec<String>>,
    callable_origins: BTreeMap<String, ValueOrigin>,
    bindings: BTreeSet<String>,
    origins: BTreeMap<String, ValueOrigin>,
}

struct TaintBindingState {
    receiver_type: Option<TypeIdentity>,
    callable_path: Option<Vec<String>>,
    callable_origin: Option<ValueOrigin>,
    bound: bool,
    origin: Option<ValueOrigin>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BreakTargetKind {
    Loop,
    While,
    For,
    Block,
}

impl BreakTargetKind {
    fn is_loop(self) -> bool {
        matches!(self, Self::Loop | Self::While | Self::For)
    }
}

struct BreakOriginState {
    kind: BreakTargetKind,
    label: Option<String>,
    binding_scope_depth: usize,
    origins: Vec<ValueOrigin>,
    environments: Vec<TaintEnvironment>,
    continue_environments: Vec<TaintEnvironment>,
}

impl<'env> ReturnTaintFacts<'env> {
    fn new(
        env: BodyEnvironment<'env>,
        caller: &'env FunctionSummary,
        summaries: &'env [FunctionSummary],
        model_text_returns: &'env BTreeSet<usize>,
    ) -> Self {
        let mut facts = Self {
            env,
            caller,
            summaries,
            model_text_returns,
            receiver_types: BTreeMap::new(),
            callable_paths: BTreeMap::new(),
            callable_origins: BTreeMap::new(),
            bindings: BTreeSet::new(),
            origins: BTreeMap::new(),
            binding_scopes: Vec::new(),
            expression_origins: BTreeMap::new(),
            break_origins: Vec::new(),
            return_origins: Vec::new(),
            unresolved_model_dispatch: false,
        };
        if caller.has_receiver {
            if let Some(owner) = &caller.owner_type {
                facts.receiver_types.insert("self".into(), owner.clone());
                facts.bindings.insert("self".into());
                facts.origins.insert(
                    "self".into(),
                    if MODEL_TYPES.contains(&owner.name.as_str()) {
                        ValueOrigin::Model
                    } else {
                        ValueOrigin::Other
                    },
                );
            }
        }
        for input in &caller.input_types {
            let Some(name) = &input.name else {
                continue;
            };
            facts.bindings.insert(name.clone());
            if let Some(identity) = receiver_type_identity(
                facts.env.module,
                &input.paths,
                facts.env.aliases,
                facts.env.imports,
            ) {
                facts.receiver_types.insert(name.clone(), identity);
            }
            facts.origins.insert(
                name.clone(),
                origin_for_type(
                    facts.env.module,
                    &input.paths,
                    facts.env.aliases,
                    facts.env.imports,
                ),
            );
        }
        facts
    }

    fn snapshot_environment(&self) -> TaintEnvironment {
        TaintEnvironment {
            receiver_types: self.receiver_types.clone(),
            callable_paths: self.callable_paths.clone(),
            callable_origins: self.callable_origins.clone(),
            bindings: self.bindings.clone(),
            origins: self.origins.clone(),
        }
    }

    fn restore_environment(&mut self, environment: TaintEnvironment) {
        self.receiver_types = environment.receiver_types;
        self.callable_paths = environment.callable_paths;
        self.callable_origins = environment.callable_origins;
        self.bindings = environment.bindings;
        self.origins = environment.origins;
    }

    fn snapshot_environment_at_scope_depth(&self, scope_depth: usize) -> TaintEnvironment {
        self.environment_at_scope_depth(self.snapshot_environment(), scope_depth)
    }

    fn environment_at_scope_depth(
        &self,
        mut environment: TaintEnvironment,
        scope_depth: usize,
    ) -> TaintEnvironment {
        for scope in self.binding_scopes[scope_depth..].iter().rev() {
            for (binding, state) in scope {
                match &state.receiver_type {
                    Some(value) => {
                        environment
                            .receiver_types
                            .insert(binding.clone(), value.clone());
                    }
                    None => {
                        environment.receiver_types.remove(binding);
                    }
                }
                match &state.callable_path {
                    Some(value) => {
                        environment
                            .callable_paths
                            .insert(binding.clone(), value.clone());
                    }
                    None => {
                        environment.callable_paths.remove(binding);
                    }
                }
                match state.callable_origin {
                    Some(value) => {
                        environment.callable_origins.insert(binding.clone(), value);
                    }
                    None => {
                        environment.callable_origins.remove(binding);
                    }
                }
                if state.bound {
                    environment.bindings.insert(binding.clone());
                } else {
                    environment.bindings.remove(binding);
                }
                match state.origin {
                    Some(value) => {
                        environment.origins.insert(binding.clone(), value);
                    }
                    None => {
                        environment.origins.remove(binding);
                    }
                }
            }
        }
        environment
    }

    fn join_environments(environments: Vec<TaintEnvironment>) -> TaintEnvironment {
        let mut environments = environments.into_iter();
        let mut joined = environments
            .next()
            .expect("environment join has at least one path");
        for environment in environments {
            joined.receiver_types.retain(|binding, identity| {
                environment.receiver_types.get(binding) == Some(identity)
            });
            joined
                .callable_paths
                .retain(|binding, path| environment.callable_paths.get(binding) == Some(path));
            joined
                .bindings
                .retain(|binding| environment.bindings.contains(binding));
            for (binding, origin) in environment.callable_origins {
                joined
                    .callable_origins
                    .entry(binding)
                    .and_modify(|current| *current = merge_origins([*current, origin]))
                    .or_insert(origin);
            }
            for (binding, origin) in environment.origins {
                joined
                    .origins
                    .entry(binding)
                    .and_modify(|current| *current = merge_origins([*current, origin]))
                    .or_insert(origin);
            }
        }
        joined
    }

    fn push_binding_scope(&mut self) {
        self.binding_scopes.push(BTreeMap::new());
    }

    fn record_binding_declaration(&mut self, binding: &str) {
        let state = TaintBindingState {
            receiver_type: self.receiver_types.get(binding).cloned(),
            callable_path: self.callable_paths.get(binding).cloned(),
            callable_origin: self.callable_origins.get(binding).copied(),
            bound: self.bindings.contains(binding),
            origin: self.origins.get(binding).copied(),
        };
        if let Some(scope) = self.binding_scopes.last_mut() {
            scope.entry(binding.to_owned()).or_insert(state);
        }
    }

    fn pop_binding_scope(&mut self) {
        let scope = self
            .binding_scopes
            .pop()
            .expect("binding scope is balanced");
        for (binding, state) in scope {
            match state.receiver_type {
                Some(value) => {
                    self.receiver_types.insert(binding.clone(), value);
                }
                None => {
                    self.receiver_types.remove(&binding);
                }
            }
            match state.callable_path {
                Some(value) => {
                    self.callable_paths.insert(binding.clone(), value);
                }
                None => {
                    self.callable_paths.remove(&binding);
                }
            }
            match state.callable_origin {
                Some(value) => {
                    self.callable_origins.insert(binding.clone(), value);
                }
                None => {
                    self.callable_origins.remove(&binding);
                }
            }
            if state.bound {
                self.bindings.insert(binding.clone());
            } else {
                self.bindings.remove(&binding);
            }
            match state.origin {
                Some(value) => {
                    self.origins.insert(binding, value);
                }
                None => {
                    self.origins.remove(&binding);
                }
            }
        }
    }

    fn cached_expression_origin<T>(&self, expression: &T) -> ValueOrigin {
        self.expression_origins
            .get(&expression_key(expression))
            .copied()
            .unwrap_or(ValueOrigin::Other)
    }

    fn cache_expression_origin<T>(&mut self, expression: &T, origin: ValueOrigin) {
        self.expression_origins
            .insert(expression_key(expression), origin);
    }

    fn callable_result_origin(&self, expression: &syn::Expr) -> ValueOrigin {
        match unwrap_expression(expression) {
            syn::Expr::Closure(expression) => self.cached_expression_origin(expression),
            syn::Expr::Path(path) if path.path.segments.len() == 1 => self
                .callable_origins
                .get(&path.path.segments[0].ident.to_string())
                .copied()
                .unwrap_or(ValueOrigin::Other),
            _ => ValueOrigin::Other,
        }
    }

    fn visit_value_block<T>(
        &mut self,
        expression: &T,
        attributes: &[syn::Attribute],
        block: &syn::Block,
    ) {
        for attribute in attributes {
            self.visit_attribute(attribute);
        }
        let origin = self.visit_scoped_block(block);
        self.cache_expression_origin(expression, origin);
    }

    fn finish_return_context(&mut self, tail_origin: ValueOrigin) -> ValueOrigin {
        let returns = self
            .return_origins
            .pop()
            .expect("return-origin context is balanced");
        merge_origins(std::iter::once(tail_origin).chain(returns))
    }

    fn bind_pattern_from_expression(&mut self, pattern: &syn::Pat, expression: &syn::Expr) {
        let identity = self.expression_type_identity(expression);
        let origin = self.expression_origin(expression);
        let callable_origin = self.callable_result_origin(expression);
        self.bind_pattern_with_evidence(pattern, identity, origin, callable_origin);
    }

    fn bind_pattern_with_evidence(
        &mut self,
        pattern: &syn::Pat,
        identity: Option<TypeIdentity>,
        origin: ValueOrigin,
        callable_origin: ValueOrigin,
    ) {
        let bindings = pattern_binding_names(pattern);
        for binding in bindings {
            self.record_binding_declaration(&binding);
            self.bindings.insert(binding.clone());
            self.callable_paths.remove(&binding);
            if callable_origin == ValueOrigin::Other {
                self.callable_origins.remove(&binding);
            } else {
                self.callable_origins
                    .insert(binding.clone(), callable_origin);
            }
            if let Some(identity) = &identity {
                self.receiver_types
                    .insert(binding.clone(), identity.clone());
            } else {
                self.receiver_types.remove(&binding);
            }
            self.origins.insert(binding, origin);
        }
    }

    fn visit_scoped_block(&mut self, block: &syn::Block) -> ValueOrigin {
        self.push_binding_scope();
        visit::visit_block(self, block);
        let origin = block
            .stmts
            .last()
            .and_then(|statement| match statement {
                syn::Stmt::Expr(expression, None) => Some(self.expression_origin(expression)),
                _ => None,
            })
            .unwrap_or(ValueOrigin::Other);
        self.pop_binding_scope();
        origin
    }

    fn call_site(&self, call: &ExprCall) -> CallSite {
        let (mut segments, mut indirect) = match unwrap_expression(&call.func) {
            syn::Expr::Path(ExprPath { path, .. }) => {
                let segments = path
                    .segments
                    .iter()
                    .map(|segment| segment.ident.to_string())
                    .collect::<Vec<_>>();
                let indirect = segments.len() == 1 && self.bindings.contains(&segments[0]);
                (segments, indirect)
            }
            syn::Expr::Field(field) => (
                vec![match &field.member {
                    syn::Member::Named(name) => name.to_string(),
                    syn::Member::Unnamed(index) => index.index.to_string(),
                }],
                true,
            ),
            _ => (Vec::new(), true),
        };
        if segments.len() == 1 {
            if let Some(target) = self.callable_paths.get(&segments[0]) {
                segments = target.clone();
                indirect = true;
            }
        }
        CallSite {
            segments,
            method: false,
            receiver_type: None,
            indirect,
        }
    }

    fn method_call_site(&self, call: &ExprMethodCall) -> CallSite {
        CallSite {
            segments: vec![call.method.to_string()],
            method: true,
            receiver_type: self.expression_type_identity(&call.receiver),
            indirect: true,
        }
    }

    fn call_returns_model_text(&self, call: &CallSite) -> bool {
        resolve_call(
            self.caller,
            call,
            self.summaries,
            self.env.imports,
            self.env.aliases,
        )
        .iter()
        .any(|target| self.model_text_returns.contains(target))
    }

    fn expression_type_identity(&self, expression: &syn::Expr) -> Option<TypeIdentity> {
        match unwrap_expression(expression) {
            syn::Expr::Path(expression) => {
                if expression.path.segments.len() == 1 {
                    let name = expression.path.segments[0].ident.to_string();
                    if let Some(identity) = self.receiver_types.get(&name) {
                        return Some(identity.clone());
                    }
                }
                receiver_type_identity(
                    self.env.module,
                    &paths_in_path(&expression.path),
                    self.env.aliases,
                    self.env.imports,
                )
            }
            syn::Expr::Field(expression) => {
                let owner = self.expression_type_identity(&expression.base)?;
                let field = match &expression.member {
                    syn::Member::Named(name) => name.to_string(),
                    syn::Member::Unnamed(index) => index.index.to_string(),
                };
                let field_type = self.env.struct_fields.get(&owner)?.get(&field)?;
                receiver_type_identity(
                    &field_type.module,
                    &field_type.paths,
                    self.env.aliases,
                    self.env.imports,
                )
            }
            syn::Expr::MethodCall(expression) => {
                let owner = self.expression_type_identity(&expression.receiver)?;
                let output = self
                    .env
                    .method_returns
                    .get(&(owner, expression.method.to_string()))?;
                receiver_type_identity(
                    &output.module,
                    &output.paths,
                    self.env.aliases,
                    self.env.imports,
                )
            }
            syn::Expr::Struct(expression) => preferred_type_identity(
                self.env.module,
                &paths_in_path(&expression.path),
                self.env.aliases,
                self.env.imports,
            ),
            syn::Expr::Call(expression) => match unwrap_expression(&expression.func) {
                syn::Expr::Path(function) => {
                    let mut segments = function
                        .path
                        .segments
                        .iter()
                        .map(|segment| segment.ident.to_string())
                        .collect::<Vec<_>>();
                    if segments.len() >= 2
                        && segments.last().is_some_and(|method| {
                            method.chars().next().is_some_and(char::is_lowercase)
                        })
                    {
                        segments.pop();
                    }
                    receiver_type_identity(
                        self.env.module,
                        &[segments],
                        self.env.aliases,
                        self.env.imports,
                    )
                }
                _ => None,
            },
            syn::Expr::Unary(expression) if matches!(expression.op, syn::UnOp::Deref(_)) => {
                self.expression_type_identity(&expression.expr)
            }
            _ => None,
        }
    }

    fn expression_origin(&self, expression: &syn::Expr) -> ValueOrigin {
        match unwrap_expression(expression) {
            syn::Expr::Path(path) if path.path.segments.len() == 1 => self
                .origins
                .get(&path.path.segments[0].ident.to_string())
                .copied()
                .unwrap_or(ValueOrigin::Other),
            syn::Expr::Field(field) => match self.expression_origin(&field.base) {
                ValueOrigin::Model => ValueOrigin::Model,
                origin => origin,
            },
            syn::Expr::MethodCall(call) => self.cached_expression_origin(call),
            syn::Expr::Call(call) => self.cached_expression_origin(call),
            syn::Expr::Block(expression) => self.cached_expression_origin(expression),
            syn::Expr::If(expression) => self.cached_expression_origin(expression),
            syn::Expr::Match(expression) => self.cached_expression_origin(expression),
            syn::Expr::Loop(expression) => self.cached_expression_origin(expression),
            syn::Expr::Async(expression) => self.cached_expression_origin(expression),
            syn::Expr::Const(expression) => self.cached_expression_origin(expression),
            syn::Expr::TryBlock(expression) => self.cached_expression_origin(expression),
            syn::Expr::Unsafe(expression) => self.cached_expression_origin(expression),
            syn::Expr::Await(expression) => self.expression_origin(&expression.base),
            syn::Expr::Index(expression) => self.cached_expression_origin(expression),
            syn::Expr::Repeat(expression) => self.cached_expression_origin(expression),
            syn::Expr::Try(expression) => self.expression_origin(&expression.expr),
            syn::Expr::Unary(expression) => self.expression_origin(&expression.expr),
            syn::Expr::Macro(expression) => {
                let identifiers = macro_identifiers(&expression.mac);
                if identifiers.iter().any(|identifier| {
                    self.origins
                        .get(identifier)
                        .is_some_and(|origin| *origin == ValueOrigin::ModelText)
                }) {
                    ValueOrigin::ModelText
                } else {
                    ValueOrigin::Other
                }
            }
            syn::Expr::Binary(expression) => self.cached_expression_origin(expression),
            syn::Expr::Array(expression) => self.cached_expression_origin(expression),
            syn::Expr::Tuple(expression) => self.cached_expression_origin(expression),
            syn::Expr::Struct(expression) => self.cached_expression_origin(expression),
            _ => ValueOrigin::Other,
        }
    }
}

impl<'ast> Visit<'ast> for ReturnTaintFacts<'_> {
    fn visit_block(&mut self, block: &'ast syn::Block) {
        self.visit_scoped_block(block);
    }

    fn visit_expr_binary(&mut self, expression: &'ast syn::ExprBinary) {
        for attribute in &expression.attrs {
            self.visit_attribute(attribute);
        }
        self.visit_expr(&expression.left);
        let left_origin = self.expression_origin(&expression.left);
        let skipped_right_environment =
            matches!(&expression.op, syn::BinOp::And(_) | syn::BinOp::Or(_))
                .then(|| self.snapshot_environment());
        self.visit_expr(&expression.right);
        let right_origin = self.expression_origin(&expression.right);
        if let Some(skipped_right_environment) = skipped_right_environment {
            if !expression_may_fall_through(&expression.right) {
                self.restore_environment(skipped_right_environment);
            } else {
                let evaluated_right_environment = self.snapshot_environment();
                self.restore_environment(Self::join_environments(vec![
                    skipped_right_environment,
                    evaluated_right_environment,
                ]));
            }
        }
        self.cache_expression_origin(expression, merge_origins([left_origin, right_origin]));
    }

    fn visit_expr_array(&mut self, expression: &'ast syn::ExprArray) {
        for attribute in &expression.attrs {
            self.visit_attribute(attribute);
        }
        let mut origins = Vec::with_capacity(expression.elems.len());
        for element in &expression.elems {
            self.visit_expr(element);
            origins.push(self.expression_origin(element));
        }
        self.cache_expression_origin(expression, merge_origins(origins));
    }

    fn visit_expr_tuple(&mut self, expression: &'ast syn::ExprTuple) {
        for attribute in &expression.attrs {
            self.visit_attribute(attribute);
        }
        let mut origins = Vec::with_capacity(expression.elems.len());
        for element in &expression.elems {
            self.visit_expr(element);
            origins.push(self.expression_origin(element));
        }
        self.cache_expression_origin(expression, merge_origins(origins));
    }

    fn visit_expr_struct(&mut self, expression: &'ast syn::ExprStruct) {
        for attribute in &expression.attrs {
            self.visit_attribute(attribute);
        }
        if let Some(qself) = &expression.qself {
            self.visit_qself(qself);
        }
        self.visit_path(&expression.path);
        let mut origins =
            Vec::with_capacity(expression.fields.len() + usize::from(expression.rest.is_some()));
        for field in &expression.fields {
            for attribute in &field.attrs {
                self.visit_attribute(attribute);
            }
            self.visit_expr(&field.expr);
            origins.push(self.expression_origin(&field.expr));
        }
        if let Some(rest) = &expression.rest {
            self.visit_expr(rest);
            origins.push(self.expression_origin(rest));
        }
        self.cache_expression_origin(expression, merge_origins(origins));
    }

    fn visit_expr_index(&mut self, expression: &'ast syn::ExprIndex) {
        for attribute in &expression.attrs {
            self.visit_attribute(attribute);
        }
        self.visit_expr(&expression.expr);
        let origin = self.expression_origin(&expression.expr);
        self.visit_expr(&expression.index);
        self.cache_expression_origin(expression, origin);
    }

    fn visit_expr_repeat(&mut self, expression: &'ast syn::ExprRepeat) {
        for attribute in &expression.attrs {
            self.visit_attribute(attribute);
        }
        self.visit_expr(&expression.expr);
        let origin = self.expression_origin(&expression.expr);
        self.visit_expr(&expression.len);
        self.cache_expression_origin(expression, origin);
    }

    fn visit_expr_block(&mut self, expression: &'ast syn::ExprBlock) {
        for attribute in &expression.attrs {
            self.visit_attribute(attribute);
        }
        let label = expression
            .label
            .as_ref()
            .map(|label| label.name.ident.to_string());
        if let Some(label) = &expression.label {
            self.visit_label(label);
        }
        if label.is_some() {
            self.break_origins.push(BreakOriginState {
                kind: BreakTargetKind::Block,
                label,
                binding_scope_depth: self.binding_scopes.len(),
                origins: Vec::new(),
                environments: Vec::new(),
                continue_environments: Vec::new(),
            });
        }
        let entry_environment = self.snapshot_environment();
        let mut origin = self.visit_scoped_block(&expression.block);
        if expression.label.is_some() {
            let tail_environment = self.snapshot_environment();
            let breaks = self
                .break_origins
                .pop()
                .expect("labeled block break origin is balanced");
            origin = merge_origins(std::iter::once(origin).chain(breaks.origins));
            let mut continuing_environments = Vec::new();
            if block_may_fall_through(&expression.block) {
                continuing_environments.push(tail_environment);
            }
            continuing_environments.extend(breaks.environments);
            if continuing_environments.is_empty() {
                continuing_environments.push(entry_environment);
            }
            self.restore_environment(Self::join_environments(continuing_environments));
        }
        self.cache_expression_origin(expression, origin);
    }

    fn visit_expr_async(&mut self, expression: &'ast syn::ExprAsync) {
        for attribute in &expression.attrs {
            self.visit_attribute(attribute);
        }
        let environment = self.snapshot_environment();
        let outer_break_origins = std::mem::take(&mut self.break_origins);
        self.return_origins.push(Vec::new());
        let tail_origin = self.visit_scoped_block(&expression.block);
        let origin = self.finish_return_context(tail_origin);
        self.restore_environment(environment);
        let deferred_break_origins =
            std::mem::replace(&mut self.break_origins, outer_break_origins);
        debug_assert!(deferred_break_origins.is_empty());
        self.cache_expression_origin(expression, origin);
    }

    fn visit_expr_const(&mut self, expression: &'ast syn::ExprConst) {
        self.visit_value_block(expression, &expression.attrs, &expression.block);
    }

    fn visit_expr_try_block(&mut self, expression: &'ast syn::ExprTryBlock) {
        self.visit_value_block(expression, &expression.attrs, &expression.block);
    }

    fn visit_expr_unsafe(&mut self, expression: &'ast syn::ExprUnsafe) {
        self.visit_value_block(expression, &expression.attrs, &expression.block);
    }

    fn visit_expr_loop(&mut self, expression: &'ast syn::ExprLoop) {
        for attribute in &expression.attrs {
            self.visit_attribute(attribute);
        }
        let label = expression
            .label
            .as_ref()
            .map(|label| label.name.ident.to_string());
        if let Some(label) = &expression.label {
            self.visit_label(label);
        }
        let entry_environment = self.snapshot_environment();
        let mut carried_environment = entry_environment.clone();
        let mut break_environments = Vec::new();
        let mut break_origins = Vec::new();
        loop {
            self.restore_environment(carried_environment.clone());
            self.break_origins.push(BreakOriginState {
                kind: BreakTargetKind::Loop,
                label: label.clone(),
                binding_scope_depth: self.binding_scopes.len(),
                origins: Vec::new(),
                environments: Vec::new(),
                continue_environments: Vec::new(),
            });
            self.visit_scoped_block(&expression.body);
            let fallthrough_environment = self.snapshot_environment();
            let target = self
                .break_origins
                .pop()
                .expect("loop break origin is balanced");
            break_environments.extend(target.environments);
            break_origins.extend(target.origins);

            let mut next_environments =
                vec![entry_environment.clone(), carried_environment.clone()];
            if block_may_fall_through(&expression.body) {
                next_environments.push(fallthrough_environment);
            }
            next_environments.extend(target.continue_environments);
            let next_environment = Self::join_environments(next_environments);
            if next_environment == carried_environment {
                break;
            }
            carried_environment = next_environment;
        }
        if break_environments.is_empty() {
            self.restore_environment(entry_environment);
        } else {
            self.restore_environment(Self::join_environments(break_environments));
        }
        self.cache_expression_origin(expression, merge_origins(break_origins));
    }

    fn visit_expr_break(&mut self, expression: &'ast syn::ExprBreak) {
        visit::visit_expr_break(self, expression);
        let origin = expression
            .expr
            .as_ref()
            .map(|value| self.expression_origin(value))
            .unwrap_or(ValueOrigin::Other);
        let target_index = if let Some(label) = &expression.label {
            let label = label.ident.to_string();
            self.break_origins
                .iter()
                .rposition(|target| target.label.as_deref() == Some(label.as_str()))
        } else {
            self.break_origins
                .iter()
                .rposition(|target| target.kind.is_loop())
        };
        if let Some(target_index) = target_index {
            let environment = self.snapshot_environment_at_scope_depth(
                self.break_origins[target_index].binding_scope_depth,
            );
            let target = &mut self.break_origins[target_index];
            target.origins.push(origin);
            target.environments.push(environment);
        }
    }

    fn visit_expr_continue(&mut self, expression: &'ast syn::ExprContinue) {
        visit::visit_expr_continue(self, expression);
        let target_index = if let Some(label) = &expression.label {
            let label = label.ident.to_string();
            self.break_origins.iter().rposition(|target| {
                target.kind.is_loop() && target.label.as_deref() == Some(label.as_str())
            })
        } else {
            self.break_origins
                .iter()
                .rposition(|target| target.kind.is_loop())
        };
        if let Some(target_index) = target_index {
            let environment = self.snapshot_environment_at_scope_depth(
                self.break_origins[target_index].binding_scope_depth,
            );
            self.break_origins[target_index]
                .continue_environments
                .push(environment);
        }
    }

    fn visit_expr_return(&mut self, expression: &'ast syn::ExprReturn) {
        visit::visit_expr_return(self, expression);
        let Some(value) = &expression.expr else {
            return;
        };
        let origin = self.expression_origin(value);
        if let Some(returns) = self.return_origins.last_mut() {
            returns.push(origin);
        }
    }

    fn visit_expr_for_loop(&mut self, expression: &'ast syn::ExprForLoop) {
        for attribute in &expression.attrs {
            self.visit_attribute(attribute);
        }
        let label = expression
            .label
            .as_ref()
            .map(|label| label.name.ident.to_string());
        if let Some(label) = &expression.label {
            self.visit_label(label);
        }
        self.visit_expr(&expression.expr);
        let skipped_body_environment = self.snapshot_environment();
        self.break_origins.push(BreakOriginState {
            kind: BreakTargetKind::For,
            label,
            binding_scope_depth: self.binding_scopes.len(),
            origins: Vec::new(),
            environments: Vec::new(),
            continue_environments: Vec::new(),
        });
        self.push_binding_scope();
        self.bind_pattern_from_expression(&expression.pat, &expression.expr);
        self.visit_block(&expression.body);
        self.pop_binding_scope();
        let evaluated_body_environment = self.snapshot_environment();
        let target = self
            .break_origins
            .pop()
            .expect("for-loop control target is balanced");
        let mut continuing_environments = vec![skipped_body_environment];
        if block_may_fall_through(&expression.body) {
            continuing_environments.push(evaluated_body_environment);
        }
        continuing_environments.extend(target.environments);
        continuing_environments.extend(target.continue_environments);
        self.restore_environment(Self::join_environments(continuing_environments));
    }

    fn visit_expr_match(&mut self, expression: &'ast syn::ExprMatch) {
        for attribute in &expression.attrs {
            self.visit_attribute(attribute);
        }
        self.visit_expr(&expression.expr);
        let scrutinee_identity = self.expression_type_identity(&expression.expr);
        let scrutinee_origin = self.expression_origin(&expression.expr);
        let scrutinee_callable_origin = self.callable_result_origin(&expression.expr);
        let post_scrutinee_environment = self.snapshot_environment();
        let mut origins = Vec::with_capacity(expression.arms.len());
        let mut continuing_environments = Vec::new();
        let mut remaining_environment = Some(post_scrutinee_environment.clone());
        for arm in &expression.arms {
            let Some(arm_entry_environment) = remaining_environment.take() else {
                break;
            };
            self.restore_environment(arm_entry_environment.clone());
            let pattern_scope_depth = self.binding_scopes.len();
            self.push_binding_scope();
            self.bind_pattern_with_evidence(
                &arm.pat,
                scrutinee_identity.clone(),
                scrutinee_origin,
                scrutinee_callable_origin,
            );
            let guard_environment =
                if let Some((_, guard)) = &arm.guard {
                    self.visit_expr(guard);
                    Some(self.environment_at_scope_depth(
                        self.snapshot_environment(),
                        pattern_scope_depth,
                    ))
                } else {
                    None
                };
            let guard_continues = match &arm.guard {
                Some((_, guard)) => expression_may_fall_through(guard),
                None => true,
            };
            if guard_continues {
                self.visit_expr(&arm.body);
                origins.push(self.expression_origin(&arm.body));
            } else {
                origins.push(ValueOrigin::Other);
            }
            self.pop_binding_scope();
            if guard_continues && expression_may_fall_through(&arm.body) {
                continuing_environments.push(self.snapshot_environment());
            }
            let pattern_may_miss = !pattern_is_obviously_irrefutable(&arm.pat);
            let mut next_arm_environments = pattern_may_miss
                .then_some(arm_entry_environment)
                .into_iter()
                .collect::<Vec<_>>();
            if guard_continues {
                if let Some(guard_environment) = guard_environment {
                    next_arm_environments.push(guard_environment);
                }
            }
            if !next_arm_environments.is_empty() {
                remaining_environment = Some(Self::join_environments(next_arm_environments));
            }
        }
        continuing_environments.extend(remaining_environment);
        if continuing_environments.is_empty() {
            continuing_environments.push(post_scrutinee_environment);
        }
        self.restore_environment(Self::join_environments(continuing_environments));
        self.cache_expression_origin(expression, merge_origins(origins));
    }

    fn visit_expr_if(&mut self, expression: &'ast syn::ExprIf) {
        for attribute in &expression.attrs {
            self.visit_attribute(attribute);
        }
        let let_condition = match unwrap_expression(&expression.cond) {
            syn::Expr::Let(condition) => Some(condition),
            _ => None,
        };
        if let Some(condition) = let_condition {
            self.visit_expr(&condition.expr);
        } else {
            self.visit_expr(&expression.cond);
        }
        let post_condition_environment = self.snapshot_environment();

        self.restore_environment(post_condition_environment.clone());
        let then_origin = if let Some(condition) = let_condition {
            self.push_binding_scope();
            self.bind_pattern_from_expression(&condition.pat, &condition.expr);
            let origin = self.visit_scoped_block(&expression.then_branch);
            self.pop_binding_scope();
            origin
        } else {
            self.visit_scoped_block(&expression.then_branch)
        };
        let then_environment = self.snapshot_environment();

        self.restore_environment(post_condition_environment.clone());
        let (alternative_origin, alternative_environment) =
            if let Some((_, alternative)) = &expression.else_branch {
                self.visit_expr(alternative);
                (
                    self.expression_origin(alternative),
                    self.snapshot_environment(),
                )
            } else {
                (ValueOrigin::Other, self.snapshot_environment())
            };
        let mut continuing_environments = Vec::with_capacity(2);
        if block_may_fall_through(&expression.then_branch) {
            continuing_environments.push(then_environment);
        }
        let alternative_continues = match &expression.else_branch {
            Some((_, alternative)) => expression_may_fall_through(alternative),
            None => true,
        };
        if alternative_continues {
            continuing_environments.push(alternative_environment);
        }
        if continuing_environments.is_empty() {
            continuing_environments.push(post_condition_environment);
        }
        self.restore_environment(Self::join_environments(continuing_environments));
        self.cache_expression_origin(expression, merge_origins([then_origin, alternative_origin]));
    }

    fn visit_expr_while(&mut self, expression: &'ast syn::ExprWhile) {
        for attribute in &expression.attrs {
            self.visit_attribute(attribute);
        }
        let label = expression
            .label
            .as_ref()
            .map(|label| label.name.ident.to_string());
        if let Some(label) = &expression.label {
            self.visit_label(label);
        }
        let let_condition = match unwrap_expression(&expression.cond) {
            syn::Expr::Let(condition) => Some(condition),
            _ => None,
        };
        if let Some(condition) = let_condition {
            self.visit_expr(&condition.expr);
        } else {
            self.visit_expr(&expression.cond);
        }
        let skipped_body_environment = self.snapshot_environment();
        self.break_origins.push(BreakOriginState {
            kind: BreakTargetKind::While,
            label,
            binding_scope_depth: self.binding_scopes.len(),
            origins: Vec::new(),
            environments: Vec::new(),
            continue_environments: Vec::new(),
        });
        if let Some(condition) = let_condition {
            self.push_binding_scope();
            self.bind_pattern_from_expression(&condition.pat, &condition.expr);
        }
        self.visit_block(&expression.body);
        if let_condition.is_some() {
            self.pop_binding_scope();
        }
        let evaluated_body_environment = self.snapshot_environment();
        let target = self
            .break_origins
            .pop()
            .expect("while-loop control target is balanced");
        let mut continuing_environments = vec![skipped_body_environment];
        if block_may_fall_through(&expression.body) {
            continuing_environments.push(evaluated_body_environment);
        }
        continuing_environments.extend(target.environments);
        continuing_environments.extend(target.continue_environments);
        self.restore_environment(Self::join_environments(continuing_environments));
    }

    fn visit_expr_closure(&mut self, closure: &'ast syn::ExprClosure) {
        let environment = self.snapshot_environment();
        let outer_break_origins = std::mem::take(&mut self.break_origins);
        self.return_origins.push(Vec::new());
        self.push_binding_scope();
        for input in &closure.inputs {
            let bindings = pattern_binding_names(input);
            let paths = match input {
                syn::Pat::Type(pattern) => type_paths(&pattern.ty),
                _ => Vec::new(),
            };
            let identity =
                receiver_type_identity(self.env.module, &paths, self.env.aliases, self.env.imports);
            let origin =
                origin_for_type(self.env.module, &paths, self.env.aliases, self.env.imports);
            for binding in bindings {
                self.record_binding_declaration(&binding);
                self.bindings.insert(binding.clone());
                self.callable_paths.remove(&binding);
                self.callable_origins.remove(&binding);
                if let Some(identity) = &identity {
                    self.receiver_types
                        .insert(binding.clone(), identity.clone());
                } else {
                    self.receiver_types.remove(&binding);
                }
                self.origins.insert(binding, origin);
            }
        }
        visit::visit_expr_closure(self, closure);
        let tail_origin = self.expression_origin(&closure.body);
        let origin = self.finish_return_context(tail_origin);
        self.pop_binding_scope();
        self.restore_environment(environment);
        let deferred_break_origins =
            std::mem::replace(&mut self.break_origins, outer_break_origins);
        debug_assert!(deferred_break_origins.is_empty());
        self.cache_expression_origin(closure, origin);
    }

    fn visit_expr_call(&mut self, call: &'ast ExprCall) {
        // Resolve the callable before arguments can mutate alias state, then
        // snapshot each argument's origin at its left-to-right evaluation
        // point instead of re-reading every argument from the final state.
        let site = self.call_site(call);
        for attribute in &call.attrs {
            self.visit_attribute(attribute);
        }
        self.visit_expr(&call.func);
        let callable_origin = self.callable_result_origin(&call.func);
        let mut argument_origins = Vec::with_capacity(call.args.len());
        for argument in &call.args {
            self.visit_expr(argument);
            argument_origins.push(self.expression_origin(argument));
        }
        let protected_model_flow = argument_origins.contains(&ValueOrigin::ModelText);
        let result_origin = if protected_model_flow
            || self.call_returns_model_text(&site)
            || callable_origin == ValueOrigin::ModelText
        {
            ValueOrigin::ModelText
        } else {
            ValueOrigin::Other
        };
        self.cache_expression_origin(call, result_origin);
        if protected_model_flow
            && site.indirect
            && resolve_call(
                self.caller,
                &site,
                self.summaries,
                self.env.imports,
                self.env.aliases,
            )
            .is_empty()
        {
            self.unresolved_model_dispatch = true;
        }
    }

    fn visit_expr_method_call(&mut self, call: &'ast ExprMethodCall) {
        // As with free calls, preserve the pre-argument receiver identity and
        // retain each value's origin at its actual evaluation point.
        let site = self.method_call_site(call);
        for attribute in &call.attrs {
            self.visit_attribute(attribute);
        }
        self.visit_expr(&call.receiver);
        let receiver_origin = self.expression_origin(&call.receiver);
        if let Some(turbofish) = &call.turbofish {
            self.visit_angle_bracketed_generic_arguments(turbofish);
        }
        let mut argument_origins = Vec::with_capacity(call.args.len());
        for argument in &call.args {
            self.visit_expr(argument);
            argument_origins.push(self.expression_origin(argument));
        }
        let protected_model_flow = argument_origins.contains(&ValueOrigin::ModelText);
        let result_origin = if (receiver_origin == ValueOrigin::Model
            && is_model_text_method(&call.method.to_string()))
            || receiver_origin == ValueOrigin::ModelText
            || protected_model_flow
            || self.call_returns_model_text(&site)
        {
            ValueOrigin::ModelText
        } else {
            ValueOrigin::Other
        };
        self.cache_expression_origin(call, result_origin);
        if protected_model_flow
            && resolve_call(
                self.caller,
                &site,
                self.summaries,
                self.env.imports,
                self.env.aliases,
            )
            .is_empty()
            && !is_harmless_standard_collection_dispatch(
                &site,
                &self.env,
                &self.caller.local_standard_roots,
            )
        {
            self.unresolved_model_dispatch = true;
        }
    }

    fn visit_local(&mut self, local: &'ast Local) {
        let bindings = pattern_binding_names(&local.pat);
        let explicit_paths = match &local.pat {
            syn::Pat::Type(pattern) => type_paths(&pattern.ty),
            _ => Vec::new(),
        };

        // Initializer and `let-else` names resolve against the previous scope.
        // Only the successful pattern path continues past the declaration, so
        // analyze the diverging branch for violations but restore the
        // post-initializer environment before installing the new bindings.
        for attribute in &local.attrs {
            self.visit_attribute(attribute);
        }
        if let Some(init) = &local.init {
            self.visit_expr(&init.expr);
        }

        let explicit = receiver_type_identity(
            self.env.module,
            &explicit_paths,
            self.env.aliases,
            self.env.imports,
        );
        let inferred = local
            .init
            .as_ref()
            .and_then(|init| self.expression_type_identity(&init.expr));
        let identity = explicit.or(inferred);
        let origin = (!bindings.is_empty()).then(|| {
            local
                .init
                .as_ref()
                .map(|init| self.expression_origin(&init.expr))
                .filter(|origin| *origin != ValueOrigin::Other)
                .unwrap_or_else(|| {
                    origin_for_type(
                        self.env.module,
                        &explicit_paths,
                        self.env.aliases,
                        self.env.imports,
                    )
                })
        });
        let callable_path = if let ([binding], Some(init)) = (bindings.as_slice(), &local.init) {
            if let syn::Expr::Path(path) = &*init.expr {
                Some((
                    binding,
                    path.path
                        .segments
                        .iter()
                        .map(|segment| segment.ident.to_string())
                        .collect(),
                ))
            } else {
                None
            }
        } else {
            None
        };
        let callable_origin = if let ([_], Some(init)) = (bindings.as_slice(), &local.init) {
            self.callable_result_origin(&init.expr)
        } else {
            ValueOrigin::Other
        };
        if let Some(init) = &local.init {
            if let Some((_, diverge)) = &init.diverge {
                let continuing_environment = self.snapshot_environment();
                self.visit_expr(diverge);
                self.restore_environment(continuing_environment);
            }
        }

        for binding in &bindings {
            self.record_binding_declaration(binding);
            self.receiver_types.remove(binding);
            self.callable_paths.remove(binding);
            self.callable_origins.remove(binding);
            self.origins.remove(binding);
            self.bindings.insert(binding.clone());
        }
        if let Some(identity) = identity {
            for binding in &bindings {
                self.receiver_types
                    .insert(binding.clone(), identity.clone());
            }
        }
        if let Some(origin) = origin {
            for binding in &bindings {
                self.origins.insert(binding.clone(), origin);
            }
        }
        if let Some((binding, path)) = callable_path {
            self.callable_paths.insert(binding.clone(), path);
        }
        if callable_origin != ValueOrigin::Other {
            self.callable_origins
                .insert(bindings[0].clone(), callable_origin);
        }
    }

    fn visit_expr_assign(&mut self, assignment: &'ast syn::ExprAssign) {
        visit::visit_expr_assign(self, assignment);

        let assigned_bindings = assignment_local_binding_names(&assignment.left, &self.bindings);
        let identity = self.expression_type_identity(&assignment.right);
        let origin = self.expression_origin(&assignment.right);
        let callable_origin = self.callable_result_origin(&assignment.right);
        let callable_path = if assigned_bindings.len() == 1 {
            match unwrap_expression(&assignment.right) {
                syn::Expr::Path(path) => Some(
                    path.path
                        .segments
                        .iter()
                        .map(|segment| segment.ident.to_string())
                        .collect::<Vec<_>>(),
                ),
                _ => None,
            }
        } else {
            None
        };
        for binding in assigned_bindings {
            if let Some(identity) = &identity {
                self.receiver_types
                    .insert(binding.clone(), identity.clone());
            } else {
                self.receiver_types.remove(&binding);
            }
            if let Some(callable_path) = &callable_path {
                self.callable_paths
                    .insert(binding.clone(), callable_path.clone());
            } else {
                self.callable_paths.remove(&binding);
            }
            if callable_origin == ValueOrigin::Other {
                self.callable_origins.remove(&binding);
            } else {
                self.callable_origins
                    .insert(binding.clone(), callable_origin);
            }
            self.origins.insert(binding, origin);
        }
    }
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
        UseTree::Glob(_) => {
            out.insert("*".into(), prefix);
        }
    }
}

fn signature_is_resolved_raw_grammar_entry(
    signature: &Signature,
    module: &ModuleIdentity,
    aliases: &Aliases,
    imports: &Imports,
) -> bool {
    let inputs = parameter_summaries(&signature.inputs);
    let output_paths = match &signature.output {
        ReturnType::Default => Vec::new(),
        ReturnType::Type(_, output) => type_paths(output),
    };
    resolved_raw_grammar_shape(module, &inputs, &output_paths, &[], &[], aliases, imports)
}

fn parameter_summaries(
    inputs: &syn::punctuated::Punctuated<FnArg, syn::Token![,]>,
) -> Vec<ParameterSummary> {
    inputs
        .iter()
        .filter_map(|argument| match argument {
            FnArg::Receiver(_) => None,
            FnArg::Typed(argument) => Some(ParameterSummary {
                name: match &*argument.pat {
                    syn::Pat::Ident(pattern) => Some(pattern.ident.to_string()),
                    syn::Pat::Type(pattern) => match &*pattern.pat {
                        syn::Pat::Ident(pattern) => Some(pattern.ident.to_string()),
                        _ => None,
                    },
                    _ => None,
                },
                paths: type_paths(&argument.ty),
                mutable: type_is_mutable_reference(&argument.ty),
            }),
        })
        .collect()
}

fn resolved_raw_grammar_shape(
    module: &ModuleIdentity,
    inputs: &[ParameterSummary],
    output_paths: &[Vec<String>],
    body_paths: &[Vec<String>],
    body_type_uses: &[TypeUse],
    aliases: &Aliases,
    imports: &Imports,
) -> bool {
    let raw_input = inputs.iter().any(|input| {
        resolved_type_names(module, &input.paths, aliases, imports)
            .iter()
            .any(|name| matches!(name.as_str(), "str" | "String" | "SourceText"))
    });
    if !raw_input {
        return false;
    }

    let output_names = resolved_type_names(module, output_paths, aliases, imports);
    let grammar_output = output_names
        .iter()
        .any(|name| HIGH_LEVEL_GRAMMAR_TYPES.contains(&name.as_str()));
    let output_parameter = inputs.iter().any(|input| {
        let names = resolved_type_names(module, &input.paths, aliases, imports);
        let named_output = input.name.as_ref().is_some_and(|name| {
            matches!(
                name.as_str(),
                "builder" | "factory" | "out" | "output" | "sink" | "target"
            )
        });
        (input.mutable || named_output)
            && (is_typed_uml_tree(&names) || is_typed_uml_tree_builder(&names))
    });
    let body_names = resolved_body_type_names(module, body_paths, body_type_uses, aliases, imports);
    let constructs_protected =
        is_typed_uml_tree(&body_names) || is_typed_uml_tree_builder(&body_names);

    grammar_output || is_typed_uml_tree(&output_names) || output_parameter || constructs_protected
}

fn resolved_body_type_names(
    module: &ModuleIdentity,
    body_paths: &[Vec<String>],
    body_type_uses: &[TypeUse],
    aliases: &Aliases,
    imports: &Imports,
) -> BTreeSet<String> {
    let mut names = resolved_type_names(module, body_paths, aliases, imports);
    for ty in body_type_uses {
        names.extend(resolved_type_names(&ty.module, &ty.paths, aliases, imports));
    }
    names
}

fn closure_is_raw_grammar_entry(
    closure: &syn::ExprClosure,
    module: &ModuleIdentity,
    aliases: &Aliases,
    imports: &Imports,
) -> bool {
    let inputs = closure
        .inputs
        .iter()
        .filter_map(|pattern| match pattern {
            syn::Pat::Type(pattern) => Some(ParameterSummary {
                name: match &*pattern.pat {
                    syn::Pat::Ident(pattern) => Some(pattern.ident.to_string()),
                    _ => None,
                },
                paths: type_paths(&pattern.ty),
                mutable: type_is_mutable_reference(&pattern.ty),
            }),
            _ => None,
        })
        .collect::<Vec<_>>();
    let output_paths = match &closure.output {
        ReturnType::Default => Vec::new(),
        ReturnType::Type(_, output) => type_paths(output),
    };
    #[derive(Default)]
    struct ExpressionPaths {
        paths: Vec<Vec<String>>,
    }
    impl<'ast> Visit<'ast> for ExpressionPaths {
        fn visit_expr_call(&mut self, call: &'ast ExprCall) {
            if let syn::Expr::Path(function) = unwrap_expression(&call.func) {
                self.paths.extend(paths_in_path(&function.path));
            }
            visit::visit_expr_call(self, call);
        }

        fn visit_expr_struct(&mut self, expression: &'ast syn::ExprStruct) {
            self.paths.extend(paths_in_path(&expression.path));
            visit::visit_expr_struct(self, expression);
        }
    }
    let mut body = ExpressionPaths::default();
    body.visit_expr(&closure.body);

    resolved_raw_grammar_shape(
        module,
        &inputs,
        &output_paths,
        &body.paths,
        &[],
        aliases,
        imports,
    )
}

fn summary_is_raw_grammar_entry(
    summary: &FunctionSummary,
    aliases: &Aliases,
    imports: &Imports,
) -> bool {
    let input_names = summary
        .input_types
        .iter()
        .flat_map(|input| resolved_type_names(&summary.id.module, &input.paths, aliases, imports))
        .collect::<BTreeSet<_>>();
    let output_names =
        resolved_type_names(&summary.id.module, &summary.output_paths, aliases, imports);
    let body_names = resolved_body_type_names(
        &summary.id.module,
        &summary.body_paths,
        &summary.body_type_uses,
        aliases,
        imports,
    );
    let exact_cached_tree_owner = summary.owner_type.as_ref().is_some_and(|owner| {
        owner.name == "UmlLoweringState"
            && owner.module == summary.id.module
            && owner.module.display() == "waml::uml::lower"
    });
    let cached_tree_lookup = summary.path == "crates/waml/src/uml/lower.rs"
        && exact_cached_tree_owner
        && summary.id.name == "tree"
        && summary.has_receiver
        && input_names.contains("SourceBundle")
        && !input_names.contains("SourceText")
        && is_typed_uml_tree(&output_names)
        && !is_typed_uml_tree(&body_names)
        && !is_typed_uml_tree_builder(&body_names)
        && cached_tree_body_is_verified(summary);
    if cached_tree_lookup {
        return false;
    }
    resolved_raw_grammar_shape(
        &summary.id.module,
        &summary.input_types,
        &summary.output_paths,
        &summary.body_paths,
        &summary.body_type_uses,
        aliases,
        imports,
    )
}

fn cached_tree_body_is_verified(summary: &FunctionSummary) -> bool {
    struct CacheTreeCalls {
        valid: bool,
        saw_reparse: bool,
        saw_contains_key: bool,
        saw_get: bool,
    }

    impl<'ast> Visit<'ast> for CacheTreeCalls {
        fn visit_expr_method_call(&mut self, call: &'ast ExprMethodCall) {
            let name = call.method.to_string();
            let valid = match name.as_str() {
                "path" => {
                    cache_tree_is_binding(&call.receiver, "self")
                        && cache_tree_args_are(&call.args, &["target"])
                }
                "cloned" => call.args.is_empty() && cache_tree_is_path_lookup(&call.receiver),
                "ok_or_else" => {
                    call.args.len() == 1
                        && cache_tree_is_cloned_lookup(&call.receiver)
                        && matches!(
                            call.args.first().map(unwrap_expression),
                            Some(syn::Expr::Closure(_))
                        )
                }
                "contains_key" => {
                    cache_tree_is_touched_islands(&call.receiver)
                        && cache_tree_args_are(&call.args, &["path"])
                }
                "reparse" => {
                    cache_tree_is_binding(&call.receiver, "self")
                        && cache_tree_args_are(&call.args, &["candidate", "path", "op"])
                }
                "get" => {
                    cache_tree_is_touched_islands(&call.receiver)
                        && cache_tree_args_are(&call.args, &["path"])
                }
                "expect" => {
                    call.args.len() == 1
                        && cache_tree_is_get_lookup(&call.receiver)
                        && call.args.first().is_some_and(cache_tree_is_string_literal)
                }
                "clone" => {
                    call.args.is_empty()
                        && (cache_tree_is_binding(&call.receiver, "path")
                            || cache_tree_is_expect_lookup(&call.receiver))
                }
                _ => false,
            };
            self.valid &= valid;
            self.saw_reparse |= valid && name == "reparse";
            self.saw_contains_key |= valid && name == "contains_key";
            self.saw_get |= valid && name == "get";
            visit::visit_expr_method_call(self, call);
        }

        fn visit_expr_call(&mut self, call: &'ast ExprCall) {
            self.valid &= cache_tree_function_call_is_verified(call);
            visit::visit_expr_call(self, call);
        }
    }

    let mut calls = CacheTreeCalls {
        valid: true,
        saw_reparse: false,
        saw_contains_key: false,
        saw_get: false,
    };
    calls.visit_block(&summary.block);

    calls.valid
        && calls.saw_reparse
        && calls.saw_contains_key
        && calls.saw_get
        && cache_tree_body_has_expected_shape(&summary.block)
        && cache_tree_tail_reads_touched_islands(&summary.block)
        && cache_tree_body_has_no_macros(&summary.block)
}

fn cache_tree_body_has_expected_shape(block: &syn::Block) -> bool {
    let [syn::Stmt::Local(path), syn::Stmt::Expr(reparse, None), syn::Stmt::Expr(result, None)] =
        block.stmts.as_slice()
    else {
        return false;
    };
    cache_tree_path_binding_is_verified(path)
        && cache_tree_reparse_branch_is_verified(reparse)
        && cache_tree_ok_result_is_verified(result)
}

fn cache_tree_path_binding_is_verified(local: &syn::Local) -> bool {
    let syn::Pat::Ident(pattern) = &local.pat else {
        return false;
    };
    if pattern.ident != "path"
        || pattern.by_ref.is_some()
        || pattern.mutability.is_some()
        || pattern.subpat.is_some()
    {
        return false;
    }
    let Some(init) = &local.init else {
        return false;
    };
    if init.diverge.is_some() {
        return false;
    }
    let syn::Expr::Try(try_expression) = unwrap_expression(&init.expr) else {
        return false;
    };
    let Some(ok_or_else) = cache_tree_method_call(&try_expression.expr, "ok_or_else") else {
        return false;
    };
    let Some(closure) = ok_or_else.args.first().and_then(|argument| {
        let syn::Expr::Closure(closure) = unwrap_expression(argument) else {
            return None;
        };
        Some(closure)
    }) else {
        return false;
    };
    ok_or_else.args.len() == 1
        && cache_tree_is_cloned_lookup(&ok_or_else.receiver)
        && closure.inputs.is_empty()
        && cache_tree_error_call_is_verified(&closure.body)
}

fn cache_tree_error_call_is_verified(expression: &syn::Expr) -> bool {
    let syn::Expr::Call(call) = unwrap_expression(expression) else {
        return false;
    };
    cache_tree_call_path_is(call, &["EditError", "at"])
        && cache_tree_function_call_is_verified(call)
}

fn cache_tree_reparse_branch_is_verified(expression: &syn::Expr) -> bool {
    let syn::Expr::If(branch) = unwrap_expression(expression) else {
        return false;
    };
    if branch.else_branch.is_some() {
        return false;
    }
    let syn::Expr::Unary(condition) = unwrap_expression(&branch.cond) else {
        return false;
    };
    if !matches!(condition.op, syn::UnOp::Not(_)) {
        return false;
    }
    let Some(contains_key) = cache_tree_method_call(&condition.expr, "contains_key") else {
        return false;
    };
    let [syn::Stmt::Expr(reparse, Some(_))] = branch.then_branch.stmts.as_slice() else {
        return false;
    };
    let syn::Expr::Try(try_expression) = unwrap_expression(reparse) else {
        return false;
    };
    let Some(reparse) = cache_tree_method_call(&try_expression.expr, "reparse") else {
        return false;
    };
    cache_tree_is_touched_islands(&contains_key.receiver)
        && cache_tree_args_are(&contains_key.args, &["path"])
        && cache_tree_is_binding(&reparse.receiver, "self")
        && cache_tree_args_are(&reparse.args, &["candidate", "path", "op"])
}

fn cache_tree_ok_result_is_verified(expression: &syn::Expr) -> bool {
    let syn::Expr::Call(call) = unwrap_expression(expression) else {
        return false;
    };
    cache_tree_call_path_is(call, &["Ok"]) && cache_tree_function_call_is_verified(call)
}

fn cache_tree_call_path_is(call: &ExprCall, expected: &[&str]) -> bool {
    let syn::Expr::Path(function) = unwrap_expression(&call.func) else {
        return false;
    };
    function.path.segments.len() == expected.len()
        && function
            .path
            .segments
            .iter()
            .zip(expected)
            .all(|(segment, expected)| segment.ident == expected)
}

fn cache_tree_is_binding(expression: &syn::Expr, expected: &str) -> bool {
    let syn::Expr::Path(path) = unwrap_expression(expression) else {
        return false;
    };
    path.path.segments.len() == 1 && path.path.segments[0].ident == expected
}

fn cache_tree_args_are(
    arguments: &syn::punctuated::Punctuated<syn::Expr, syn::token::Comma>,
    expected: &[&str],
) -> bool {
    arguments.len() == expected.len()
        && arguments
            .iter()
            .zip(expected)
            .all(|(argument, expected)| cache_tree_is_binding(argument, expected))
}

fn cache_tree_is_touched_islands(expression: &syn::Expr) -> bool {
    let syn::Expr::Field(field) = unwrap_expression(expression) else {
        return false;
    };
    matches!(&field.member, syn::Member::Named(name) if name == "touched_islands")
        && cache_tree_is_binding(&field.base, "self")
}

fn cache_tree_method_call<'ast>(
    expression: &'ast syn::Expr,
    expected: &str,
) -> Option<&'ast ExprMethodCall> {
    let syn::Expr::MethodCall(call) = unwrap_expression(expression) else {
        return None;
    };
    (call.method == expected).then_some(call)
}

fn cache_tree_is_path_lookup(expression: &syn::Expr) -> bool {
    let Some(call) = cache_tree_method_call(expression, "path") else {
        return false;
    };
    cache_tree_is_binding(&call.receiver, "self") && cache_tree_args_are(&call.args, &["target"])
}

fn cache_tree_is_cloned_lookup(expression: &syn::Expr) -> bool {
    let Some(call) = cache_tree_method_call(expression, "cloned") else {
        return false;
    };
    call.args.is_empty() && cache_tree_is_path_lookup(&call.receiver)
}

fn cache_tree_is_get_lookup(expression: &syn::Expr) -> bool {
    let Some(call) = cache_tree_method_call(expression, "get") else {
        return false;
    };
    cache_tree_is_touched_islands(&call.receiver) && cache_tree_args_are(&call.args, &["path"])
}

fn cache_tree_is_expect_lookup(expression: &syn::Expr) -> bool {
    let Some(call) = cache_tree_method_call(expression, "expect") else {
        return false;
    };
    call.args.len() == 1
        && cache_tree_is_get_lookup(&call.receiver)
        && call.args.first().is_some_and(cache_tree_is_string_literal)
}

fn cache_tree_is_string_literal(expression: &syn::Expr) -> bool {
    matches!(
        unwrap_expression(expression),
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(_),
            ..
        })
    )
}

fn cache_tree_function_call_is_verified(call: &ExprCall) -> bool {
    let syn::Expr::Path(function) = unwrap_expression(&call.func) else {
        return false;
    };
    let segments = function
        .path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>();
    match segments.as_slice() {
        [ok] if ok == "Ok" => {
            let Some(argument) = call.args.first() else {
                return false;
            };
            let syn::Expr::Tuple(tuple) = unwrap_expression(argument) else {
                return false;
            };
            call.args.len() == 1
                && tuple.elems.len() == 2
                && tuple.elems.first().is_some_and(|path| {
                    cache_tree_method_call(path, "clone").is_some_and(|clone| {
                        clone.args.is_empty() && cache_tree_is_binding(&clone.receiver, "path")
                    })
                })
                && tuple.elems.get(1).is_some_and(|tree| {
                    cache_tree_method_call(tree, "clone").is_some_and(|clone| {
                        clone.args.is_empty() && cache_tree_is_expect_lookup(&clone.receiver)
                    })
                })
        }
        [error, at] if error == "EditError" && at == "at" => {
            call.args.len() == 2
                && call
                    .args
                    .first()
                    .is_some_and(|op| cache_tree_is_binding(op, "op"))
                && call.args.get(1).is_some_and(cache_tree_is_string_literal)
        }
        _ => false,
    }
}

fn cache_tree_tail_reads_touched_islands(block: &syn::Block) -> bool {
    let Some(syn::Stmt::Expr(tail, None)) = block.stmts.last() else {
        return false;
    };
    let syn::Expr::Call(ok) = unwrap_expression(tail) else {
        return false;
    };
    let syn::Expr::Path(function) = unwrap_expression(&ok.func) else {
        return false;
    };
    if function
        .path
        .segments
        .last()
        .map(|segment| segment.ident != "Ok")
        .unwrap_or(true)
    {
        return false;
    }
    let Some(argument) = ok.args.first() else {
        return false;
    };
    let syn::Expr::Tuple(tuple) = unwrap_expression(argument) else {
        return false;
    };
    let Some(tree) = tuple.elems.get(1) else {
        return false;
    };

    let mut expression = tree;
    let mut saw_get = false;
    loop {
        match unwrap_expression(expression) {
            syn::Expr::MethodCall(call) => {
                saw_get |= call.method == "get";
                expression = &call.receiver;
            }
            syn::Expr::Field(field) => {
                let syn::Member::Named(name) = &field.member else {
                    return false;
                };
                let syn::Expr::Path(base) = unwrap_expression(&field.base) else {
                    return false;
                };
                return saw_get
                    && name == "touched_islands"
                    && base.path.segments.len() == 1
                    && base.path.segments[0].ident == "self";
            }
            _ => return false,
        }
    }
}

fn cache_tree_body_has_no_macros(block: &syn::Block) -> bool {
    #[derive(Default)]
    struct MacroNames {
        names: BTreeSet<String>,
    }

    impl<'ast> Visit<'ast> for MacroNames {
        fn visit_macro(&mut self, item_macro: &'ast syn::Macro) {
            self.names.insert(macro_path(item_macro));
        }
    }

    let mut macros = MacroNames::default();
    macros.visit_block(block);
    macros.names.is_empty()
}

fn summary_has_model_input(
    summary: &FunctionSummary,
    aliases: &Aliases,
    imports: &Imports,
) -> bool {
    summary.model_input
        || summary.input_types.iter().any(|input| {
            resolved_type_names(&summary.id.module, &input.paths, aliases, imports)
                .iter()
                .any(|name| MODEL_TYPES.contains(&name.as_str()))
        })
        || (summary.has_receiver
            && resolved_type_names(&summary.id.module, &summary.owner_paths, aliases, imports)
                .iter()
                .any(|name| MODEL_TYPES.contains(&name.as_str())))
}

fn summary_is_model_to_text(
    summary: &FunctionSummary,
    aliases: &Aliases,
    imports: &Imports,
) -> bool {
    if !summary_has_model_input(summary, aliases, imports) {
        return false;
    }
    summary.output_type.as_ref().is_some_and(|output| {
        type_is_text_result(output)
            || (matches!(output, Type::Path(_))
                && resolved_type_names(&summary.id.module, &summary.output_paths, aliases, imports)
                    .iter()
                    .any(|name| matches!(name.as_str(), "str" | "String")))
    })
}

fn summary_exposes_visible_model_text(
    summary: &FunctionSummary,
    aliases: &Aliases,
    imports: &Imports,
) -> bool {
    if !summary.visible
        || !summary_is_model_to_text(summary, aliases, imports)
        || is_allowlisted_visible_model_text(summary)
    {
        return false;
    }
    if summary.has_receiver {
        return true;
    }
    const ROOT_MODEL_TYPES: &[&str] = &[
        "ClassDiagram",
        "DeclaredBundle",
        "Diagram",
        "DocumentDto",
        "FlowDoc",
        "Model",
        "Projection",
        "SequenceDoc",
        "UmlModel",
    ];
    summary.input_types.iter().any(|input| {
        resolved_type_names(&summary.id.module, &input.paths, aliases, imports)
            .iter()
            .any(|name| ROOT_MODEL_TYPES.contains(&name.as_str()))
    }) || matches!(
        summary.id.name.as_str(),
        "export"
            | "export_model"
            | "serialize"
            | "serialize_model"
            | "write_model"
            | "write_source"
    )
}

fn is_authorized_model_reparse(summary: &FunctionSummary) -> bool {
    summary.path == "crates/waml/src/uml/lower.rs" || summary.path == "crates/waml/src/okf/lower.rs"
}

fn is_typed_uml_tree(names: &BTreeSet<String>) -> bool {
    names.contains("SyntaxTree") && names.contains("UmlLanguage")
}

fn is_typed_uml_tree_builder(names: &BTreeSet<String>) -> bool {
    names.contains("UmlLanguage")
        && names.iter().any(|name| {
            name.ends_with("Builder")
                || name.ends_with("Factory")
                || matches!(name.as_str(), "GreenElement" | "GreenNode" | "GreenToken")
        })
}

fn resolved_type_names(
    context: &ModuleIdentity,
    paths: &[Vec<String>],
    aliases: &Aliases,
    imports: &Imports,
) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let mut pending = paths
        .iter()
        .cloned()
        .map(|path| (context.clone(), path))
        .collect::<Vec<_>>();
    let mut seen = BTreeSet::new();
    while let Some((path_context, path)) = pending.pop() {
        let Some(name) = path.last().cloned() else {
            continue;
        };
        names.insert(name.clone());
        let (module, alias) = resolve_type_path(&path_context, &path, imports);
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
    names
}

fn preferred_type_identity(
    context: &ModuleIdentity,
    paths: &[Vec<String>],
    aliases: &Aliases,
    imports: &Imports,
) -> Option<TypeIdentity> {
    let mut pending = paths
        .iter()
        .cloned()
        .map(|path| (context.clone(), path))
        .collect::<Vec<_>>();
    let mut seen = BTreeSet::new();
    let mut candidates = BTreeSet::new();
    while let Some((path_context, path)) = pending.pop() {
        let (module, name) = resolve_type_path(&path_context, &path, imports);
        if !seen.insert((module.clone(), name.clone())) {
            continue;
        }
        if let Some(targets) = aliases.get(&(module.clone(), name.clone())) {
            pending.extend(
                targets
                    .iter()
                    .cloned()
                    .map(|target| (module.clone(), target)),
            );
            continue;
        }
        candidates.insert(TypeIdentity::new(module, name));
    }
    if let Some(model) = candidates
        .iter()
        .find(|identity| MODEL_TYPES.contains(&identity.name.as_str()))
    {
        return Some(model.clone());
    }
    candidates.into_iter().find(|identity| {
        identity.name.chars().next().is_some_and(char::is_uppercase)
            && !matches!(
                identity.name.as_str(),
                "Analysis"
                    | "Arc"
                    | "Box"
                    | "Cow"
                    | "Error"
                    | "Option"
                    | "Result"
                    | "String"
                    | "Vec"
            )
    })
}

fn receiver_type_identity(
    context: &ModuleIdentity,
    paths: &[Vec<String>],
    aliases: &Aliases,
    imports: &Imports,
) -> Option<TypeIdentity> {
    fn resolve(
        context: &ModuleIdentity,
        paths: &[Vec<String>],
        aliases: &Aliases,
        imports: &Imports,
        seen: &mut BTreeSet<(ModuleIdentity, String)>,
    ) -> Option<TypeIdentity> {
        let path = paths.first()?;
        let (module, name) = resolve_type_path(context, path, imports);
        if !seen.insert((module.clone(), name.clone())) {
            return None;
        }
        if let Some(targets) = aliases.get(&(module.clone(), name.clone())) {
            return resolve(&module, targets, aliases, imports, seen);
        }

        let identity = TypeIdentity::new(module, name);
        if matches!(identity.name.as_str(), "Arc" | "Box" | "Cow" | "Pin" | "Rc") {
            return resolve(context, &paths[1..], aliases, imports, seen).or(Some(identity));
        }
        Some(identity)
    }

    resolve(context, paths, aliases, imports, &mut BTreeSet::new())
}

fn is_harmless_standard_collection_dispatch(
    call: &CallSite,
    env: &BodyEnvironment<'_>,
    local_standard_roots: &BTreeSet<String>,
) -> bool {
    let Some(receiver) = &call.receiver_type else {
        return false;
    };
    if env.struct_fields.contains_key(receiver) {
        return false;
    }

    let external_standard = matches!(
        receiver.module.crate_key.as_str(),
        "external:std" | "external:alloc"
    );
    let unqualified_prelude_vec = receiver.name == "Vec"
        && receiver.module == *env.module
        && !local_standard_roots.contains("Vec")
        && !env
            .imports
            .get(env.module)
            .is_some_and(|bindings| bindings.contains_key("*"));
    if !external_standard && !unqualified_prelude_vec {
        return false;
    }
    if external_standard {
        let external_root = receiver
            .module
            .crate_key
            .strip_prefix("external:")
            .unwrap_or_default();
        if local_standard_roots.contains(external_root) {
            return false;
        }
    }

    matches!(
        (
            receiver.name.as_str(),
            call.segments.last().map(String::as_str)
        ),
        ("Vec", Some("push")) | ("HashSet", Some("contains"))
    )
}

fn is_standard_to_string_trait(identity: &TypeIdentity) -> bool {
    identity.name == "ToString"
        && matches!(
            identity.module.crate_key.as_str(),
            "external:std" | "external:alloc"
        )
}

fn origin_for_type(
    context: &ModuleIdentity,
    paths: &[Vec<String>],
    aliases: &Aliases,
    imports: &Imports,
) -> ValueOrigin {
    let names = resolved_type_names(context, paths, aliases, imports);
    if names
        .iter()
        .any(|name| MODEL_TYPES.contains(&name.as_str()))
    {
        ValueOrigin::Model
    } else if names.contains("SourceText") {
        ValueOrigin::ProtectedSource
    } else {
        ValueOrigin::Other
    }
}

fn type_is_mutable_reference(ty: &Type) -> bool {
    match ty {
        Type::Group(group) => type_is_mutable_reference(&group.elem),
        Type::Paren(paren) => type_is_mutable_reference(&paren.elem),
        Type::Ptr(pointer) => pointer.mutability.is_some(),
        Type::Reference(reference) => reference.mutability.is_some(),
        _ => false,
    }
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
    let identifiers = macro_identifiers(&item_macro.mac);
    let label = item_macro
        .ident
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_else(|| macro_path(&item_macro.mac));
    let grammar_type = HIGH_LEVEL_GRAMMAR_TYPES
        .iter()
        .find(|name| identifiers.contains(**name));
    if let Some(grammar_type) = grammar_type {
        violations.push(Violation {
            path: path.to_owned(),
            reason: format!(
                "opaque macro `{label}` references grammar type `{grammar_type}` outside the syntax authority"
            ),
        });
    } else if identifiers.contains("SourceText")
        && identifiers.contains("SyntaxTree")
        && identifiers.contains("UmlLanguage")
    {
        violations.push(Violation {
            path: path.to_owned(),
            reason: format!(
                "opaque macro `{label}` can expand the protected `SourceText -> SyntaxTree<UmlLanguage>` authority"
            ),
        });
    }
}

struct AuthorityAttributeVisitor<'a> {
    path: &'a str,
    violations: &'a mut Vec<Violation>,
    local_macros: &'a BTreeSet<String>,
}

impl<'ast> Visit<'ast> for AuthorityAttributeVisitor<'_> {
    fn visit_attribute(&mut self, attribute: &'ast Attribute) {
        inspect_authority_attribute(self.path, attribute, self.violations);
        visit::visit_attribute(self, attribute);
    }

    fn visit_macro(&mut self, item_macro: &'ast syn::Macro) {
        let name = macro_path(item_macro);
        let local = item_macro.path.segments.len() == 1 && self.local_macros.contains(&name);
        if !local && name != "include" && name != "macro_rules" && !is_harmless_body_macro(&name) {
            self.violations.push(Violation {
                path: self.path.to_owned(),
                reason: format!(
                    "unapproved authority function-like proc macro `{name}` cannot be audited inside the protected parser boundary"
                ),
            });
        }
        visit::visit_macro(self, item_macro);
    }
}

fn local_macro_names(parsed: &syn::File) -> BTreeSet<String> {
    #[derive(Default)]
    struct Collector {
        names: BTreeSet<String>,
    }

    impl<'ast> Visit<'ast> for Collector {
        fn visit_item_macro(&mut self, item_macro: &'ast ItemMacro) {
            if item_macro.mac.path.is_ident("macro_rules") {
                if let Some(name) = &item_macro.ident {
                    self.names.insert(name.to_string());
                }
            }
            visit::visit_item_macro(self, item_macro);
        }
    }

    let mut collector = Collector::default();
    collector.visit_file(parsed);
    collector.names
}

fn inspect_authority_attribute(path: &str, attribute: &Attribute, violations: &mut Vec<Violation>) {
    let name = attribute
        .path()
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>()
        .join("::");
    if name == "derive" {
        let derives = attribute.parse_args_with(
            syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
        );
        match derives {
            Ok(derives) => {
                for derive in derives {
                    let derive_name = derive
                        .segments
                        .iter()
                        .map(|segment| segment.ident.to_string())
                        .collect::<Vec<_>>()
                        .join("::");
                    if !is_harmless_authority_derive(&derive_name) {
                        violations.push(Violation {
                            path: path.to_owned(),
                            reason: format!(
                                "unapproved authority derive/proc macro `{derive_name}` cannot be audited inside the protected parser boundary"
                            ),
                        });
                    }
                }
            }
            Err(error) => violations.push(Violation {
                path: path.to_owned(),
                reason: format!(
                    "authority derive list is not statically auditable inside the protected parser boundary: {error}"
                ),
            }),
        }
    } else if !is_harmless_authority_attribute(&name) {
        violations.push(Violation {
            path: path.to_owned(),
            reason: format!(
                "unapproved authority attribute/proc macro `{name}` cannot be audited inside the protected parser boundary"
            ),
        });
    }
}

fn is_harmless_authority_derive(name: &str) -> bool {
    matches!(
        name,
        "Clone" | "Copy" | "Debug" | "Default" | "Eq" | "Hash" | "Ord" | "PartialEq" | "PartialOrd"
    )
}

fn is_harmless_authority_attribute(name: &str) -> bool {
    matches!(
        name,
        "allow"
            | "cfg"
            | "cold"
            | "deny"
            | "deprecated"
            | "doc"
            | "forbid"
            | "inline"
            | "must_use"
            | "non_exhaustive"
            | "repr"
            | "test"
            | "warn"
    )
}

fn resolve_type_path(
    context: &ModuleIdentity,
    path: &[String],
    imports: &Imports,
) -> (ModuleIdentity, String) {
    let name = path.last().cloned().unwrap_or_default();
    if let Some(first) = path.first() {
        if let Some(imported) = imports
            .get(context)
            .and_then(|bindings| bindings.get(first))
        {
            let mut expanded = imported.clone();
            expanded.extend(path[1..].iter().cloned());
            if expanded != path {
                return resolve_type_path(context, &expanded, imports);
            }
        }
    }
    if path.len() == 1 {
        return (context.clone(), name);
    }
    let mut module = context.clone();
    let mut segments = path;
    match segments.first().map(String::as_str) {
        Some("crate") => {
            module.reset_to_crate_root();
            segments = &segments[1..];
        }
        Some("self") => segments = &segments[1..],
        Some("super") => {
            module.pop_module();
            segments = &segments[1..];
        }
        Some(external @ ("std" | "alloc" | "core")) => {
            module =
                ModuleIdentity::new(format!("external:{external}"), vec![external.to_string()]);
            segments = &segments[1..];
        }
        _ => module.reset_to_crate_root(),
    }
    module.extend(segments[..segments.len().saturating_sub(1)].iter().cloned());
    (module, name)
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

fn item_attributes(item: &Item) -> &[Attribute] {
    match item {
        Item::Const(item) => &item.attrs,
        Item::Enum(item) => &item.attrs,
        Item::ExternCrate(item) => &item.attrs,
        Item::Fn(item) => &item.attrs,
        Item::ForeignMod(item) => &item.attrs,
        Item::Impl(item) => &item.attrs,
        Item::Macro(item) => &item.attrs,
        Item::Mod(item) => &item.attrs,
        Item::Static(item) => &item.attrs,
        Item::Struct(item) => &item.attrs,
        Item::Trait(item) => &item.attrs,
        Item::TraitAlias(item) => &item.attrs,
        Item::Type(item) => &item.attrs,
        Item::Union(item) => &item.attrs,
        Item::Use(item) => &item.attrs,
        Item::Verbatim(_) => &[],
        _ => &[],
    }
}

fn module_for_file(path: &str) -> ModuleIdentity {
    let segments = path.split('/').collect::<Vec<_>>();
    let crate_name = segments.get(1).copied().unwrap_or("fixture").to_owned();
    let Some(src) = segments.iter().position(|segment| *segment == "src") else {
        let stem = Path::new(path)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("build");
        return ModuleIdentity::new(
            format!("synthetic:{crate_name}:{path}"),
            vec![crate_name, stem.to_owned()],
        );
    };
    let mut module = ModuleIdentity::new(format!("synthetic:{crate_name}:lib"), vec![crate_name]);
    let tail = &segments[src + 1..];
    for (index, segment) in tail.iter().enumerate() {
        let last = index + 1 == tail.len();
        let value = segment.strip_suffix(".rs").unwrap_or(segment);
        if last && matches!(value, "lib" | "main" | "mod") {
            continue;
        }
        module = module.child(value.to_owned());
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
        Type::ImplTrait(object) => {
            for bound in &object.bounds {
                if let syn::TypeParamBound::Trait(bound) = bound {
                    paths.extend(paths_in_path(&bound.path));
                }
            }
        }
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
        Type::TraitObject(object) => {
            for bound in &object.bounds {
                if let syn::TypeParamBound::Trait(bound) = bound {
                    paths.extend(paths_in_path(&bound.path));
                }
            }
        }
        Type::Tuple(tuple) => {
            for element in &tuple.elems {
                visit_type_paths(element, paths);
            }
        }
        _ => {}
    }
}

fn paths_in_path(path: &syn::Path) -> Vec<Vec<String>> {
    let segments = path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>();
    let mut paths = vec![segments.clone()];
    if segments.len() >= 2
        && segments
            .last()
            .is_some_and(|name| name.chars().next().is_some_and(char::is_lowercase))
    {
        paths.push(segments[..segments.len() - 1].to_vec());
    }
    for segment in &path.segments {
        if let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments {
            for argument in &arguments.args {
                if let syn::GenericArgument::Type(argument) = argument {
                    visit_type_paths(argument, &mut paths);
                }
            }
        }
    }
    paths
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
        Type::ImplTrait(object) => {
            for bound in &object.bounds {
                if let syn::TypeParamBound::Trait(bound) = bound {
                    for segment in &bound.path.segments {
                        visit(&segment.ident.to_string());
                    }
                }
            }
        }
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
        Type::TraitObject(object) => {
            for bound in &object.bounds {
                if let syn::TypeParamBound::Trait(bound) = bound {
                    for segment in &bound.path.segments {
                        visit(&segment.ident.to_string());
                    }
                }
            }
        }
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

fn module_can_access_raw_parser(module: &ModuleIdentity) -> bool {
    module.segments.first().is_some_and(|name| name == "waml")
        && module.segments.get(1).is_some_and(|name| name == "uml")
        || module
            .segments
            .first()
            .is_some_and(|crate_name| crate_name == "waml-syntax")
}

fn is_allowlisted_leaf_codec(id: &FunctionId) -> bool {
    id.owner.is_none()
        && matches!(
            (id.module.segments.as_slice(), id.name.as_str()),
            ([crate_name, module], "parse_value")
                if crate_name == "waml" && module == "frontmatter"
        )
        || id.owner.is_none()
            && matches!(
                (id.module.segments.as_slice(), id.name.as_str()),
                ([crate_name, module], "parse_end" | "parse_ends")
                    if crate_name == "waml" && module == "model"
            )
        || id.owner.is_none()
            && matches!(
                (id.module.segments.as_slice(), id.name.as_str()),
                ([crate_name, first, second], "parse_selector")
                    if crate_name == "waml" && first == "uml" && second == "selector"
            )
        || id.owner.is_none()
            && matches!(
                (id.module.segments.as_slice(), id.name.as_str()),
                ([crate_name, first, second], "parse_link_ref" | "parse_link_in_text")
                    if crate_name == "waml" && first == "uml" && second == "analysis"
            )
        || id.owner.is_none()
            && id
                .module
                .segments
                .first()
                .is_some_and(|crate_name| crate_name == "waml_editor")
            && id
                .module
                .segments
                .last()
                .is_some_and(|module| module == "cli")
            && id.name == "parse_hex"
        || id.owner.is_none()
            && id
                .module
                .segments
                .first()
                .is_some_and(|crate_name| crate_name == "waml_editor")
            && id
                .module
                .segments
                .last()
                .is_some_and(|module| module == "scene")
            && matches!(
                id.name.as_str(),
                "placement_candidate" | "placement_preview"
            )
}

fn is_allowlisted_visible_model_text(summary: &FunctionSummary) -> bool {
    let id = &summary.id;
    if summary.allowlisted_standard_text_trait && id.name == "to_string" {
        return true;
    }
    matches!(
        (
            id.module.segments.as_slice(),
            id.owner.as_deref(),
            id.name.as_str()
        ),
        ([crate_name, module], None, "render_ends")
            if crate_name == "waml" && module == "model"
    ) || matches!(
        (
            id.module.segments.as_slice(),
            id.owner.as_deref(),
            id.name.as_str()
        ),
        (
            [crate_name, module],
            Some(
                "RelationshipKind"
                    | "BehaviorKind"
                    | "FlowNodeKind"
                    | "MessageVerb"
                    | "FragmentKind"
                    | "ElementType"
            ),
            "as_str" | "name" | "keyword"
        ) if crate_name == "waml" && module == "model"
    )
}

fn slash_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
