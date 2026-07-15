use crate::frontmatter::{FmValue, Frontmatter};
use crate::model::{Attribute, ClassifierType, RelEnd, RelationshipKind, TypeRef, Visibility};
use crate::multiplicity::Multiplicity;
use crate::okf;
use crate::parse::parse_document;
use crate::serialize::serialize_document;
use crate::syntax::{Document, Line, ParsedName, ParsedRel, Section};

pub type Bundle = Vec<(String, String)>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpError {
    pub index: usize,
    pub op: String,
    pub selector: Option<String>,
    pub reason: String,
}

impl OpError {
    pub(crate) fn at(op: &str, reason: impl Into<String>) -> OpError {
        OpError { index: 0, op: op.to_string(), selector: None, reason: reason.into() }
    }

    pub(crate) fn with_sel(mut self, sel: String) -> OpError {
        self.selector = Some(sel);
        self
    }
}

/// How a relationship's name is given on an op (a `Ref`'s title is resolved at apply time).
#[derive(Debug, Clone, PartialEq)]
pub enum NameSpec {
    Label(String),
    Ref(String), // target slug
}

/// A fully-specified display block. The panel always holds a resolved
/// display, so every non-nullable field is present; nullable fields use
/// their own absent state (`None` ⇒ omit the key).
#[derive(Debug, Clone, PartialEq)]
pub struct DiagramDisplaySet {
    pub show_attributes: bool,
    pub attribute_detail: String,
    pub show_attribute_visibility: bool,
    pub show_attribute_multiplicity: bool,
    pub max_attributes: Option<u32>,
    pub association_labels: String,
    pub emphasize_multiplicity: bool,
    pub show_stereotype: bool,
    pub stereotype_filter: Option<Vec<String>>,
    pub stereotype_colors: Vec<String>,
}

/// One mutation. One variant per sugar command; grows task by task.
#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    AttrAdd {
        node: String,
        name: String,
        ty_token: String,
        multiplicity: Multiplicity,
        visibility: Option<Visibility>,
    },
    AttrSet {
        node: String,
        name: String,
        ty_token: Option<String>,
        multiplicity: Option<Multiplicity>,
        visibility: Option<Visibility>,
        rename: Option<String>,
    },
    AttrRm { node: String, name: String },
    ValueAdd { node: String, literal: String },
    ValueRm { node: String, literal: String },
    RelAdd {
        source: String,
        kind: RelationshipKind,
        target: String,
        name: Option<NameSpec>,
        ends: Option<(RelEnd, RelEnd)>,
    },
    RelSet { selector: Selector, ends: Option<(RelEnd, RelEnd)>, name: Option<NameSpec> },
    RelRm { selector: Selector },
    NodeNew {
        slug: String,
        /// Target package directory ("" = root). File written at `<dir>/<slug>.md`.
        dir: String,
        ty: ClassifierType,
        title: String,
        stereotype: Vec<String>,
        description: Option<String>,
        abstract_: bool,
    },
    NodeSet {
        slug: String,
        title: Option<String>,
        description: Option<String>,
        stereotype: Option<Vec<String>>,
        abstract_: Option<bool>,
        ty: Option<ClassifierType>,
    },
    NodeRm { slug: String, cascade: bool },
    NodeRename { from: String, to: String },
    PkgMove { slug: String, to_dir: String },
    PkgRename { from: String, to: String },
    PkgDelete { path: String, cascade: bool },
    PkgReorder { path: String, order: Vec<String> },
    PkgSort { path: String },
    DiagramSet {
        key: String,                          // diagram doc id (full-path or bare slug)
        title: Option<String>,                // None = leave unchanged
        description: Option<String>,          // None = leave unchanged
        display: Option<DiagramDisplaySet>,   // None = leave display untouched
    },
}

pub fn apply(bundle: &[(String, String)], ops: &[Op]) -> Result<Bundle, OpError> {
    let mut work: Bundle = bundle.to_vec();
    for (i, op) in ops.iter().enumerate() {
        apply_one(&mut work, op).map_err(|mut e| {
            e.index = i;
            e
        })?;
    }
    Ok(work)
}

fn apply_one(work: &mut Bundle, op: &Op) -> Result<(), OpError> {
    match op {
        Op::AttrAdd { node, name, ty_token, multiplicity, visibility } => {
            op_attr_add(work, node, name, ty_token, multiplicity, *visibility)
        }
        Op::AttrSet { node, name, ty_token, multiplicity, visibility, rename } => {
            op_attr_set(work, node, name, ty_token, multiplicity, *visibility, rename)
        }
        Op::AttrRm { node, name } => op_attr_rm(work, node, name),
        Op::ValueAdd { node, literal } => op_value_add(work, node, literal),
        Op::ValueRm { node, literal } => op_value_rm(work, node, literal),
        Op::RelAdd { source, kind, target, name, ends } => {
            op_rel_add(work, source, *kind, target, name, ends)
        }
        Op::RelSet { selector, ends, name } => op_rel_set(work, selector, ends, name),
        Op::RelRm { selector } => op_rel_rm(work, selector),
        Op::NodeNew { slug, dir, ty, title, stereotype, description, abstract_ } => {
            op_node_new(work, slug, dir, ty, title, stereotype, description, *abstract_)
        }
        Op::NodeSet { slug, title, description, stereotype, abstract_, ty } => {
            op_node_set(work, slug, title, description, stereotype, abstract_, ty)
        }
        Op::NodeRm { slug, cascade } => op_node_rm(work, slug, *cascade),
        Op::NodeRename { from, to } => rename::op_node_rename(work, from, to),
        Op::PkgMove { slug, to_dir } => pkg::op_pkg_move(work, slug, to_dir),
        Op::PkgRename { from, to } => pkg::op_pkg_rename(work, from, to),
        Op::PkgDelete { path, cascade } => pkg::op_pkg_delete(work, path, *cascade),
        Op::PkgReorder { path, order } => pkg::op_pkg_reorder(work, path, order),
        Op::PkgSort { path } => pkg::op_pkg_sort(work, path),
        Op::DiagramSet { key, title, description, display } => {
            op_diagram_set(work, key, title, description, display)
        }
    }
}

// ---- shared helpers (reused by every later op) ----

pub(crate) fn slug_of(path: &str) -> String {
    let seg = path.rsplit(['/', '\\']).next().unwrap_or(path);
    seg.strip_suffix(".md").unwrap_or(seg).to_string()
}

