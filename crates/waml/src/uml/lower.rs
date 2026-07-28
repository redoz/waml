use super::selector::{render_selector, RelBy, Selector};
use super::{DiagramDisplaySet, FieldEdit, NameSpec};
use crate::edit::EditError;
use crate::frontmatter::{FmValue, Frontmatter};
use crate::model::{
    Attribute, CardinalityVisibility, ElementType, RelEnd, RelationshipKind, TypeRef, Visibility,
};
use crate::multiplicity::Multiplicity;
use crate::okf;
use crate::parse::parse_document;
use crate::serialize::serialize_document;
use crate::source::SourceBundle;
use crate::syntax::{
    Direction, Document, LayoutItem, LayoutStatement, Line, NameRef, Operand, OperandRef,
    ParsedName, ParsedRel, Section,
};

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
pub(crate) fn resolve_index(work: &SourceBundle, target: &str) -> Option<usize> {
    if let Some(i) = work
        .documents()
        .iter()
        .position(|document| okf::id_of(document.path().as_str()) == target)
    {
        return Some(i);
    }
    let mut matches = work
        .documents()
        .iter()
        .enumerate()
        .filter(|(_, document)| slug_of(document.path().as_str()) == target);
    match (matches.next(), matches.next()) {
        (Some((i, _)), None) => Some(i),
        _ => None,
    }
}

/// The bare basename actually embedded in this bundle's same-directory
/// relative hrefs (`./slug.md`) for a resolved target. An unresolved token
/// (a forward reference to a not-yet-existing doc) passes through unchanged.
pub(crate) fn stored_slug(work: &SourceBundle, target: &str) -> String {
    resolve_index(work, target)
        .and_then(|i| work.document_at(i))
        .map(|document| slug_of(document.path().as_str()))
        .unwrap_or_else(|| target.to_string())
}

pub(crate) fn find_doc(work: &SourceBundle, target: &str, op: &str) -> Result<usize, EditError> {
    resolve_index(work, target).ok_or_else(|| EditError::at(op, format!("no document '{target}'")))
}

/// Parse the target file, mutate via `f`, re-serialize canonically.
pub(crate) fn edit_doc<F>(
    work: &mut SourceBundle,
    slug: &str,
    op: &str,
    f: F,
) -> Result<(), EditError>
where
    F: FnOnce(&mut Document) -> Result<(), EditError>,
{
    let i = find_doc(work, slug, op)?;
    let mut doc = parse_document(work.document_at(i).expect("resolved document index").text());
    f(&mut doc)?;
    *work
        .document_at_mut(i)
        .expect("resolved document index")
        .text_mut() = serialize_document(&doc);
    Ok(())
}