/// Resolve a caller-given target — a full bundle-path id (`okf::id_of`, what
/// the parse/graph layer keys `Node`/`Edge`/`Diagram` by) or a bare basename
/// — to the doc's index. Full-id match takes priority; a bare target falls
/// back to a unique-basename match across the bundle (mirrors
/// `solve::resolve`'s `NameRef::Bare` handling). Ambiguous basenames are left
/// unresolved rather than guessing.
pub(crate) fn resolve_index(work: &Bundle, target: &str) -> Option<usize> {
    if let Some(i) = work.iter().position(|(p, _)| okf::id_of(p) == target) {
        return Some(i);
    }
    let mut matches = work.iter().enumerate().filter(|(_, (p, _))| slug_of(p) == target);
    match (matches.next(), matches.next()) {
        (Some((i, _)), None) => Some(i),
        _ => None,
    }
}

/// The bare basename actually embedded in this bundle's same-directory
/// relative hrefs (`./slug.md`) for a resolved target. An unresolved token
/// (a forward reference to a not-yet-existing doc) passes through unchanged.
pub(crate) fn stored_slug(work: &Bundle, target: &str) -> String {
    resolve_index(work, target).map(|i| slug_of(&work[i].0)).unwrap_or_else(|| target.to_string())
}

pub(crate) fn find_doc(work: &Bundle, target: &str, op: &str) -> Result<usize, OpError> {
    resolve_index(work, target).ok_or_else(|| OpError::at(op, format!("no document '{target}'")))
}

/// Parse the target file, mutate via `f`, re-serialize canonically.
pub(crate) fn edit_doc<F>(work: &mut Bundle, slug: &str, op: &str, f: F) -> Result<(), OpError>
where
    F: FnOnce(&mut Document) -> Result<(), OpError>,
{
    let i = find_doc(work, slug, op)?;
    let mut doc = parse_document(&work[i].1);
    f(&mut doc)?;
    work[i].1 = serialize_document(&doc);
    Ok(())
}

/// Get the `## Attributes` list, creating an empty section if absent
/// (canonical serialize re-orders sections, so append position is irrelevant).
pub(crate) fn attrs_mut(doc: &mut Document) -> &mut Vec<Line<Attribute>> {
    if !doc.sections.iter().any(|s| matches!(s, Section::Attributes(_))) {
        doc.sections.push(Section::Attributes(Vec::new()));
    }
    doc.sections
        .iter_mut()
        .find_map(|s| match s {
            Section::Attributes(a) => Some(a),
            _ => None,
        })
        .expect("attributes section just ensured")
}

/// Get the `## Values` list, creating an empty section if absent
/// (canonical serialize re-orders sections, so append position is irrelevant).
pub(crate) fn values_mut(doc: &mut Document) -> &mut Vec<Line<String>> {
    if !doc.sections.iter().any(|s| matches!(s, Section::Values(_))) {
        doc.sections.push(Section::Values(Vec::new()));
    }
    doc.sections
        .iter_mut()
        .find_map(|s| match s {
            Section::Values(v) => Some(v),
            _ => None,
        })
        .expect("values section just ensured")
}

/// Forward-ref-safe: a token matching an existing doc slug links to it (using
/// that doc's title); otherwise it is a bare type token. Mirrors build_model.
pub(crate) fn resolve_type(work: &Bundle, token: &str) -> TypeRef {
    if let Some(i) = resolve_index(work, token) {
        let (path, text) = &work[i];
        let title = parse_document(text)
            .frontmatter
            .get_str("title")
            .map(String::from)
            .unwrap_or_else(|| token.to_string());
        TypeRef { name: title, ref_: Some(slug_of(path)) }
    } else {
        TypeRef { name: token.to_string(), ref_: None }
    }
}

/// Get the `## Relationships` list, creating an empty section if absent
/// (canonical serialize re-orders sections, so append position is irrelevant).
pub(crate) fn rels_mut(doc: &mut Document) -> &mut Vec<Line<ParsedRel>> {
    if !doc.sections.iter().any(|s| matches!(s, Section::Relationships(_))) {
        doc.sections.push(Section::Relationships(Vec::new()));
    }
    doc.sections
        .iter_mut()
        .find_map(|s| match s {
            Section::Relationships(r) => Some(r),
            _ => None,
        })
        .expect("relationships section just ensured")
}

/// Look up a document's `title` by slug, falling back to the slug itself
/// (forward-ref-safe, mirrors `resolve_type`).
pub(crate) fn resolve_title(work: &Bundle, slug: &str) -> String {
    resolve_index(work, slug)
        .and_then(|i| parse_document(&work[i].1).frontmatter.get_str("title").map(String::from))
        .unwrap_or_else(|| slug.to_string())
}

/// Resolve an op's `NameSpec` into the `ParsedName` stored on the document
/// (a `Ref`'s title is resolved against the bundle at apply time).
fn build_name(work: &Bundle, spec: &Option<NameSpec>) -> Option<ParsedName> {
    match spec {
        None => None,
        Some(NameSpec::Label(l)) => Some(ParsedName::Label(l.clone())),
        Some(NameSpec::Ref(slug)) => {
            Some(ParsedName::Ref { title: resolve_title(work, slug), slug: stored_slug(work, slug) })
        }
    }
}

/// `RelBy::Endpoint.target` may be a full bundle-path id (the parse/graph
/// layer's edge key); `ParsedRel.target_slug` is always the bare
/// same-directory-relative href token. Resolve before matching.
fn resolve_rel_by(work: &Bundle, by: &RelBy) -> RelBy {
    match by {
        RelBy::Endpoint { kind, target } => RelBy::Endpoint { kind: *kind, target: stored_slug(work, target) },
        RelBy::Named(name) => RelBy::Named(name.clone()),
    }
}

/// Does a parsed relationship match a selector's `RelBy` address?
fn rel_matches(r: &ParsedRel, by: &RelBy) -> bool {
    match by {
        RelBy::Endpoint { kind, target } => r.kind == *kind && r.target_slug == *target,
        RelBy::Named(name) => match &r.name {
            Some(ParsedName::Label(l)) => l == name,
            Some(ParsedName::Ref { title, .. }) => title == name,
            None => false,
        },
    }
}

/// Extract `(source, by)` from a `Selector::Rel`, erroring for any other selector shape.
fn rel_target<'a>(selector: &'a Selector, op: &str) -> Result<(&'a str, &'a RelBy), OpError> {
    match selector {
        Selector::Rel { source, by } => Ok((source.as_str(), by)),
        _ => Err(OpError::at(op, format!("selector '{}' does not address a relationship", render_selector(selector)))
            .with_sel(render_selector(selector))),
    }
}

fn fm_set(fm: &mut Frontmatter, key: &str, val: FmValue) {
    if let Some(e) = fm.entries.iter_mut().find(|(k, _)| k == key) {
        e.1 = val;
    } else {
        fm.entries.push((key.to_string(), val));
    }
}

fn str_list(items: &[String]) -> FmValue {
    FmValue::List(items.iter().map(|s| FmValue::Str(s.clone())).collect())
}

/// Slugs of every document that references `slug` (rel target, attribute
/// type-ref, `as [Ref]` name, diagram member). Sorted, deduped.
pub fn referrers(work: &Bundle, slug: &str) -> Vec<String> {
    // Referring docs store bare same-directory-relative hrefs (`./slug.md`),
    // not full ids — translate `slug` (which may be a full bundle-path id,
    // per `resolve_index`) down to that bare form before matching stored refs.
    let target_idx = resolve_index(work, slug);
    let target = target_idx.map(|i| slug_of(&work[i].0)).unwrap_or_else(|| slug.to_string());
    let mut out = Vec::new();
    for (i, (p, text)) in work.iter().enumerate() {
        if Some(i) == target_idx {
            continue;
        }
        let doc = parse_document(text);
        let hit = doc.sections.iter().any(|sec| match sec {
            Section::Attributes(attrs) => attrs.iter().filter_map(Line::parsed).any(|a| a.ty.ref_.as_deref() == Some(target.as_str())),
            Section::Relationships(rels) => rels.iter().filter_map(Line::parsed).any(|r| {
                r.target_slug == target
                    || matches!(&r.name, Some(ParsedName::Ref { slug: rs, .. }) if rs == &target)
            }),
            Section::Members(block) => {
                fn group_has(g: &crate::syntax::MemberGroup, slug: &str) -> bool {
                    g.members.iter().filter_map(Line::parsed).any(|m| m.slug == slug)
                        || g.children.iter().any(|c| group_has(c, slug))
                }
                block.groups.iter().any(|g| group_has(g, &target))
            }
            Section::Layout(stmts) => {
                fn operand_refs(op: &crate::syntax::Operand, slug: &str) -> bool {
                    use crate::syntax::{NameRef, OperandRef};
                    match &op.ref_ {
                        OperandRef::Name(NameRef::Link { slug: rs, .. }) => rs == slug,
                        OperandRef::Name(NameRef::Bare(s)) => s == slug,
                        OperandRef::InlineGroup { items, .. } => items.iter().any(|it| operand_refs(it, slug)),
                        OperandRef::Paren(inner) => operand_refs(inner, slug),
                    }
                }
                stmts.iter().filter_map(Line::parsed).any(|it| match &it.stmt {
                    crate::syntax::LayoutStatement::Standalone(op) => operand_refs(op, &target),
                    crate::syntax::LayoutStatement::Placement { operands, .. } => {
                        operands.iter().any(|op| operand_refs(op, &target))
                    }
                    crate::syntax::LayoutStatement::Alignment { left, right } => {
                        operand_refs(&left.operand, &target) || operand_refs(&right.operand, &target)
                    }
                })
            }
            _ => false,
        });
        if hit {
            out.push(slug_of(p));
        }
    }
    out.sort();
    out.dedup();
    out
}

fn op_attr_add(
    work: &mut Bundle,
    node: &str,
    name: &str,
    ty_token: &str,
    multiplicity: &Multiplicity,
    visibility: Option<Visibility>,
) -> Result<(), OpError> {
    let ty = resolve_type(work, ty_token);
    edit_doc(work, node, "attr.add", |doc| {
        let attrs = attrs_mut(doc);
        if attrs.iter().filter_map(Line::parsed).any(|a| a.name == name) {
            return Err(OpError::at("attr.add", format!("attribute '{name}' already exists in {node}")));
        }
        attrs.push(Line::Parsed(Attribute {
            name: name.to_string(),
            ty,
            multiplicity: multiplicity.clone(),
            visibility,
            description: None,
        }));
        Ok(())
    })
}

#[allow(clippy::too_many_arguments)]
fn op_attr_set(
    work: &mut Bundle,
    node: &str,
    name: &str,
    ty_token: &Option<String>,
    multiplicity: &Option<Multiplicity>,
    visibility: Option<Visibility>,
    rename: &Option<String>,
) -> Result<(), OpError> {
    let ty = ty_token.as_ref().map(|t| resolve_type(work, t));
    edit_doc(work, node, "attr.set", |doc| {
        let attrs = attrs_mut(doc);
        if let Some(new) = rename {
            if new != name && attrs.iter().filter_map(Line::parsed).any(|a| a.name == *new) {
                return Err(OpError::at("attr.set", format!("attribute '{new}' already exists in {node}")));
            }
        }
        let a = attrs
            .iter_mut()
            .filter_map(Line::parsed_mut)
            .find(|a| a.name == name)
            .ok_or_else(|| OpError::at("attr.set", format!("no attribute '{name}' in {node}")))?;
        if let Some(t) = ty {
            a.ty = t;
        }
        if let Some(m) = multiplicity {
            a.multiplicity = m.clone();
        }
        if let Some(v) = visibility {
            a.visibility = Some(v);
        }
        if let Some(new) = rename {
            a.name = new.clone();
        }
        Ok(())
    })
}

fn op_attr_rm(work: &mut Bundle, node: &str, name: &str) -> Result<(), OpError> {
    edit_doc(work, node, "attr.rm", |doc| {
        let attrs = attrs_mut(doc);
        let before = attrs.len();
        attrs.retain(|a| a.parsed().is_none_or(|x| x.name != name));
        if attrs.len() == before {
            return Err(OpError::at("attr.rm", format!("no attribute '{name}' in {node}")));
        }
        Ok(())
    })
}

fn op_value_add(work: &mut Bundle, node: &str, literal: &str) -> Result<(), OpError> {
    edit_doc(work, node, "value.add", |doc| {
        let values = values_mut(doc);
        if values.iter().filter_map(Line::parsed).any(|v| v == literal) {
            return Err(OpError::at("value.add", format!("value '{literal}' already in {node}")));
        }
        values.push(Line::Parsed(literal.to_string()));
        Ok(())
    })
}

fn op_value_rm(work: &mut Bundle, node: &str, literal: &str) -> Result<(), OpError> {
    edit_doc(work, node, "value.rm", |doc| {
        let values = values_mut(doc);
        let before = values.len();
        values.retain(|l| l.parsed().is_none_or(|v| v != literal));
        if values.len() == before {
            return Err(OpError::at("value.rm", format!("no value '{literal}' in {node}")));
        }
        Ok(())
    })
}