/// Get the `## Attributes` list, creating an empty section if absent
/// (canonical serialize re-orders sections, so append position is irrelevant).
pub(crate) fn attrs_mut(doc: &mut Document) -> &mut Vec<Line<Attribute>> {
    if !doc
        .sections
        .iter()
        .any(|s| matches!(s, Section::Attributes(_)))
    {
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

/// Get the `## Layout` list, creating an empty section if absent
/// (canonical serialize re-orders sections, so append position is irrelevant).
pub(crate) fn layout_mut(doc: &mut Document) -> &mut Vec<Line<LayoutItem>> {
    if !doc.sections.iter().any(|s| matches!(s, Section::Layout(_))) {
        doc.sections.push(Section::Layout(Vec::new()));
    }
    doc.sections
        .iter_mut()
        .find_map(|s| match s {
            Section::Layout(l) => Some(l),
            _ => None,
        })
        .expect("layout section just ensured")
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
pub(crate) fn resolve_type(work: &SourceBundle, token: &str) -> TypeRef {
    if let Some(i) = resolve_index(work, token) {
        let document = work.document_at(i).expect("resolved document index");
        let title = parse_document(document.text())
            .frontmatter
            .get_str("title")
            .map(String::from)
            .unwrap_or_else(|| token.to_string());
        TypeRef {
            name: title,
            ref_: Some(slug_of(document.path().as_str())),
        }
    } else {
        TypeRef {
            name: token.to_string(),
            ref_: None,
        }
    }
}

/// Get the `## Relationships` list, creating an empty section if absent
/// (canonical serialize re-orders sections, so append position is irrelevant).
pub(crate) fn rels_mut(doc: &mut Document) -> &mut Vec<Line<ParsedRel>> {
    if !doc
        .sections
        .iter()
        .any(|s| matches!(s, Section::Relationships(_)))
    {
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
pub(crate) fn resolve_title(work: &SourceBundle, slug: &str) -> String {
    resolve_index(work, slug)
        .and_then(|i| {
            parse_document(work.document_at(i)?.text())
                .frontmatter
                .get_str("title")
                .map(String::from)
        })
        .unwrap_or_else(|| slug.to_string())
}

/// Resolve an op's `NameSpec` into the `ParsedName` stored on the document
/// (a `Ref`'s title is resolved against the bundle at apply time).
fn build_name(work: &SourceBundle, spec: &Option<NameSpec>) -> Option<ParsedName> {
    match spec {
        None => None,
        Some(NameSpec::Label(l)) => Some(ParsedName::Label(l.clone())),
        Some(NameSpec::Ref(slug)) => Some(ParsedName::Ref {
            title: resolve_title(work, slug),
            slug: stored_slug(work, slug),
        }),
    }
}

/// `RelBy::Endpoint.target` may be a full bundle-path id (the parse/graph
/// layer's edge key); `ParsedRel.target_slug` is always the bare
/// same-directory-relative href token. Resolve before matching.
fn resolve_rel_by(work: &SourceBundle, by: &RelBy) -> RelBy {
    match by {
        RelBy::Endpoint { kind, target } => RelBy::Endpoint {
            kind: *kind,
            target: stored_slug(work, target),
        },
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
fn rel_target<'a>(selector: &'a Selector, op: &str) -> Result<(&'a str, &'a RelBy), EditError> {
    match selector {
        Selector::Rel { source, by } => Ok((source.as_str(), by)),
        _ => Err(EditError::at(
            op,
            format!(
                "selector '{}' does not address a relationship",
                render_selector(selector)
            ),
        )
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
pub fn referrers(work: &[(String, String)], slug: &str) -> Vec<String> {
    let Ok(source) = SourceBundle::try_from_pairs(work.iter().cloned()) else {
        return Vec::new();
    };
    referrers_source(&source, slug)
}

pub fn referrers_source(work: &SourceBundle, slug: &str) -> Vec<String> {
    // Referring docs store bare same-directory-relative hrefs (`./slug.md`),
    // not full ids — translate `slug` (which may be a full bundle-path id,
    // per `resolve_index`) down to that bare form before matching stored refs.
    let target_idx = resolve_index(work, slug);
    let target = target_idx
        .and_then(|i| work.document_at(i))
        .map(|document| slug_of(document.path().as_str()))
        .unwrap_or_else(|| slug.to_string());
    let mut out = Vec::new();
    for (i, document) in work.documents().iter().enumerate() {
        if Some(i) == target_idx {
            continue;
        }
        let doc = parse_document(document.text());
        let hit = doc.sections.iter().any(|sec| match sec {
            Section::Attributes(attrs) => attrs
                .iter()
                .filter_map(Line::parsed)
                .any(|a| a.ty.ref_.as_deref() == Some(target.as_str())),
            Section::Relationships(rels) => rels.iter().filter_map(Line::parsed).any(|r| {
                r.target_slug == target
                    || matches!(&r.name, Some(ParsedName::Ref { slug: rs, .. }) if rs == &target)
            }),
            Section::Members(block) => {
                fn group_has(g: &crate::syntax::MemberGroup, slug: &str) -> bool {
                    g.members.iter().filter_map(Line::parsed).any(|m| match m {
                        crate::syntax::MemberItem::Member(ml) => ml.slug == slug,
                        crate::syntax::MemberItem::Instance(inst) => inst.classifier.slug == slug,
                    }) || g.children.iter().any(|c| group_has(c, slug))
                }
                block.groups.iter().any(|g| group_has(g, &target))
            }
            Section::Layout(stmts) => {
                fn operand_refs(op: &crate::syntax::Operand, slug: &str) -> bool {
                    use crate::syntax::{NameRef, OperandRef};
                    match &op.ref_ {
                        OperandRef::Name(NameRef::Link { slug: rs, .. }) => rs == slug,
                        OperandRef::Name(NameRef::Bare(s)) => s == slug,
                        OperandRef::InlineGroup { items, .. } => {
                            items.iter().any(|it| operand_refs(it, slug))
                        }
                        OperandRef::Paren(inner) => operand_refs(inner, slug),
                    }
                }
                stmts
                    .iter()
                    .filter_map(Line::parsed)
                    .any(|it| match &it.stmt {
                        crate::syntax::LayoutStatement::Standalone(op) => operand_refs(op, &target),
                        crate::syntax::LayoutStatement::Placement { operands, .. } => {
                            operands.iter().any(|op| operand_refs(op, &target))
                        }
                        crate::syntax::LayoutStatement::Alignment { left, right } => {
                            operand_refs(&left.operand, &target)
                                || operand_refs(&right.operand, &target)
                        }
                    })
            }
            _ => false,
        });
        if hit {
            out.push(slug_of(document.path().as_str()));
        }
    }
    out.sort();
    out.dedup();
    out
}

pub(crate) fn op_attr_add(
    work: &mut SourceBundle,
    node: &str,
    name: &str,
    ty_token: &str,
    multiplicity: &Option<Multiplicity>,
    visibility: Option<Visibility>,
) -> Result<(), EditError> {
    let ty = resolve_type(work, ty_token);
    edit_doc(work, node, "attr.add", |doc| {
        let attrs = attrs_mut(doc);
        if attrs
            .iter()
            .filter_map(Line::parsed)
            .any(|a| a.name == name)
        {
            return Err(EditError::at(
                "attr.add",
                format!("attribute '{name}' already exists in {node}"),
            ));
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
pub(crate) fn op_attr_set(
    work: &mut SourceBundle,
    node: &str,
    name: &str,
    ty_token: &Option<String>,
    multiplicity: &FieldEdit<Multiplicity>,
    visibility: Option<Visibility>,
    rename: &Option<String>,
) -> Result<(), EditError> {
    let ty = ty_token.as_ref().map(|t| resolve_type(work, t));
    edit_doc(work, node, "attr.set", |doc| {
        let attrs = attrs_mut(doc);
        if let Some(new) = rename {
            if new != name
                && attrs
                    .iter()
                    .filter_map(Line::parsed)
                    .any(|a| a.name == *new)
            {
                return Err(EditError::at(
                    "attr.set",
                    format!("attribute '{new}' already exists in {node}"),
                ));
            }
        }
        let a = attrs
            .iter_mut()
            .filter_map(Line::parsed_mut)
            .find(|a| a.name == name)
            .ok_or_else(|| EditError::at("attr.set", format!("no attribute '{name}' in {node}")))?;
        if let Some(t) = ty {
            a.ty = t;
        }
        match multiplicity {
            FieldEdit::Unchanged => {}
            FieldEdit::Clear => a.multiplicity = None,
            FieldEdit::Set(value) => a.multiplicity = Some(value.clone()),
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

pub(crate) fn op_attr_rm(work: &mut SourceBundle, node: &str, name: &str) -> Result<(), EditError> {
    edit_doc(work, node, "attr.rm", |doc| {
        let attrs = attrs_mut(doc);
        let before = attrs.len();
        attrs.retain(|a| a.parsed().map_or(true, |x| x.name != name));
        if attrs.len() == before {
            return Err(EditError::at(
                "attr.rm",
                format!("no attribute '{name}' in {node}"),
            ));
        }
        Ok(())
    })
}

pub(crate) fn op_value_add(
    work: &mut SourceBundle,
    node: &str,
    literal: &str,
) -> Result<(), EditError> {
    edit_doc(work, node, "value.add", |doc| {
        let values = values_mut(doc);
        if values.iter().filter_map(Line::parsed).any(|v| v == literal) {
            return Err(EditError::at(
                "value.add",
                format!("value '{literal}' already in {node}"),
            ));
        }
        values.push(Line::Parsed(literal.to_string()));
        Ok(())
    })
}

pub(crate) fn op_value_rm(
    work: &mut SourceBundle,
    node: &str,
    literal: &str,
) -> Result<(), EditError> {
    edit_doc(work, node, "value.rm", |doc| {
        let values = values_mut(doc);
        let before = values.len();
        values.retain(|l| l.parsed().map_or(true, |v| v != literal));
        if values.len() == before {
            return Err(EditError::at(
                "value.rm",
                format!("no value '{literal}' in {node}"),
            ));
        }
        Ok(())
    })
}

pub(crate) fn op_rel_add(
    work: &mut SourceBundle,
    source: &str,
    kind: RelationshipKind,
    target: &str,
    name: &Option<NameSpec>,
    ends: &Option<(RelEnd, RelEnd)>,
) -> Result<(), EditError> {
    if kind.is_ended() != ends.is_some() {
        let msg = if kind.is_ended() {
            format!("relationship '{}' requires ends", kind.as_str())
        } else {
            format!("relationship '{}' does not take ends", kind.as_str())
        };
        return Err(EditError::at("rel.add", msg));
    }
    let target_title = resolve_title(work, target);
    let target_ref = stored_slug(work, target);
    let name = build_name(work, name);
    let ends = ends.clone();
    edit_doc(work, source, "rel.add", |doc| {
        let rels = rels_mut(doc);
        if rels
            .iter()
            .filter_map(Line::parsed)
            .any(|r| r.kind == kind && r.target_slug == target_ref)
        {
            return Err(EditError::at(
                "rel.add",
                format!(
                    "relationship '{} {target}' already exists in {source}",
                    kind.as_str()
                ),
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

pub(crate) fn op_rel_set(
    work: &mut SourceBundle,
    selector: &Selector,
    ends: &Option<(RelEnd, RelEnd)>,
    name: &Option<NameSpec>,
) -> Result<(), EditError> {
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
            .ok_or_else(|| {
                EditError::at("rel.set", format!("no relationship '{disp}'")).with_sel(disp.clone())
            })?;
        if let Some((f, t)) = new_ends {
            if !r.kind.is_ended() {
                return Err(EditError::at(
                    "rel.set",
                    format!("'{}' does not take ends", r.kind.as_str()),
                ));
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

pub(crate) fn op_rel_rm(work: &mut SourceBundle, selector: &Selector) -> Result<(), EditError> {
    let (source, by) = rel_target(selector, "rel.rm")?;
    let (source, by) = (source.to_string(), resolve_rel_by(work, by));
    let disp = render_selector(selector);
    edit_doc(work, &source, "rel.rm", |doc| {
        let rels = rels_mut(doc);
        let before = rels.len();
        rels.retain(|r| r.parsed().map_or(true, |x| !rel_matches(x, &by)));
        if rels.len() == before {
            return Err(
                EditError::at("rel.rm", format!("no relationship '{disp}'")).with_sel(disp.clone())
            );
        }
        Ok(())
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn op_node_new(
    work: &mut SourceBundle,
    slug: &str,
    dir: &str,
    ty: &ElementType,
    title: &str,
    stereotype: &[String],
    description: &Option<String>,
    abstract_: bool,
) -> Result<(), EditError> {
    let path = if dir.is_empty() {
        format!("{slug}.md")
    } else {
        format!("{dir}/{slug}.md")
    };
    if work
        .documents()
        .iter()
        .any(|document| okf::id_of(document.path().as_str()) == okf::id_of(&path))
    {
        return Err(EditError::at(
            "node.new",
            format!("document '{slug}' already exists"),
        ));
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
    work.push_document(path, serialize_document(&doc))
        .map_err(|error| EditError::at("node.new", error.to_string()))?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn op_node_set(
    work: &mut SourceBundle,
    slug: &str,
    title: &Option<String>,
    description: &Option<String>,
    stereotype: &Option<Vec<String>>,
    abstract_: &Option<bool>,
    ty: &Option<ElementType>,
) -> Result<(), EditError> {
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
    // `attributeDetail` stays listed so a legacy key is stripped on the next
    // whole-block rewrite, even though we only ever emit `showType` now.
    "showAttributes",
    "showType",
    "attributeDetail",
    "showAttributeVisibility",
    "showAttributeMultiplicity",
    "maxAttributes",
    "showRoles",
    "cardinality",
    "showLabels",
    "showStereotype",
    "stereotypeFilter",
    "stereotypeColors",
];

pub(crate) fn op_diagram_set(
    work: &mut SourceBundle,
    key: &str,
    title: &Option<String>,
    description: &Option<String>,
    clear_description: bool,
    display: &Option<DiagramDisplaySet>,
) -> Result<(), EditError> {
    if description
        .as_deref()
        .is_some_and(|description| description.contains('\n') || description.contains('\r'))
    {
        return Err(
            EditError::at("diagram.set", "description must be one line").with_sel(key.to_string())
        );
    }
    edit_doc(work, key, "diagram.set", |doc| {
        if let Some(t) = title {
            fm_set(&mut doc.frontmatter, "title", FmValue::Str(t.clone()));
            doc.title = t.clone();
        }
        if clear_description {
            doc.frontmatter
                .entries
                .retain(|(key, _)| key != "description");
        } else if let Some(d) = description {
            fm_set(&mut doc.frontmatter, "description", FmValue::Str(d.clone()));
        }
        if let Some(ds) = display {
            // Whole-block replace: drop every display key first so a field left
            // absent on this DiagramSet (e.g. maxAttributes not present on `ds`)
            // clears back to its tri-state-absent wire representation, then
            // re-set exactly the keys this fully-resolved display carries.
            doc.frontmatter
                .entries
                .retain(|(k, _)| !DISPLAY_KEYS.contains(&k.as_str()));
            fm_set(
                &mut doc.frontmatter,
                "showAttributes",
                FmValue::Bool(ds.show_attributes),
            );
            fm_set(
                &mut doc.frontmatter,
                "showType",
                FmValue::Bool(ds.show_type),
            );
            fm_set(
                &mut doc.frontmatter,
                "showAttributeVisibility",
                FmValue::Bool(ds.show_attribute_visibility),
            );
            fm_set(
                &mut doc.frontmatter,
                "showAttributeMultiplicity",
                FmValue::Bool(ds.cardinality.legacy_attribute_gate()),
            );
            if let Some(max) = ds.max_attributes {
                fm_set(
                    &mut doc.frontmatter,
                    "maxAttributes",
                    FmValue::Num(max as f64),
                );
            }
            fm_set(
                &mut doc.frontmatter,
                "showRoles",
                FmValue::Bool(ds.show_roles),
            );
            fm_set(
                &mut doc.frontmatter,
                "cardinality",
                FmValue::Str(
                    match ds.cardinality {
                        CardinalityVisibility::Off => "off",
                        CardinalityVisibility::Explicit => "explicit",
                        CardinalityVisibility::All => "all",
                    }
                    .into(),
                ),
            );
            fm_set(
                &mut doc.frontmatter,
                "showLabels",
                FmValue::Bool(ds.show_labels),
            );
            fm_set(
                &mut doc.frontmatter,
                "showStereotype",
                FmValue::Bool(ds.show_stereotype),
            );
            if let Some(filter) = &ds.stereotype_filter {
                fm_set(&mut doc.frontmatter, "stereotypeFilter", str_list(filter));
            }
            if !ds.stereotype_colors.is_empty() {
                fm_set(
                    &mut doc.frontmatter,
                    "stereotypeColors",
                    str_list(&ds.stereotype_colors),
                );
            }
        }
        Ok(())
    })
}

/// A `[title](./slug.md)` operand with no axis/hints.
fn link_operand(title: &str, slug: &str) -> Operand {
    Operand {
        ref_: OperandRef::Name(NameRef::Link {
            title: title.to_string(),
            slug: slug.to_string(),
        }),
        axis: None,
        hints: vec![],
    }
}

/// The bare slug an operand references (Link href stem or bare name), if any.
fn operand_slug(op: &Operand) -> Option<&str> {
    match &op.ref_ {
        OperandRef::Name(NameRef::Link { slug, .. }) => Some(slug.as_str()),
        OperandRef::Name(NameRef::Bare(s)) => Some(s.as_str()),
        _ => None,
    }
}

/// Horizontal axis = Left/Right; Vertical = Above/Below.
///
/// A 2-operand `[subject] <dir> [reference]` placement on the given axis.
/// True when `stmt` is a 2-operand placement for the UNORDERED `{subject,
/// reference}` pair, on ANY axis and in EITHER operand order. Placement is
/// one-relation-per-pair: authoring a new direction rewrites whatever relation
/// the pair already had (a pair can't hold both `left of` and `below` -- the
/// solver center-aligns the cross axis, so two
/// relations on one pair mutually conflict).
fn placement_matches(stmt: &LayoutStatement, subject: &str, reference: &str) -> bool {
    let LayoutStatement::Placement {
        operands,
        directions,
    } = stmt
    else {
        return false;
    };
    let (a, b) = (operand_slug(&operands[0]), operand_slug(&operands[1]));
    operands.len() == 2
        && directions.len() == 1
        && ((a == Some(subject) && b == Some(reference))
            || (a == Some(reference) && b == Some(subject)))
}

pub(crate) fn op_place_set(
    work: &mut SourceBundle,
    diagram: &str,
    subject_title: &str,
    subject_slug: &str,
    reference_title: &str,
    reference_slug: &str,
    directions: &[Direction],
) -> Result<(), EditError> {
    let subject_title = subject_title.to_string();
    let subject_slug = subject_slug.to_string();
    let reference_title = reference_title.to_string();
    let reference_slug = reference_slug.to_string();
    let directions = directions.to_vec();
    edit_doc(work, diagram, "place.set", |doc| {
        let layout = layout_mut(doc);
        // One relation per pair: drop ANY existing placement for this
        // (subject, reference) pair on either axis, then author the new one(s).
        // Re-dragging a node onto a target it already relates to REWRITES the
        // relation rather than stacking a conflicting cross-axis one.
        layout.retain(|line| match line.parsed() {
            Some(item) => !placement_matches(&item.stmt, &subject_slug, &reference_slug),
            None => true,
        });
        for dir in &directions {
            let stmt = LayoutStatement::Placement {
                operands: vec![
                    link_operand(&subject_title, &subject_slug),
                    link_operand(&reference_title, &reference_slug),
                ],
                directions: vec![*dir],
            };
            layout.push(Line::Parsed(LayoutItem { line: 0, stmt }));
        }
        Ok(())
    })
}

pub(crate) fn op_place_rm(
    work: &mut SourceBundle,
    diagram: &str,
    subject_slug: &str,
    reference_slug: &str,
) -> Result<(), EditError> {
    let subject_slug = subject_slug.to_string();
    let reference_slug = reference_slug.to_string();
    edit_doc(work, diagram, "place.rm", |doc| {
        let layout = layout_mut(doc);
        layout.retain(|line| match line.parsed() {
            Some(item) => !placement_matches(&item.stmt, &subject_slug, &reference_slug),
            None => true,
        });
        Ok(())
    })
}

pub(crate) fn op_node_rm(
    work: &mut SourceBundle,
    slug: &str,
    cascade: bool,
) -> Result<(), EditError> {
    let i = find_doc(work, slug, "node.rm")?;
    if !cascade {
        let refs = referrers_source(work, slug);
        if !refs.is_empty() {
            return Err(EditError::at(
                "node.rm",
                format!(
                    "'{slug}' referenced by: {} (use --cascade)",
                    refs.join(", ")
                ),
            ));
        }
    }
    work.remove_document(i);
    Ok(())
}