fn op_rel_add(
    work: &mut Bundle,
    source: &str,
    kind: RelationshipKind,
    target: &str,
    name: &Option<NameSpec>,
    ends: &Option<(RelEnd, RelEnd)>,
) -> Result<(), OpError> {
    if kind.is_ended() != ends.is_some() {
        let msg = if kind.is_ended() {
            format!("relationship '{}' requires ends", kind.as_str())
        } else {
            format!("relationship '{}' does not take ends", kind.as_str())
        };
        return Err(OpError::at("rel.add", msg));
    }
    let target_title = resolve_title(work, target);
    let target_ref = stored_slug(work, target);
    let name = build_name(work, name);
    let ends = ends.clone();
    edit_doc(work, source, "rel.add", |doc| {
        let rels = rels_mut(doc);
        if rels.iter().filter_map(Line::parsed).any(|r| r.kind == kind && r.target_slug == target_ref) {
            return Err(OpError::at(
                "rel.add",
                format!("relationship '{} {target}' already exists in {source}", kind.as_str()),
            ));
        }
        let (from_end, to_end) = ends.unwrap_or_default();
        rels.push(Line::Parsed(ParsedRel {
            kind,
            target_title,
            target_slug: target_ref,
            name,
            from_end,
            to_end,
            line: 0,
            span: None,
        }));
        Ok(())
    })
}

fn op_rel_set(
    work: &mut Bundle,
    selector: &Selector,
    ends: &Option<(RelEnd, RelEnd)>,
    name: &Option<NameSpec>,
) -> Result<(), OpError> {
    let (source, by) = rel_target(selector, "rel.set")?;
    let (source, by) = (source.to_string(), resolve_rel_by(work, by));
    let disp = render_selector(selector);
    let new_ends = ends.clone();
    let new_name = build_name(work, name);
    edit_doc(work, &source, "rel.set", |doc| {
        let rels = rels_mut(doc);
        let r = rels
            .iter_mut()
            .filter_map(Line::parsed_mut)
            .find(|r| rel_matches(r, &by))
            .ok_or_else(|| OpError::at("rel.set", format!("no relationship '{disp}'")).with_sel(disp.clone()))?;
        if let Some((f, t)) = new_ends {
            if !r.kind.is_ended() {
                return Err(OpError::at("rel.set", format!("'{}' does not take ends", r.kind.as_str())));
            }
            r.from_end = f;
            r.to_end = t;
        }
        if let Some(n) = new_name {
            r.name = Some(n);
        }
        Ok(())
    })
}

fn op_rel_rm(work: &mut Bundle, selector: &Selector) -> Result<(), OpError> {
    let (source, by) = rel_target(selector, "rel.rm")?;
    let (source, by) = (source.to_string(), resolve_rel_by(work, by));
    let disp = render_selector(selector);
    edit_doc(work, &source, "rel.rm", |doc| {
        let rels = rels_mut(doc);
        let before = rels.len();
        rels.retain(|r| r.parsed().is_none_or(|x| !rel_matches(x, &by)));
        if rels.len() == before {
            return Err(OpError::at("rel.rm", format!("no relationship '{disp}'")).with_sel(disp.clone()));
        }
        Ok(())
    })
}

#[allow(clippy::too_many_arguments)]
fn op_node_new(
    work: &mut Bundle,
    slug: &str,
    dir: &str,
    ty: &ClassifierType,
    title: &str,
    stereotype: &[String],
    description: &Option<String>,
    abstract_: bool,
) -> Result<(), OpError> {
    let path = if dir.is_empty() { format!("{slug}.md") } else { format!("{dir}/{slug}.md") };
    if work.iter().any(|(p, _)| okf::id_of(p) == okf::id_of(&path)) {
        return Err(OpError::at("node.new", format!("document '{slug}' already exists")));
    }
    let mut entries: Vec<(String, FmValue)> = vec![("type".into(), FmValue::Str(ty.as_str()))];
    if !stereotype.is_empty() {
        entries.push(("stereotype".into(), str_list(stereotype)));
    }
    if abstract_ {
        entries.push(("abstract".into(), FmValue::Bool(true)));
    }
    entries.push(("title".into(), FmValue::Str(title.to_string())));
    if let Some(d) = description {
        entries.push(("description".into(), FmValue::Str(d.clone())));
    }
    let doc = Document {
        frontmatter: Frontmatter { entries },
        title: title.to_string(),
        sections: Vec::new(),
    };
    work.push((path, serialize_document(&doc)));
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn op_node_set(
    work: &mut Bundle,
    slug: &str,
    title: &Option<String>,
    description: &Option<String>,
    stereotype: &Option<Vec<String>>,
    abstract_: &Option<bool>,
    ty: &Option<ClassifierType>,
) -> Result<(), OpError> {
    edit_doc(work, slug, "node.set", |doc| {
        if let Some(t) = title {
            fm_set(&mut doc.frontmatter, "title", FmValue::Str(t.clone()));
            doc.title = t.clone();
        }
        if let Some(d) = description {
            fm_set(&mut doc.frontmatter, "description", FmValue::Str(d.clone()));
        }
        if let Some(list) = stereotype {
            fm_set(&mut doc.frontmatter, "stereotype", str_list(list));
        }
        if let Some(a) = abstract_ {
            fm_set(&mut doc.frontmatter, "abstract", FmValue::Bool(*a));
        }
        if let Some(t) = ty {
            fm_set(&mut doc.frontmatter, "type", FmValue::Str(t.as_str()));
        }
        Ok(())
    })
}

const DISPLAY_KEYS: &[&str] = &[
    "showAttributes", "attributeDetail", "showAttributeVisibility",
    "showAttributeMultiplicity", "maxAttributes", "associationLabels",
    "emphasizeMultiplicity", "showStereotype", "stereotypeFilter",
    "stereotypeColors",
];

fn op_diagram_set(
    work: &mut Bundle,
    key: &str,
    title: &Option<String>,
    description: &Option<String>,
    display: &Option<DiagramDisplaySet>,
) -> Result<(), OpError> {
    edit_doc(work, key, "diagram.set", |doc| {
        if let Some(t) = title {
            fm_set(&mut doc.frontmatter, "title", FmValue::Str(t.clone()));
            doc.title = t.clone();
        }
        if let Some(d) = description {
            fm_set(&mut doc.frontmatter, "description", FmValue::Str(d.clone()));
        }
        if let Some(ds) = display {
            // Whole-block replace: drop every display key first so a field left
            // absent on this DiagramSet (e.g. maxAttributes not present on `ds`)
            // clears back to its tri-state-absent wire representation, then
            // re-set exactly the keys this fully-resolved display carries.
            doc.frontmatter.entries.retain(|(k, _)| !DISPLAY_KEYS.contains(&k.as_str()));
            fm_set(&mut doc.frontmatter, "showAttributes", FmValue::Bool(ds.show_attributes));
            fm_set(&mut doc.frontmatter, "attributeDetail", FmValue::Str(ds.attribute_detail.clone()));
            fm_set(&mut doc.frontmatter, "showAttributeVisibility", FmValue::Bool(ds.show_attribute_visibility));
            fm_set(&mut doc.frontmatter, "showAttributeMultiplicity", FmValue::Bool(ds.show_attribute_multiplicity));
            if let Some(max) = ds.max_attributes {
                fm_set(&mut doc.frontmatter, "maxAttributes", FmValue::Num(max as f64));
            }
            fm_set(&mut doc.frontmatter, "associationLabels", FmValue::Str(ds.association_labels.clone()));
            fm_set(&mut doc.frontmatter, "emphasizeMultiplicity", FmValue::Bool(ds.emphasize_multiplicity));
            fm_set(&mut doc.frontmatter, "showStereotype", FmValue::Bool(ds.show_stereotype));
            if let Some(filter) = &ds.stereotype_filter {
                fm_set(&mut doc.frontmatter, "stereotypeFilter", str_list(filter));
            }
            if !ds.stereotype_colors.is_empty() {
                fm_set(&mut doc.frontmatter, "stereotypeColors", str_list(&ds.stereotype_colors));
            }
        }
        Ok(())
    })
}

fn op_node_rm(work: &mut Bundle, slug: &str, cascade: bool) -> Result<(), OpError> {
    let i = find_doc(work, slug, "node.rm")?;
    if !cascade {
        let refs = referrers(work, slug);
        if !refs.is_empty() {
            return Err(OpError::at(
                "node.rm",
                format!("'{slug}' referenced by: {} (use --cascade)", refs.join(", ")),
            ));
        }
    }
    work.remove(i);
    Ok(())
}

pub mod pkg;
pub mod rename;
pub mod selector;
pub use selector::{parse_selector, render_selector, RelBy, Selector};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::multiplicity::Multiplicity;
    use crate::ops::selector::{RelBy, Selector};
    use crate::model::RelationshipKind;
    use crate::grammar::parse_ends;
    use crate::model::ClassifierType;

    fn attr_add(node: &str, name: &str, ty: &str) -> Op {
        Op::AttrAdd { node: node.into(), name: name.into(), ty_token: ty.into(),
            multiplicity: Multiplicity::default(), visibility: None }
    }

    #[test]
    fn attr_add_appends_a_bare_attribute() {
        let b = vec![("shop/order.md".to_string(),
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId\n".to_string())];
        let out = apply(&b, &[attr_add("order", "total", "Money")]).unwrap();
        assert!(out[0].1.contains("- total: Money"));
        assert!(out[0].1.contains("- id: OrderId"), "existing attr kept");
    }

    #[test]
    fn attr_add_links_a_known_slug() {
        let b = vec![
            ("a/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string()),
            ("a/money.md".to_string(), "---\ntype: uml.DataType\ntitle: Money\n---\n# Money\n".to_string()),
        ];
        let out = apply(&b, &[attr_add("order", "total", "money")]).unwrap();
        assert!(out[0].1.contains("- total: [Money](./money.md)"), "known slug links with target title");
    }

    #[test]
    fn attr_add_refuses_a_duplicate_name() {
        let b = vec![("a/order.md".to_string(),
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId\n".to_string())];
        let err = apply(&b, &[attr_add("order", "id", "X")]).unwrap_err();
        assert_eq!(err.index, 0);
        assert_eq!(err.op, "attr.add");
        assert!(err.reason.contains("already exists"));
    }

    #[test]
    fn attr_add_on_missing_node_errors() {
        let b: Bundle = vec![];
        let err = apply(&b, &[attr_add("ghost", "x", "Y")]).unwrap_err();
        assert!(err.reason.contains("no document 'ghost'"));
    }

    #[test]
    fn apply_is_atomic_on_a_later_failure() {
        let b = vec![("a/order.md".to_string(),
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId\n".to_string())];
        let ops = vec![attr_add("order", "total", "Money"), attr_add("order", "id", "X")]; // 2nd is a dup
        let err = apply(&b, &ops).unwrap_err();
        assert_eq!(err.index, 1, "failing op index reported");
        assert!(!b[0].1.contains("total"), "input bundle untouched; caller writes nothing");
    }

    #[test]
    fn attr_set_changes_type_and_multiplicity() {
        let b = vec![("a/order.md".to_string(),
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId\n".to_string())];
        let out = apply(&b, &[Op::AttrSet {
            node: "order".into(), name: "id".into(),
            ty_token: Some("String".into()),
            multiplicity: Some(Multiplicity::parse("0..1").unwrap()),
            visibility: Some(crate::model::Visibility::Private),
            rename: None,
        }]).unwrap();
        assert!(out[0].1.contains("- id: String {0..1}"));
        let doc = parse_document(&out[0].1);
        let attrs = doc.sections.iter().find_map(|s| match s {
            Section::Attributes(a) => Some(a),
            _ => None,
        }).expect("attributes section present");
        let id = attrs.iter().filter_map(Line::parsed).find(|a| a.name == "id").expect("id attribute present");
        assert_eq!(id.visibility, Some(crate::model::Visibility::Private));
    }

    #[test]
    fn attr_set_renames_and_refuses_collision() {
        let b = vec![("a/order.md".to_string(),
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId\n- total: Money\n".to_string())];
        let ok = apply(&b, &[Op::AttrSet { node:"order".into(), name:"id".into(), ty_token:None, multiplicity:None, visibility:None, rename: Some("orderId".into()) }]).unwrap();
        assert!(ok[0].1.contains("- orderId: OrderId"));
        let err = apply(&b, &[Op::AttrSet { node:"order".into(), name:"id".into(), ty_token:None, multiplicity:None, visibility:None, rename: Some("total".into()) }]).unwrap_err();
        assert!(err.reason.contains("already exists"));
    }

    #[test]
    fn attr_set_on_missing_attr_errors() {
        let b = vec![("a/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string())];
        let err = apply(&b, &[Op::AttrSet { node:"order".into(), name:"ghost".into(), ty_token:Some("X".into()), multiplicity:None, visibility:None, rename:None }]).unwrap_err();
        assert!(err.reason.contains("no attribute 'ghost'"));
    }

    #[test]
    fn attr_rm_removes_and_refuses_missing() {
        let b = vec![("a/order.md".to_string(),
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId\n- total: Money\n".to_string())];
        let out = apply(&b, &[Op::AttrRm { node:"order".into(), name:"total".into() }]).unwrap();
        assert!(!out[0].1.contains("total"));
        assert!(out[0].1.contains("- id: OrderId"));
        let err = apply(&b, &[Op::AttrRm { node:"order".into(), name:"ghost".into() }]).unwrap_err();
        assert!(err.reason.contains("no attribute 'ghost'"));
    }

    #[test]
    fn value_add_appends_and_refuses_duplicate() {
        let b = vec![("a/order-status.md".to_string(),
            "---\ntype: uml.Enum\ntitle: OrderStatus\n---\n# OrderStatus\n\n## Values\n- DRAFT\n".to_string())];
        let out = apply(&b, &[Op::ValueAdd { node:"order-status".into(), literal:"PLACED".into() }]).unwrap();
        assert!(out[0].1.contains("- DRAFT"));
        assert!(out[0].1.contains("- PLACED"));
        let err = apply(&b, &[Op::ValueAdd { node:"order-status".into(), literal:"DRAFT".into() }]).unwrap_err();
        assert!(err.reason.contains("already"));
    }

    #[test]
    fn value_rm_removes_and_refuses_missing() {
        let b = vec![("a/order-status.md".to_string(),
            "---\ntype: uml.Enum\ntitle: OrderStatus\n---\n# OrderStatus\n\n## Values\n- DRAFT\n- PLACED\n".to_string())];
        let out = apply(&b, &[Op::ValueRm { node:"order-status".into(), literal:"DRAFT".into() }]).unwrap();
        assert!(!out[0].1.contains("DRAFT"));
        assert!(out[0].1.contains("- PLACED"));
        let err = apply(&b, &[Op::ValueRm { node:"order-status".into(), literal:"GONE".into() }]).unwrap_err();
        assert!(err.reason.contains("no value 'GONE'"));
    }

    #[test]
    fn rel_add_composes_with_ends() {
        let b = vec![
            ("a/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string()),
            ("a/order-line.md".to_string(), "---\ntype: uml.Class\ntitle: OrderLine\n---\n# OrderLine\n".to_string()),
        ];
        let out = apply(&b, &[Op::RelAdd {
            source: "order".into(), kind: RelationshipKind::Composes, target: "order-line".into(),
            name: None, ends: parse_ends("1 to 1..* lines"),
        }]).unwrap();
        assert!(out[0].1.contains("- composes [OrderLine](./order-line.md): 1 to 1..* lines"));
    }

    #[test]
    fn rel_add_enforces_ends_xor_verb() {
        let b = vec![("a/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string())];
        // composes requires ends
        let e1 = apply(&b, &[Op::RelAdd { source:"order".into(), kind:RelationshipKind::Composes, target:"x".into(), name:None, ends:None }]).unwrap_err();
        assert!(e1.reason.contains("requires ends"));
        // depends forbids ends
        let e2 = apply(&b, &[Op::RelAdd { source:"order".into(), kind:RelationshipKind::Depends, target:"x".into(), name:None, ends: parse_ends("1 to 1") }]).unwrap_err();
        assert!(e2.reason.contains("does not take ends"));
    }

    #[test]
    fn rel_add_allows_forward_ref_and_refuses_duplicate() {
        let b = vec![("a/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string())];
        // forward ref (ghost.md absent) is allowed; title falls back to the slug
        let out = apply(&b, &[Op::RelAdd { source:"order".into(), kind:RelationshipKind::Depends, target:"ghost".into(), name:None, ends:None }]).unwrap();
        assert!(out[0].1.contains("- depends [ghost](./ghost.md)"));
        let dup = apply(&out, &[Op::RelAdd { source:"order".into(), kind:RelationshipKind::Depends, target:"ghost".into(), name:None, ends:None }]).unwrap_err();
        assert!(dup.reason.contains("already exists"));
    }

    #[test]
    fn rel_set_updates_ends_and_rel_rm_removes() {
        let b = vec![
            ("a/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Relationships\n- composes [OrderLine](./order-line.md): 1 to 1..* lines\n".to_string()),
            ("a/order-line.md".to_string(), "---\ntype: uml.Class\ntitle: OrderLine\n---\n# OrderLine\n".to_string()),
        ];
        let sel = Selector::Rel { source:"order".into(), by: RelBy::Endpoint { kind: RelationshipKind::Composes, target:"order-line".into() } };
        let set = apply(&b, &[Op::RelSet { selector: sel.clone(), ends: parse_ends("1 to *"), name: None }]).unwrap();
        assert!(set[0].1.contains(": 1 to *"));
        let rm = apply(&b, &[Op::RelRm { selector: sel }]).unwrap();
        assert!(!rm[0].1.contains("composes"));
    }

    #[test]
    fn rel_rm_on_missing_rel_errors() {
        let b = vec![("a/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string())];
        let sel = Selector::Rel { source:"order".into(), by: RelBy::Named("nope".into()) };
        let err = apply(&b, &[Op::RelRm { selector: sel }]).unwrap_err();
        assert!(err.reason.contains("no relationship"));
        assert!(err.selector.is_some());
    }

    #[test]
    fn rel_set_on_missing_rel_errors() {
        let b = vec![("a/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string())];
        let sel = Selector::Rel { source:"order".into(), by: RelBy::Named("nope".into()) };
        let err = apply(&b, &[Op::RelSet { selector: sel, ends: None, name: None }]).unwrap_err();
        assert!(err.reason.contains("no relationship"));
        assert!(err.selector.is_some());
    }

    #[test]
    fn rel_matches_ref_named_selector() {
        let b = vec![
            ("a/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string()),
            ("a/customer.md".to_string(), "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n".to_string()),
            ("a/order-line.md".to_string(), "---\ntype: uml.Class\ntitle: OrderLine\n---\n# OrderLine\n".to_string()),
        ];
        let added = apply(&b, &[Op::RelAdd {
            source: "order".into(), kind: RelationshipKind::Depends, target: "order-line".into(),
            name: Some(NameSpec::Ref("customer".into())), ends: None,
        }]).unwrap();
        let sel = Selector::Rel { source: "order".into(), by: RelBy::Named("Customer".into()) };
        let rm = apply(&added, &[Op::RelRm { selector: sel }]).unwrap();
        assert!(!rm[0].1.contains("depends"), "Ref-named relationship must be reachable via RelBy::Named on its resolved title");
    }

    #[test]
    fn node_new_writes_frontmatter_and_title_and_refuses_dup() {
        let b: Bundle = vec![];
        let out = apply(&b, &[Op::NodeNew {
            slug: "order".into(), dir: String::new(), ty: ClassifierType::parse("uml.Class"), title: "Order".into(),
            stereotype: vec!["entity".into()], description: Some("An order.".into()), abstract_: false,
        }]).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].0, "order.md");
        assert!(out[0].1.contains("type: \"uml.Class\""));
        assert!(out[0].1.contains("title: \"Order\""));
        assert!(out[0].1.contains("# Order"));
        let dup = apply(&out, &[Op::NodeNew { slug:"order".into(), dir: String::new(), ty: ClassifierType::parse("uml.Class"), title:"X".into(), stereotype: vec![], description: None, abstract_: false }]).unwrap_err();
        assert!(dup.reason.contains("already exists"));
    }

    #[test]
    fn node_new_writes_into_target_directory() {
        let out = apply(&vec![], &[Op::NodeNew {
            slug: "order".into(), dir: "sales".into(), ty: ClassifierType::parse("uml.Class"),
            title: "Order".into(), stereotype: vec![], description: None, abstract_: false,
        }]).unwrap();
        assert_eq!(out[0].0, "sales/order.md");
    }

    #[test]
    fn node_set_updates_title_frontmatter_in_place() {
        let b = vec![("a/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string())];
        let out = apply(&b, &[Op::NodeSet {
            slug: "order".into(), title: Some("Sales Order".into()), description: None,
            stereotype: Some(vec!["aggregateRoot".into()]), abstract_: None, ty: None,
        }]).unwrap();
        assert_eq!(out[0].0, "a/order.md", "node.set never moves the file");
        assert!(out[0].1.contains("title: \"Sales Order\""));
        assert!(out[0].1.contains("# Sales Order"));
        assert!(out[0].1.contains("stereotype: [\"aggregateRoot\"]"));
    }

    #[test]
    fn node_rm_refuses_referenced_then_allows_cascade() {
        let b = vec![
            ("a/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Relationships\n- depends [Money](./money.md)\n".to_string()),
            ("a/money.md".to_string(), "---\ntype: uml.DataType\ntitle: Money\n---\n# Money\n".to_string()),
        ];
        let err = apply(&b, &[Op::NodeRm { slug:"money".into(), cascade: false }]).unwrap_err();
        assert!(err.reason.contains("referenced by"));
        assert!(err.reason.contains("order"));
        let out = apply(&b, &[Op::NodeRm { slug:"money".into(), cascade: true }]).unwrap();
        assert!(out.iter().all(|(p, _)| slug_of(p) != "money"));
    }

    #[test]
    fn node_rm_deletes_unreferenced() {
        let b = vec![("a/lonely.md".to_string(), "---\ntype: uml.Class\ntitle: Lonely\n---\n# Lonely\n".to_string())];
        let out = apply(&b, &[Op::NodeRm { slug:"lonely".into(), cascade: false }]).unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn referrers_includes_layout_link_reference() {
        let b = vec![
            ("a/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string()),
            ("a/diagram.md".to_string(),
             "---\ntype: Diagram\ntitle: D\nprofile: uml-domain\n---\n# D\n\n## Layout\n- [Order](./order.md) with collapsed\n".to_string()),
        ];
        let refs = referrers(&b, "order");
        assert!(refs.contains(&"diagram".to_string()), "diagram referencing 'order' only via a Layout link must be reported: {refs:?}");
    }

    #[test]
    fn referrers_includes_layout_bare_reference() {
        let b = vec![
            ("a/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string()),
            ("a/customer.md".to_string(), "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n".to_string()),
            ("a/diagram.md".to_string(),
             "---\ntype: Diagram\ntitle: D\nprofile: uml-domain\n---\n# D\n\n## Layout\n- order left of customer\n".to_string()),
        ];
        let refs = referrers(&b, "order");
        assert!(refs.contains(&"diagram".to_string()), "diagram referencing 'order' only via a bare Layout operand must be reported: {refs:?}");
    }

    // ---- full bundle-path id resolution (matches the parse/graph layer's
    // `okf::id_of` keying, not just a bare same-directory basename) ----

    #[test]
    fn find_doc_resolves_full_path_id_for_a_nested_doc() {
        let b = vec![("shop/order.md".to_string(),
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId\n".to_string())];
        let out = apply(&b, &[attr_add("shop/order", "total", "Money")]).unwrap();
        assert!(out[0].1.contains("- total: Money"), "op.node addressed by full-path id must resolve: {:?}", out[0].1);
    }

    #[test]
    fn attr_add_links_a_known_slug_addressed_by_full_path_id() {
        let b = vec![
            ("a/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string()),
            ("a/money.md".to_string(), "---\ntype: uml.DataType\ntitle: Money\n---\n# Money\n".to_string()),
        ];
        // both the node being edited and the type token are passed as full-path ids
        let out = apply(&b, &[attr_add("a/order", "total", "a/money")]).unwrap();
        assert!(out[0].1.contains("- total: [Money](./money.md)"),
            "type token resolved by full-path id must still emit a bare same-directory href: {:?}", out[0].1);
    }

    #[test]
    fn node_set_resolves_nested_doc_by_full_path_id() {
        let b = vec![("shop/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string())];
        let out = apply(&b, &[Op::NodeSet {
            slug: "shop/order".into(), title: Some("Sales Order".into()), description: None,
            stereotype: None, abstract_: None, ty: None,
        }]).unwrap();
        assert_eq!(out[0].0, "shop/order.md");
        assert!(out[0].1.contains("title: \"Sales Order\""));
    }

    #[test]
    fn node_rm_resolves_nested_doc_by_full_path_id_and_referrers_stay_bare() {
        let b = vec![
            ("shop/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Relationships\n- depends [Money](./money.md)\n".to_string()),
            ("shop/money.md".to_string(), "---\ntype: uml.DataType\ntitle: Money\n---\n# Money\n".to_string()),
        ];
        let err = apply(&b, &[Op::NodeRm { slug: "shop/money".into(), cascade: false }]).unwrap_err();
        assert!(err.reason.contains("referenced by"));
        assert!(err.reason.contains("order"));
        let out = apply(&b, &[Op::NodeRm { slug: "shop/money".into(), cascade: true }]).unwrap();
        assert!(out.iter().all(|(p, _)| slug_of(p) != "money"));
    }

    #[test]
    fn node_new_collision_check_is_scoped_to_the_destination_path_not_global() {
        // A same-basename doc already exists in a different directory — this
        // must NOT collide (full-path keying is what allows same-basename
        // docs to coexist across directories in the first place).
        let b = vec![("shop/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string())];
        let out = apply(&b, &[Op::NodeNew {
            slug: "order".into(), dir: "billing".into(), ty: ClassifierType::parse("uml.Class"),
            title: "Order".into(), stereotype: vec![], description: None, abstract_: false,
        }]).unwrap();
        assert!(out.iter().any(|(p, _)| p == "billing/order.md"));
        // same directory + same basename must still collide
        let dup = apply(&b, &[Op::NodeNew {
            slug: "order".into(), dir: "shop".into(), ty: ClassifierType::parse("uml.Class"),
            title: "X".into(), stereotype: vec![], description: None, abstract_: false,
        }]).unwrap_err();
        assert!(dup.reason.contains("already exists"));
    }

    #[test]
    fn rel_set_resolves_endpoint_target_addressed_by_full_path_id() {
        let b = vec![
            ("shop/order.md".to_string(),
             "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Relationships\n- associates [Customer](./customer.md): 1 to 1\n".to_string()),
            ("shop/customer.md".to_string(), "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n".to_string()),
        ];
        let sel = Selector::Rel {
            source: "shop/order".into(),
            by: RelBy::Endpoint { kind: RelationshipKind::Associates, target: "shop/customer".into() },
        };
        let ends = parse_ends("1 to 1..* customers").unwrap();
        let out = apply(&b, &[Op::RelSet { selector: sel, ends: Some(ends), name: None }]).unwrap();
        let order = &out.iter().find(|(p, _)| p == "shop/order.md").unwrap().1;
        assert!(order.contains("1..* customers"), "endpoint addressed by full-path id must resolve: {order}");
    }

    #[test]
    fn rel_rm_resolves_endpoint_target_addressed_by_full_path_id() {
        let b = vec![
            ("shop/order.md".to_string(),
             "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Relationships\n- associates [Customer](./customer.md): 1 to 1\n".to_string()),
            ("shop/customer.md".to_string(), "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n".to_string()),
        ];
        let sel = Selector::Rel {
            source: "shop/order".into(),
            by: RelBy::Endpoint { kind: RelationshipKind::Associates, target: "shop/customer".into() },
        };
        let out = apply(&b, &[Op::RelRm { selector: sel }]).unwrap();
        let order = &out.iter().find(|(p, _)| p == "shop/order.md").unwrap().1;
        assert!(!order.contains("associates"), "endpoint addressed by full-path id must resolve for removal: {order}");
    }

    fn diagram_doc() -> Bundle {
        vec![("shop/dia.md".to_string(),
            "---\ntype: Diagram\ntitle: D\nprofile: uml-domain\n---\n# D\n".to_string())]
    }

    fn full_display() -> DiagramDisplaySet {
        DiagramDisplaySet {
            show_attributes: false,
            attribute_detail: "name-only".into(),
            show_attribute_visibility: false,
            show_attribute_multiplicity: false,
            max_attributes: Some(6),
            association_labels: "hidden".into(),
            emphasize_multiplicity: true,
            show_stereotype: false,
            stereotype_filter: Some(vec!["entity".into()]),
            stereotype_colors: vec!["entity:#ffedd5".into()],
        }
    }

    #[test]
    fn diagram_set_writes_title_and_note() {
        let out = apply(&diagram_doc(), &[Op::DiagramSet {
            key: "dia".into(), title: Some("Order lifecycle".into()),
            description: Some("Notes for reviewers".into()), display: None,
        }]).unwrap();
        assert!(out[0].1.contains("title: \"Order lifecycle\""));
        assert!(out[0].1.contains("# Order lifecycle"), "H1 kept in sync");
        assert!(out[0].1.contains("description: \"Notes for reviewers\""));
    }

    #[test]
    fn diagram_set_replaces_display_block_and_drops_stale_keys() {
        let set = apply(&diagram_doc(), &[Op::DiagramSet {
            key: "dia".into(), title: None, description: None, display: Some(full_display()),
        }]).unwrap();
        assert!(set[0].1.contains("showAttributes: false"));
        assert!(set[0].1.contains("maxAttributes: 6"));
        assert!(set[0].1.contains("stereotypeFilter: [\"entity\"]"));

        // A second DiagramSet with a display that omits maxAttributes/stereotypeFilter
        // must drop those stale keys entirely (whole-block replace).
        let cleared = apply(&set, &[Op::DiagramSet {
            key: "dia".into(), title: None, description: None,
            display: Some(DiagramDisplaySet {
                max_attributes: None,
                stereotype_filter: None,
                stereotype_colors: vec![],
                ..full_display()
            }),
        }]).unwrap();
        assert!(!cleared[0].1.contains("maxAttributes"), "stale key must be dropped: {}", cleared[0].1);
        assert!(!cleared[0].1.contains("stereotypeFilter"), "stale key must be dropped: {}", cleared[0].1);
        assert!(!cleared[0].1.contains("stereotypeColors"), "stale key must be dropped: {}", cleared[0].1);
    }

    #[test]
    fn diagram_set_on_missing_diagram_errors() {
        let err = apply(&diagram_doc(), &[Op::DiagramSet {
            key: "ghost".into(), title: Some("X".into()), description: None, display: None,
        }]).unwrap_err();
        assert!(err.reason.contains("no document 'ghost'"));
    }

    #[test]
    fn diagram_set_leaves_untouched_fields_alone() {
        let out = apply(&diagram_doc(), &[Op::DiagramSet {
            key: "dia".into(), title: None, description: None, display: None,
        }]).unwrap();
        // A no-op DiagramSet must match plain parse+serialize normalization —
        // i.e. edit_doc's own round-trip introduces no extra drift.
        let normalized = serialize_document(&parse_document(&diagram_doc()[0].1));
        assert_eq!(out[0].1, normalized, "no-op DiagramSet leaves the doc unchanged beyond normal round-trip");
    }

    #[test]
    fn diagram_set_resolves_nested_doc_by_full_path_id() {
        let out = apply(&diagram_doc(), &[Op::DiagramSet {
            key: "shop/dia".into(), title: Some("D2".into()), description: None, display: None,
        }]).unwrap();
        assert_eq!(out[0].0, "shop/dia.md");
        assert!(out[0].1.contains("title: \"D2\""));
    }
}
