use serde::{Deserialize, Serialize};
use waml::edit::{Batch, Step};
use waml::layout::Direction;
use waml::model::{
    parse_ends, render_ends, CardinalityVisibility, ElementType, RelEnd, RelationshipKind,
    Visibility,
};
use waml::multiplicity::Multiplicity;
use waml::source::SourceBundle;
use waml::uml::selector::RelBy;
use waml::uml::{DiagramDisplaySet, FieldEdit, NameSpec};
use waml::{okf, uml};

fn one() -> u32 {
    1
}

fn default_true() -> bool {
    true
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "op")]
pub enum OpDto {
    #[serde(rename = "node.new")]
    NodeNew {
        #[serde(default = "one")]
        v: u32,
        slug: String,
        #[serde(default)]
        dir: String,
        ty: String,
        title: String,
        #[serde(default)]
        stereotype: Vec<String>,
        #[serde(default)]
        desc: Option<String>,
        #[serde(default, rename = "abstract")]
        abstract_: bool,
    },
    #[serde(rename = "node.rename")]
    NodeRename {
        #[serde(default = "one")]
        v: u32,
        from: String,
        to: String,
    },
    #[serde(rename = "node.set")]
    NodeSet {
        #[serde(default = "one")]
        v: u32,
        slug: String,
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        desc: Option<String>,
        #[serde(default)]
        stereotype: Option<Vec<String>>,
        #[serde(default, rename = "abstract")]
        abstract_: Option<bool>,
        #[serde(default)]
        ty: Option<String>,
    },
    #[serde(rename = "node.rm")]
    NodeRm {
        #[serde(default = "one")]
        v: u32,
        slug: String,
        #[serde(default)]
        cascade: bool,
    },
    #[serde(rename = "attr.add")]
    AttrAdd {
        #[serde(default = "one")]
        v: u32,
        node: String,
        name: String,
        ty: String,
        #[serde(default)]
        mult: Option<String>,
        #[serde(default)]
        vis: Option<String>,
    },
    #[serde(rename = "attr.set")]
    AttrSet {
        #[serde(default = "one")]
        v: u32,
        node: String,
        name: String,
        #[serde(default)]
        ty: Option<String>,
        #[serde(default, skip_serializing_if = "FieldEdit::is_unchanged")]
        mult: FieldEdit<String>,
        #[serde(default)]
        vis: Option<String>,
        #[serde(default)]
        rename: Option<String>,
    },
    #[serde(rename = "attr.rm")]
    AttrRm {
        #[serde(default = "one")]
        v: u32,
        node: String,
        name: String,
    },
    #[serde(rename = "value.add")]
    ValueAdd {
        #[serde(default = "one")]
        v: u32,
        node: String,
        literal: String,
    },
    #[serde(rename = "value.rm")]
    ValueRm {
        #[serde(default = "one")]
        v: u32,
        node: String,
        literal: String,
    },
    #[serde(rename = "rel.add")]
    RelAdd {
        #[serde(default = "one")]
        v: u32,
        source: String,
        kind: String,
        target: String,
        #[serde(default, rename = "as")]
        as_label: Option<String>,
        #[serde(default)]
        as_ref: Option<String>,
        #[serde(default)]
        ends: Option<String>,
    },
    #[serde(rename = "rel.set")]
    RelSet {
        #[serde(default = "one")]
        v: u32,
        source: String,
        #[serde(default)]
        kind: Option<String>,
        #[serde(default)]
        target: Option<String>,
        #[serde(default, rename = "as")]
        as_sel: Option<String>,
        #[serde(default)]
        ends: Option<String>,
        #[serde(default, rename = "set_as")]
        set_label: Option<String>,
        #[serde(default)]
        set_as_ref: Option<String>,
    },
    #[serde(rename = "rel.rm")]
    RelRm {
        #[serde(default = "one")]
        v: u32,
        source: String,
        #[serde(default)]
        kind: Option<String>,
        #[serde(default)]
        target: Option<String>,
        #[serde(default, rename = "as")]
        as_sel: Option<String>,
    },
    #[serde(rename = "pkg.move")]
    PkgMove {
        #[serde(default = "one")]
        v: u32,
        slug: String,
        to_dir: String,
    },
    #[serde(rename = "pkg.rename")]
    PkgRename {
        #[serde(default = "one")]
        v: u32,
        from: String,
        to: String,
    },
    #[serde(rename = "pkg.delete")]
    PkgDelete {
        #[serde(default = "one")]
        v: u32,
        path: String,
        #[serde(default)]
        cascade: bool,
    },
    #[serde(rename = "pkg.reorder")]
    PkgReorder {
        #[serde(default = "one")]
        v: u32,
        path: String,
        #[serde(default)]
        order: Vec<String>,
    },
    #[serde(rename = "pkg.sort")]
    PkgSort {
        #[serde(default = "one")]
        v: u32,
        path: String,
    },
    #[serde(rename = "pkg.retitle")]
    PkgRetitle {
        #[serde(default = "one")]
        v: u32,
        path: String,
        title: String,
    },
    #[serde(rename = "pkg.insert")]
    PkgInsert {
        #[serde(default = "one")]
        v: u32,
        parent_path: String,
        name: String,
        #[serde(default)]
        docs: Vec<(String, String)>,
    },
    #[serde(rename = "place.set")]
    PlaceSet {
        #[serde(default = "one")]
        v: u32,
        diagram: String,
        subject_title: String,
        subject_slug: String,
        reference_title: String,
        reference_slug: String,
        #[serde(default)]
        directions: Vec<Direction>,
    },
    #[serde(rename = "place.rm")]
    PlaceRm {
        #[serde(default = "one")]
        v: u32,
        diagram: String,
        subject_slug: String,
        reference_slug: String,
    },
    #[serde(rename = "diagram.set")]
    DiagramSet {
        #[serde(default = "one")]
        v: u32,
        key: String,
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        desc: Option<String>,
        #[serde(default, rename = "clearDesc", skip_serializing_if = "is_false")]
        clear_desc: bool,
        #[serde(default)]
        display: Option<DisplayDto>,
    },
}

/// A fully-specified display block on the wire. Attribute cardinality uses the
/// enum plus its derived legacy gate; relationship cardinality remains an
/// independent boolean.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayDto {
    pub show_attributes: bool,
    pub show_type: bool,
    pub show_attribute_visibility: bool,
    pub show_attribute_multiplicity: bool,
    #[serde(default)]
    pub max_attributes: Option<u32>,
    pub show_roles: bool,
    pub cardinality: CardinalityVisibility,
    #[serde(default = "default_true")]
    pub show_cardinality: bool,
    pub show_labels: bool,
    pub show_stereotype: bool,
    #[serde(default)]
    pub stereotype_filter: Option<Vec<String>>,
    #[serde(default)]
    pub stereotype_colors: Vec<String>,
}

fn display_dto_to_set(d: &DisplayDto) -> DiagramDisplaySet {
    DiagramDisplaySet {
        show_attributes: d.show_attributes,
        show_type: d.show_type,
        show_attribute_visibility: d.show_attribute_visibility,
        max_attributes: d.max_attributes,
        show_roles: d.show_roles,
        cardinality: d.cardinality,
        show_cardinality: d.show_cardinality,
        show_labels: d.show_labels,
        show_stereotype: d.show_stereotype,
        stereotype_filter: d.stereotype_filter.clone(),
        stereotype_colors: d.stereotype_colors.clone(),
    }
}

fn display_set_to_dto(ds: &DiagramDisplaySet) -> DisplayDto {
    DisplayDto {
        show_attributes: ds.show_attributes,
        show_type: ds.show_type,
        show_attribute_visibility: ds.show_attribute_visibility,
        show_attribute_multiplicity: ds.cardinality.legacy_attribute_gate(),
        max_attributes: ds.max_attributes,
        show_roles: ds.show_roles,
        cardinality: ds.cardinality,
        show_cardinality: ds.show_cardinality,
        show_labels: ds.show_labels,
        show_stereotype: ds.show_stereotype,
        stereotype_filter: ds.stereotype_filter.clone(),
        stereotype_colors: ds.stereotype_colors.clone(),
    }
}

fn check_v(v: u32, op: &str) -> Result<(), String> {
    if v != 1 {
        return Err(format!("unsupported version {v} for op '{op}'"));
    }
    Ok(())
}
fn mult_opt(s: &Option<String>) -> Result<Option<Multiplicity>, String> {
    s.as_ref()
        .map(|m| Multiplicity::parse(m).ok_or_else(|| format!("bad multiplicity '{m}'")))
        .transpose()
}
fn mult_edit(value: &FieldEdit<String>) -> Result<FieldEdit<Multiplicity>, String> {
    match value {
        FieldEdit::Unchanged => Ok(FieldEdit::Unchanged),
        FieldEdit::Clear => Ok(FieldEdit::Clear),
        FieldEdit::Set(value) => Multiplicity::parse(value)
            .map(FieldEdit::Set)
            .ok_or_else(|| format!("bad multiplicity '{value}'")),
    }
}
fn vis_opt(s: &Option<String>) -> Result<Option<Visibility>, String> {
    match s {
        None => Ok(None),
        Some(c) => {
            let ch = c.chars().next().ok_or("empty visibility")?;
            Visibility::from_marker(ch)
                .map(Some)
                .ok_or_else(|| format!("bad visibility '{c}'"))
        }
    }
}
fn ends_opt(s: &Option<String>) -> Result<Option<(RelEnd, RelEnd)>, String> {
    s.as_ref()
        .map(|e| parse_ends(e).ok_or_else(|| format!("bad ends '{e}'")))
        .transpose()
}
fn kind_req(s: &str) -> Result<RelationshipKind, String> {
    RelationshipKind::parse(s).ok_or_else(|| format!("unknown verb '{s}'"))
}
fn name_of(label: &Option<String>, as_ref: &Option<String>) -> Option<NameSpec> {
    label
        .as_ref()
        .map(|l| NameSpec::Label(l.clone()))
        .or_else(|| as_ref.as_ref().map(|s| NameSpec::Ref(s.clone())))
}
fn rel_sel(
    source: &str,
    kind: &Option<String>,
    target: &Option<String>,
    as_sel: &Option<String>,
) -> Result<uml::RelationshipSelector, String> {
    let by = match (kind, target, as_sel) {
        (Some(k), Some(t), _) => RelBy::Endpoint {
            kind: kind_req(k)?,
            target: t.clone(),
        },
        (_, _, Some(n)) => RelBy::Named(n.clone()),
        _ => return Err("relationship selector needs kind+target or as".into()),
    };
    Ok(uml::RelationshipSelector {
        source: source.to_string(),
        by,
    })
}

fn directory(path: &str) -> Result<okf::DirectoryAddress, String> {
    let path = path.trim_matches('/');
    okf::DirectoryAddress::parse(if path.is_empty() {
        "/".to_string()
    } else {
        format!("/{path}")
    })
    .map_err(|error| error.to_string())
}

pub fn to_batch(dtos: &[OpDto]) -> Result<Batch, String> {
    dtos.iter()
        .enumerate()
        .map(|(index, dto)| {
            dto.to_step()
                .map_err(|error| format!("op {index}: {error}"))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Batch::new)
}

impl OpDto {
    pub fn to_step(&self) -> Result<Step, String> {
        Ok(match self {
            OpDto::NodeNew {
                v,
                slug,
                dir,
                ty,
                title,
                stereotype,
                desc,
                abstract_,
            } => {
                check_v(*v, "node.new")?;
                Step::Uml(uml::Op::ClassifierNew {
                    slug: slug.clone(),
                    directory: directory(dir)?,
                    ty: ElementType::parse(ty),
                    title: title.clone(),
                    stereotype: stereotype.clone(),
                    description: desc.clone(),
                    abstract_: *abstract_,
                })
            }
            OpDto::NodeRename { v, from, to } => {
                check_v(*v, "node.rename")?;
                Step::Uml(uml::Op::ClassifierRename {
                    from: from.clone(),
                    to: to.clone(),
                })
            }
            OpDto::NodeSet {
                v,
                slug,
                title,
                desc,
                stereotype,
                abstract_,
                ty,
            } => {
                check_v(*v, "node.set")?;
                Step::Uml(uml::Op::ClassifierSet {
                    id: slug.clone(),
                    title: title.clone(),
                    description: desc.clone(),
                    stereotype: stereotype.clone(),
                    abstract_: *abstract_,
                    ty: ty.as_ref().map(|value| ElementType::parse(value)),
                })
            }
            OpDto::NodeRm { v, slug, cascade } => {
                check_v(*v, "node.rm")?;
                Step::Uml(uml::Op::ClassifierRemove {
                    id: slug.clone(),
                    cascade: *cascade,
                })
            }
            OpDto::AttrAdd {
                v,
                node,
                name,
                ty,
                mult,
                vis,
            } => {
                check_v(*v, "attr.add")?;
                Step::Uml(uml::Op::AttributeAdd {
                    node: node.clone(),
                    name: name.clone(),
                    ty_token: ty.clone(),
                    multiplicity: mult_opt(mult)?,
                    visibility: vis_opt(vis)?,
                })
            }
            OpDto::AttrSet {
                v,
                node,
                name,
                ty,
                mult,
                vis,
                rename,
            } => {
                check_v(*v, "attr.set")?;
                Step::Uml(uml::Op::AttributeSet {
                    node: node.clone(),
                    name: name.clone(),
                    ty_token: ty.clone(),
                    multiplicity: mult_edit(mult)?,
                    visibility: vis_opt(vis)?,
                    rename: rename.clone(),
                })
            }
            OpDto::AttrRm { v, node, name } => {
                check_v(*v, "attr.rm")?;
                Step::Uml(uml::Op::AttributeRemove {
                    node: node.clone(),
                    name: name.clone(),
                })
            }
            OpDto::ValueAdd { v, node, literal } => {
                check_v(*v, "value.add")?;
                Step::Uml(uml::Op::ValueAdd {
                    node: node.clone(),
                    literal: literal.clone(),
                })
            }
            OpDto::ValueRm { v, node, literal } => {
                check_v(*v, "value.rm")?;
                Step::Uml(uml::Op::ValueRemove {
                    node: node.clone(),
                    literal: literal.clone(),
                })
            }
            OpDto::RelAdd {
                v,
                source,
                kind,
                target,
                as_label,
                as_ref,
                ends,
            } => {
                check_v(*v, "rel.add")?;
                Step::Uml(uml::Op::RelationshipAdd {
                    source: source.clone(),
                    kind: kind_req(kind)?,
                    target: target.clone(),
                    name: name_of(as_label, as_ref),
                    ends: ends_opt(ends)?,
                })
            }
            OpDto::RelSet {
                v,
                source,
                kind,
                target,
                as_sel,
                ends,
                set_label,
                set_as_ref,
            } => {
                check_v(*v, "rel.set")?;
                Step::Uml(uml::Op::RelationshipSet {
                    selector: rel_sel(source, kind, target, as_sel)?,
                    ends: ends_opt(ends)?,
                    name: name_of(set_label, set_as_ref),
                })
            }
            OpDto::RelRm {
                v,
                source,
                kind,
                target,
                as_sel,
            } => {
                check_v(*v, "rel.rm")?;
                Step::Uml(uml::Op::RelationshipRemove {
                    selector: rel_sel(source, kind, target, as_sel)?,
                })
            }
            OpDto::PkgMove { v, slug, to_dir } => {
                check_v(*v, "pkg.move")?;
                Step::Okf(okf::Op::ConceptMove {
                    id: slug.clone(),
                    to_directory: directory(to_dir)?,
                })
            }
            OpDto::PkgRename { v, from, to } => {
                check_v(*v, "pkg.rename")?;
                let from_parent = from.rsplit_once('/').map_or("", |(parent, _)| parent);
                let to_parent = to.rsplit_once('/').map_or("", |(parent, _)| parent);
                if from_parent == to_parent {
                    Step::Okf(okf::Op::DirectoryRename {
                        directory: directory(from)?,
                        name: to.rsplit('/').next().unwrap_or(to).to_string(),
                    })
                } else {
                    Step::Okf(okf::Op::DirectoryMove {
                        directory: directory(from)?,
                        to_parent: directory(to_parent)?,
                        name: Some(to.rsplit('/').next().unwrap_or(to).to_string()),
                    })
                }
            }
            OpDto::PkgDelete { v, path, cascade } => {
                check_v(*v, "pkg.delete")?;
                Step::Okf(okf::Op::DirectoryDelete {
                    directory: directory(path)?,
                    cascade: *cascade,
                })
            }
            OpDto::PkgReorder { v, path, order } => {
                check_v(*v, "pkg.reorder")?;
                Step::Okf(okf::Op::IndexReorder {
                    directory: directory(path)?,
                    order: order.clone(),
                })
            }
            OpDto::PkgSort { v, path } => {
                check_v(*v, "pkg.sort")?;
                Step::Okf(okf::Op::IndexSort {
                    directory: directory(path)?,
                })
            }
            OpDto::PkgRetitle { v, path, title } => {
                check_v(*v, "pkg.retitle")?;
                Step::Okf(okf::Op::IndexRetitle {
                    directory: directory(path)?,
                    title: title.clone(),
                })
            }
            OpDto::PkgInsert {
                v,
                parent_path,
                name,
                docs,
            } => {
                check_v(*v, "pkg.insert")?;
                Step::Okf(okf::Op::BundleImport {
                    parent: directory(parent_path)?,
                    name: name.clone(),
                    bundle: SourceBundle::try_from_pairs(docs.clone())
                        .map_err(|error| error.to_string())?,
                })
            }
            OpDto::PlaceSet {
                v,
                diagram,
                subject_title,
                subject_slug,
                reference_title,
                reference_slug,
                directions,
            } => {
                check_v(*v, "place.set")?;
                Step::Uml(uml::Op::PlacementSet {
                    diagram: diagram.clone(),
                    subject_title: subject_title.clone(),
                    subject_slug: subject_slug.clone(),
                    reference_title: reference_title.clone(),
                    reference_slug: reference_slug.clone(),
                    directions: directions.clone(),
                })
            }
            OpDto::PlaceRm {
                v,
                diagram,
                subject_slug,
                reference_slug,
            } => {
                check_v(*v, "place.rm")?;
                Step::Uml(uml::Op::PlacementRemove {
                    diagram: diagram.clone(),
                    subject_slug: subject_slug.clone(),
                    reference_slug: reference_slug.clone(),
                })
            }
            OpDto::DiagramSet {
                v,
                key,
                title,
                desc,
                clear_desc,
                display,
            } => {
                check_v(*v, "diagram.set")?;
                Step::Uml(uml::Op::DiagramSet {
                    key: key.clone(),
                    title: title.clone(),
                    description: desc.clone(),
                    clear_description: *clear_desc,
                    display: display.as_ref().map(display_dto_to_set),
                })
            }
        })
    }

    pub fn from_step(step: &Step) -> Self {
        let ends_str =
            |ends: &Option<(RelEnd, RelEnd)>| ends.as_ref().map(|(from, to)| render_ends(from, to));
        let name_parts = |name: &Option<NameSpec>| match name {
            Some(NameSpec::Label(label)) => (Some(label.clone()), None),
            Some(NameSpec::Ref(reference)) => (None, Some(reference.clone())),
            None => (None, None),
        };
        let legacy_directory =
            |address: &okf::DirectoryAddress| address.as_str().trim_start_matches('/').to_string();
        match step {
            Step::Okf(op) => match op {
                okf::Op::ConceptMove { id, to_directory } => OpDto::PkgMove {
                    v: 1,
                    slug: id.clone(),
                    to_dir: legacy_directory(to_directory),
                },
                okf::Op::DirectoryRename { directory, name } => {
                    let from = legacy_directory(directory);
                    let to = match from.rsplit_once('/') {
                        Some((parent, _)) => format!("{parent}/{name}"),
                        None => name.clone(),
                    };
                    OpDto::PkgRename { v: 1, from, to }
                }
                okf::Op::DirectoryMove {
                    directory,
                    to_parent,
                    name,
                } => {
                    let from = legacy_directory(directory);
                    let parent = legacy_directory(to_parent);
                    let name = name
                        .as_deref()
                        .unwrap_or_else(|| from.rsplit('/').next().unwrap_or_default());
                    let to = if parent.is_empty() {
                        name.to_string()
                    } else {
                        format!("{parent}/{name}")
                    };
                    OpDto::PkgRename { v: 1, from, to }
                }
                okf::Op::DirectoryDelete { directory, cascade } => OpDto::PkgDelete {
                    v: 1,
                    path: legacy_directory(directory),
                    cascade: *cascade,
                },
                okf::Op::IndexReorder { directory, order } => OpDto::PkgReorder {
                    v: 1,
                    path: legacy_directory(directory),
                    order: order.clone(),
                },
                okf::Op::IndexSort { directory } => OpDto::PkgSort {
                    v: 1,
                    path: legacy_directory(directory),
                },
                okf::Op::IndexRetitle { directory, title } => OpDto::PkgRetitle {
                    v: 1,
                    path: legacy_directory(directory),
                    title: title.clone(),
                },
                okf::Op::BundleImport {
                    parent,
                    name,
                    bundle,
                } => OpDto::PkgInsert {
                    v: 1,
                    parent_path: legacy_directory(parent),
                    name: name.clone(),
                    docs: bundle.to_pairs(),
                },
            },
            Step::Uml(op) => match op {
                uml::Op::AttributeAdd {
                    node,
                    name,
                    ty_token,
                    multiplicity,
                    visibility,
                } => OpDto::AttrAdd {
                    v: 1,
                    node: node.clone(),
                    name: name.clone(),
                    ty: ty_token.clone(),
                    mult: multiplicity
                        .as_ref()
                        .map(|value| value.as_str().to_string()),
                    vis: visibility.map(|value| value.marker().to_string()),
                },
                uml::Op::AttributeSet {
                    node,
                    name,
                    ty_token,
                    multiplicity,
                    visibility,
                    rename,
                } => OpDto::AttrSet {
                    v: 1,
                    node: node.clone(),
                    name: name.clone(),
                    ty: ty_token.clone(),
                    mult: match multiplicity {
                        FieldEdit::Unchanged => FieldEdit::Unchanged,
                        FieldEdit::Clear => FieldEdit::Clear,
                        FieldEdit::Set(value) => FieldEdit::Set(value.as_str().to_string()),
                    },
                    vis: visibility.map(|value| value.marker().to_string()),
                    rename: rename.clone(),
                },
                uml::Op::AttributeRemove { node, name } => OpDto::AttrRm {
                    v: 1,
                    node: node.clone(),
                    name: name.clone(),
                },
                uml::Op::ValueAdd { node, literal } => OpDto::ValueAdd {
                    v: 1,
                    node: node.clone(),
                    literal: literal.clone(),
                },
                uml::Op::ValueRemove { node, literal } => OpDto::ValueRm {
                    v: 1,
                    node: node.clone(),
                    literal: literal.clone(),
                },
                uml::Op::RelationshipAdd {
                    source,
                    kind,
                    target,
                    name,
                    ends,
                } => {
                    let (as_label, as_ref) = name_parts(name);
                    OpDto::RelAdd {
                        v: 1,
                        source: source.clone(),
                        kind: kind.as_str().to_string(),
                        target: target.clone(),
                        as_label,
                        as_ref,
                        ends: ends_str(ends),
                    }
                }
                uml::Op::RelationshipSet {
                    selector,
                    ends,
                    name,
                } => {
                    let (source, kind, target, as_sel) = sel_parts(selector);
                    let (set_label, set_as_ref) = name_parts(name);
                    OpDto::RelSet {
                        v: 1,
                        source,
                        kind,
                        target,
                        as_sel,
                        ends: ends_str(ends),
                        set_label,
                        set_as_ref,
                    }
                }
                uml::Op::RelationshipRemove { selector } => {
                    let (source, kind, target, as_sel) = sel_parts(selector);
                    OpDto::RelRm {
                        v: 1,
                        source,
                        kind,
                        target,
                        as_sel,
                    }
                }
                uml::Op::ClassifierNew {
                    slug,
                    directory,
                    ty,
                    title,
                    stereotype,
                    description,
                    abstract_,
                } => OpDto::NodeNew {
                    v: 1,
                    slug: slug.clone(),
                    dir: legacy_directory(directory),
                    ty: ty.as_str(),
                    title: title.clone(),
                    stereotype: stereotype.clone(),
                    desc: description.clone(),
                    abstract_: *abstract_,
                },
                uml::Op::ClassifierSet {
                    id,
                    title,
                    description,
                    stereotype,
                    abstract_,
                    ty,
                } => OpDto::NodeSet {
                    v: 1,
                    slug: id.clone(),
                    title: title.clone(),
                    desc: description.clone(),
                    stereotype: stereotype.clone(),
                    abstract_: *abstract_,
                    ty: ty.as_ref().map(|value| value.as_str()),
                },
                uml::Op::ClassifierRemove { id, cascade } => OpDto::NodeRm {
                    v: 1,
                    slug: id.clone(),
                    cascade: *cascade,
                },
                uml::Op::ClassifierRename { from, to } => OpDto::NodeRename {
                    v: 1,
                    from: from.clone(),
                    to: to.clone(),
                },
                uml::Op::DiagramSet {
                    key,
                    title,
                    description,
                    clear_description,
                    display,
                } => OpDto::DiagramSet {
                    v: 1,
                    key: key.clone(),
                    title: title.clone(),
                    desc: description.clone(),
                    clear_desc: *clear_description,
                    display: display.as_ref().map(display_set_to_dto),
                },
                uml::Op::PlacementSet {
                    diagram,
                    subject_title,
                    subject_slug,
                    reference_title,
                    reference_slug,
                    directions,
                } => OpDto::PlaceSet {
                    v: 1,
                    diagram: diagram.clone(),
                    subject_title: subject_title.clone(),
                    subject_slug: subject_slug.clone(),
                    reference_title: reference_title.clone(),
                    reference_slug: reference_slug.clone(),
                    directions: directions.clone(),
                },
                uml::Op::PlacementRemove {
                    diagram,
                    subject_slug,
                    reference_slug,
                } => OpDto::PlaceRm {
                    v: 1,
                    diagram: diagram.clone(),
                    subject_slug: subject_slug.clone(),
                    reference_slug: reference_slug.clone(),
                },
            },
        }
    }
}

/// Decompose a relationship selector into wire fields for compatibility conversion.
#[allow(dead_code)]
fn sel_parts(
    sel: &uml::RelationshipSelector,
) -> (String, Option<String>, Option<String>, Option<String>) {
    match &sel.by {
        RelBy::Endpoint { kind, target } => (
            sel.source.clone(),
            Some(kind.as_str().to_string()),
            Some(target.clone()),
            None,
        ),
        RelBy::Named(name) => (sel.source.clone(), None, None, Some(name.clone())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(line: &str) -> Step {
        let dto: OpDto = serde_json::from_str(line).unwrap();
        dto.to_step().unwrap()
    }

    #[test]
    fn parses_attr_add_line() {
        let op = round_trip(
            r#"{"v":1,"op":"attr.add","node":"order","name":"total","ty":"Money","mult":"0..1"}"#,
        );
        match op {
            Step::Uml(uml::Op::AttributeAdd {
                node,
                name,
                ty_token,
                multiplicity,
                ..
            }) => {
                assert_eq!(
                    (node.as_str(), name.as_str(), ty_token.as_str()),
                    ("order", "total", "Money")
                );
                assert_eq!(
                    multiplicity.as_ref().map(Multiplicity::as_str),
                    Some("0..1")
                );
            }
            _ => panic!("wrong op"),
        }
    }

    #[test]
    fn attr_set_omitted_mult_preserves_authored_multiplicity() {
        let op = round_trip(r#"{"op":"attr.set","node":"order","name":"total","ty":"Cash"}"#);

        assert!(matches!(
            op,
            Step::Uml(uml::Op::AttributeSet {
                multiplicity: FieldEdit::Unchanged,
                ..
            })
        ));
    }

    #[test]
    fn attr_set_null_mult_explicitly_clears_authored_multiplicity() {
        let op = round_trip(r#"{"op":"attr.set","node":"order","name":"total","mult":null}"#);

        assert!(matches!(
            op,
            Step::Uml(uml::Op::AttributeSet {
                multiplicity: FieldEdit::Clear,
                ..
            })
        ));
    }

    #[test]
    fn attr_set_wire_round_trip_preserves_all_multiplicity_intents() {
        for intent in [
            FieldEdit::Unchanged,
            FieldEdit::Clear,
            FieldEdit::Set(Multiplicity::parse("0..1").unwrap()),
        ] {
            let step = Step::Uml(uml::Op::AttributeSet {
                node: "order".into(),
                name: "total".into(),
                ty_token: None,
                multiplicity: intent,
                visibility: None,
                rename: None,
            });
            let value = serde_json::to_value(OpDto::from_step(&step)).unwrap();
            match &step {
                Step::Uml(uml::Op::AttributeSet {
                    multiplicity: FieldEdit::Unchanged,
                    ..
                }) => assert!(value.get("mult").is_none()),
                Step::Uml(uml::Op::AttributeSet {
                    multiplicity: FieldEdit::Clear,
                    ..
                }) => assert!(value.get("mult").is_some_and(serde_json::Value::is_null)),
                Step::Uml(uml::Op::AttributeSet {
                    multiplicity: FieldEdit::Set(_),
                    ..
                }) => assert_eq!(value.get("mult").and_then(|v| v.as_str()), Some("0..1")),
                _ => unreachable!(),
            }
            let dto: OpDto = serde_json::from_value(value).unwrap();
            assert_eq!(dto.to_step().unwrap(), step);
        }
    }

    #[test]
    fn version_defaults_to_one_and_rejects_unknown() {
        // absent v defaults to 1
        let _ = round_trip(r#"{"op":"value.add","node":"e","literal":"X"}"#);
        // explicit unknown v is rejected
        let dto: OpDto =
            serde_json::from_str(r#"{"v":2,"op":"value.add","node":"e","literal":"X"}"#).unwrap();
        assert!(dto.to_step().is_err());
    }

    #[test]
    fn rel_add_line_parses_kind_and_ends() {
        let op = round_trip(
            r#"{"v":1,"op":"rel.add","source":"order","kind":"composes","target":"order-line","ends":"1 to 1..* lines"}"#,
        );
        assert!(matches!(op, Step::Uml(uml::Op::RelationshipAdd { .. })));
    }

    #[test]
    fn from_step_round_trips_through_wire() {
        // build a core Op, render to a DTO line, parse back, expect equality
        let step = Step::Uml(uml::Op::AttributeAdd {
            node: "order".into(),
            name: "total".into(),
            ty_token: "Money".into(),
            multiplicity: Some(Multiplicity::parse("0..1").unwrap()),
            visibility: None,
        });
        let line = serde_json::to_string(&OpDto::from_step(&step)).unwrap();
        let back = {
            let dto: OpDto = serde_json::from_str(&line).unwrap();
            dto.to_step().unwrap()
        };
        assert_eq!(step, back);
    }

    #[test]
    fn mixed_domain_batch_is_ordered_and_atomic() {
        let source = SourceBundle::try_from_pairs([
            (
                "sales/order.md",
                "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n",
            ),
            ("archive/order.md", "# Existing\n"),
            ("a.md", "---\ntype: uml.Class\ntitle: A\n---\n# A\n"),
            ("b.md", "---\ntype: uml.Class\ntitle: B\n---\n# B\n"),
            (
                "dia.md",
                "---\ntype: Diagram\ntitle: Before\nprofile: uml-domain\n---\n# Before\n",
            ),
        ])
        .unwrap();
        let success: Vec<OpDto> = serde_json::from_value(serde_json::json!([
            {"op":"pkg.retitle","path":"sales","title":"Commerce"},
            {"op":"node.rename","from":"a","to":"renamed"},
            {
                "op":"place.set",
                "diagram":"dia",
                "subject_title":"A",
                "subject_slug":"renamed",
                "reference_title":"B",
                "reference_slug":"b",
                "directions":["LeftOf"]
            }
        ]))
        .unwrap();
        let changed = waml::edit::apply(&source, &to_batch(&success).unwrap()).unwrap();
        assert!(changed
            .documents()
            .iter()
            .any(|document| document.path().as_str() == "renamed.md"));
        assert!(changed
            .documents()
            .iter()
            .any(|document| document.path().as_str() == "sales/index.md"
                && document.text().contains("Commerce")));
        assert!(changed
            .documents()
            .iter()
            .any(|document| document.path().as_str() == "dia.md"
                && document.text().contains("renamed")));

        let original = source.to_pairs();
        let identities = source
            .documents()
            .iter()
            .map(|document| document.text().as_ptr())
            .collect::<Vec<_>>();
        let failure: Vec<OpDto> = serde_json::from_value(serde_json::json!([
            {"op":"pkg.retitle","path":"sales","title":"Changed"},
            {"op":"node.rename","from":"a","to":"renamed"},
            {
                "op":"place.set",
                "diagram":"dia",
                "subject_title":"A",
                "subject_slug":"renamed",
                "reference_title":"B",
                "reference_slug":"b",
                "directions":["LeftOf"]
            },
            {"op":"pkg.move","slug":"sales/order","to_dir":"archive"}
        ]))
        .unwrap();
        let error = waml::edit::apply(&source, &to_batch(&failure).unwrap()).unwrap_err();
        assert_eq!(error.index, 3);
        assert_eq!(source.to_pairs(), original);
        assert_eq!(
            source
                .documents()
                .iter()
                .map(|document| document.text().as_ptr())
                .collect::<Vec<_>>(),
            identities
        );
    }

    #[test]
    fn dto_node_new_then_attr_add_uses_the_same_mixed_candidate() {
        let source = SourceBundle::default();
        let dtos: Vec<OpDto> = serde_json::from_value(serde_json::json!([
            {
                "op":"node.new",
                "slug":"invoice",
                "dir":"sales",
                "ty":"uml.Class",
                "title":"Invoice",
                "stereotype":[],
                "abstract":false
            },
            {
                "op":"attr.add",
                "node":"sales/invoice",
                "name":"number",
                "ty":"String"
            }
        ]))
        .unwrap();

        let changed = waml::edit::apply(&source, &to_batch(&dtos).unwrap()).unwrap();

        assert!(changed
            .documents()
            .iter()
            .any(|document| document.path().as_str() == "sales/invoice.md"
                && document.text().contains("- number: String")));
    }

    #[test]
    fn dto_okf_move_then_attr_add_rebinds_the_renamed_concept() {
        let source = SourceBundle::try_from_pairs([(
            "sales/order.md",
            "---\r\ntype: uml.Class\r\ntitle: Order\r\n---\r\n\r\n# Order\r\n\r\n## Notes\r\nkeep Ω  \r\n",
        )])
        .unwrap();
        let dtos: Vec<OpDto> = serde_json::from_value(serde_json::json!([
            {"op":"pkg.move","slug":"sales/order","to_dir":"archive"},
            {"op":"attr.add","node":"archive/order","name":"id","ty":"String"}
        ]))
        .unwrap();

        let changed = waml::edit::apply(&source, &to_batch(&dtos).unwrap()).unwrap();
        let document = changed
            .documents()
            .iter()
            .find(|document| document.path().as_str() == "archive/order.md")
            .unwrap();
        assert!(document.text().contains("- id: String\r\n"));
        assert!(document.text().contains("## Notes\r\nkeep Ω  \r\n"));
    }

    #[test]
    fn every_op_survives_a_wire_round_trip() {
        use waml::model::parse_ends;
        use waml::model::{ElementType, RelationshipKind};
        use waml::multiplicity::Multiplicity;
        use waml::okf::DirectoryAddress;
        use waml::uml::selector::RelationshipSelector;
        use waml::uml::{NameSpec, Selector};

        let steps = vec![
            Step::Uml(uml::Op::ClassifierNew {
                slug: "order".into(),
                directory: DirectoryAddress::parse("/sales").unwrap(),
                ty: ElementType::parse("uml.Class"),
                title: "Order".into(),
                stereotype: vec!["entity".into()],
                description: Some("x".into()),
                abstract_: true,
            }),
            Step::Uml(uml::Op::ClassifierRename {
                from: "a".into(),
                to: "b".into(),
            }),
            Step::Uml(uml::Op::ClassifierSet {
                id: "order".into(),
                title: Some("O".into()),
                description: None,
                stereotype: Some(vec!["e".into()]),
                abstract_: Some(false),
                ty: None,
            }),
            Step::Uml(uml::Op::ClassifierRemove {
                id: "x".into(),
                cascade: true,
            }),
            Step::Uml(uml::Op::AttributeAdd {
                node: "order".into(),
                name: "total".into(),
                ty_token: "Money".into(),
                multiplicity: Some(Multiplicity::parse("0..1").unwrap()),
                visibility: Some(Visibility::Private),
            }),
            Step::Uml(uml::Op::AttributeSet {
                node: "order".into(),
                name: "total".into(),
                ty_token: Some("Cash".into()),
                multiplicity: FieldEdit::Set(Multiplicity::default()),
                visibility: None,
                rename: Some("amount".into()),
            }),
            Step::Uml(uml::Op::AttributeRemove {
                node: "order".into(),
                name: "total".into(),
            }),
            Step::Uml(uml::Op::ValueAdd {
                node: "e".into(),
                literal: "PLACED".into(),
            }),
            Step::Uml(uml::Op::ValueRemove {
                node: "e".into(),
                literal: "DRAFT".into(),
            }),
            Step::Uml(uml::Op::RelationshipAdd {
                source: "order".into(),
                kind: RelationshipKind::Composes,
                target: "order-line".into(),
                name: Some(NameSpec::Label("has".into())),
                ends: parse_ends("1 to 1..* lines"),
            }),
            Step::Uml(uml::Op::RelationshipSet {
                selector: RelationshipSelector::try_from(Selector::Rel {
                    source: "order".into(),
                    by: RelBy::Endpoint {
                        kind: RelationshipKind::Composes,
                        target: "order-line".into(),
                    },
                })
                .unwrap(),
                ends: parse_ends("1 to *"),
                name: None,
            }),
            Step::Uml(uml::Op::RelationshipRemove {
                selector: RelationshipSelector::try_from(Selector::Rel {
                    source: "order".into(),
                    by: RelBy::Named("has".into()),
                })
                .unwrap(),
            }),
            Step::Okf(okf::Op::ConceptMove {
                id: "order".into(),
                to_directory: DirectoryAddress::parse("/billing").unwrap(),
            }),
            Step::Okf(okf::Op::DirectoryRename {
                directory: DirectoryAddress::parse("/a").unwrap(),
                name: "b".into(),
            }),
            Step::Okf(okf::Op::DirectoryDelete {
                directory: DirectoryAddress::parse("/sales").unwrap(),
                cascade: false,
            }),
            Step::Okf(okf::Op::IndexReorder {
                directory: DirectoryAddress::parse("/sales").unwrap(),
                order: vec!["a".into()],
            }),
            Step::Okf(okf::Op::IndexSort {
                directory: DirectoryAddress::parse("/sales").unwrap(),
            }),
            Step::Okf(okf::Op::IndexRetitle {
                directory: DirectoryAddress::parse("/sales").unwrap(),
                title: "Sales Domain".into(),
            }),
            Step::Okf(okf::Op::BundleImport {
                parent: DirectoryAddress::parse("/sales").unwrap(),
                name: "orders".into(),
                bundle: SourceBundle::try_from_pairs([(
                    "t/order.md".to_string(),
                    "body".to_string(),
                )])
                .unwrap(),
            }),
            Step::Uml(uml::Op::PlacementSet {
                diagram: "dia".into(),
                subject_title: "Order".into(),
                subject_slug: "order".into(),
                reference_title: "Customer".into(),
                reference_slug: "customer".into(),
                directions: vec![Direction::LeftOf, Direction::Above],
            }),
            Step::Uml(uml::Op::PlacementRemove {
                diagram: "dia".into(),
                subject_slug: "order".into(),
                reference_slug: "customer".into(),
            }),
            Step::Uml(uml::Op::DiagramSet {
                key: "dia".into(),
                title: Some("D".into()),
                description: None,
                clear_description: false,
                display: None,
            }),
            Step::Uml(uml::Op::DiagramSet {
                key: "dia".into(),
                title: None,
                description: Some("notes".into()),
                clear_description: false,
                display: Some(DiagramDisplaySet {
                    show_attributes: false,
                    show_type: false,
                    show_attribute_visibility: false,
                    max_attributes: Some(6),
                    show_roles: false,
                    cardinality: CardinalityVisibility::Off,
                    show_cardinality: true,
                    show_labels: true,
                    show_stereotype: false,
                    stereotype_filter: Some(vec!["entity".into()]),
                    stereotype_colors: vec!["entity:#ffedd5".into()],
                }),
            }),
        ];
        for step in &steps {
            let line = serde_json::to_string(&OpDto::from_step(step)).unwrap();
            let back: OpDto = serde_json::from_str(&line).unwrap();
            assert_eq!(
                &back.to_step().unwrap(),
                step,
                "wire round-trip changed op: {line}"
            );
        }
    }

    #[test]
    fn pkg_insert_wire_round_trips() {
        let step = Step::Okf(okf::Op::BundleImport {
            parent: waml::okf::DirectoryAddress::parse("/sales").unwrap(),
            name: "orders".into(),
            bundle: SourceBundle::try_from_pairs([(
                "t/order.md".to_string(),
                "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string(),
            )])
            .unwrap(),
        });
        let line = serde_json::to_string(&OpDto::from_step(&step)).unwrap();
        assert!(line.contains("\"op\":\"pkg.insert\""), "wire tag: {line}");
        let back: OpDto = serde_json::from_str(&line).unwrap();
        assert_eq!(
            back.to_step().unwrap(),
            step,
            "round-trip changed op: {line}"
        );
    }

    #[test]
    fn diagram_set_wire_tag_is_diagram_dot_set() {
        let step = Step::Uml(uml::Op::DiagramSet {
            key: "dia".into(),
            title: Some("D".into()),
            description: None,
            clear_description: false,
            display: None,
        });
        let dto = OpDto::from_step(&step);
        let line = serde_json::to_string(&dto).unwrap();
        assert!(
            line.contains("\"op\":\"diagram.set\""),
            "unexpected wire tag: {line}"
        );
    }

    #[test]
    fn diagram_set_clear_description_survives_a_wire_round_trip() {
        let step = Step::Uml(uml::Op::DiagramSet {
            key: "dia".into(),
            title: None,
            description: None,
            clear_description: true,
            display: None,
        });

        let line = serde_json::to_string(&OpDto::from_step(&step)).unwrap();
        let back: OpDto = serde_json::from_str(&line).unwrap();

        assert_eq!(back.to_step().unwrap(), step);
        assert!(line.contains("\"clearDesc\":true"), "wire payload: {line}");
    }

    #[test]
    fn typescript_clear_description_payload_removes_the_authored_value() {
        let dto: OpDto =
            serde_json::from_str(r#"{"op":"diagram.set","key":"dia","clearDesc":true}"#).unwrap();
        let bundle = vec![(
            "dia.md".to_string(),
            "---\ntype: Diagram\ntitle: D\nprofile: uml-domain\ndescription: Notes\n---\n# D\n"
                .to_string(),
        )];

        let source = SourceBundle::try_from_pairs(bundle).unwrap();
        let batch = to_batch(&[dto]).unwrap();
        let updated = waml::edit::apply(&source, &batch).unwrap();
        let prepared = waml::analysis::prepare_candidate(updated, None, 0).unwrap();
        let model = &prepared.uml().projection;

        assert_eq!(model.diagrams[0].description, None);
    }

    #[test]
    fn wire_payload_never_exposes_invocation_revision_tokens() {
        let dto: OpDto =
            serde_json::from_str(r#"{"op":"attr.rm","v":1,"node":"order","name":"id"}"#).unwrap();
        let value = serde_json::to_value(dto).unwrap();
        let object = value.as_object().unwrap();

        assert_eq!(
            object.get("op").and_then(serde_json::Value::as_str),
            Some("attr.rm")
        );
        assert_eq!(object.get("v").and_then(serde_json::Value::as_u64), Some(1));
        assert!(!object.contains_key("revision"));
        assert!(!object.contains_key("sessionRevision"));
        assert!(!object.contains_key("documentRevision"));
    }

    #[test]
    fn legacy_diagram_set_wire_defaults_to_not_clearing_description() {
        let dto: OpDto = serde_json::from_str(r#"{"op":"diagram.set","v":1,"key":"dia"}"#).unwrap();

        assert_eq!(
            dto.to_step().unwrap(),
            Step::Uml(uml::Op::DiagramSet {
                key: "dia".into(),
                title: None,
                description: None,
                clear_description: false,
                display: None,
            })
        );
    }

    #[test]
    fn diagram_set_display_dto_round_trips_absent_nullable_fields() {
        let step = Step::Uml(uml::Op::DiagramSet {
            key: "dia".into(),
            title: None,
            description: None,
            clear_description: false,
            display: Some(DiagramDisplaySet {
                show_attributes: true,
                show_type: true,
                show_attribute_visibility: true,
                max_attributes: None,
                show_roles: true,
                cardinality: CardinalityVisibility::Explicit,
                show_cardinality: true,
                show_labels: true,
                show_stereotype: true,
                stereotype_filter: None,
                stereotype_colors: vec![],
            }),
        });
        let dto = OpDto::from_step(&step);
        let line = serde_json::to_string(&dto).unwrap();
        let back: OpDto = serde_json::from_str(&line).unwrap();
        assert_eq!(
            back.to_step().unwrap(),
            step,
            "absent maxAttributes/stereotypeFilter must round-trip as None: {line}"
        );
    }

    #[test]
    fn display_dto_keeps_the_legacy_gate_at_the_wire_boundary() {
        let dto = DisplayDto {
            show_attributes: true,
            show_type: true,
            show_attribute_visibility: true,
            show_attribute_multiplicity: true,
            max_attributes: None,
            show_roles: true,
            cardinality: CardinalityVisibility::Off,
            show_cardinality: true,
            show_labels: true,
            show_stereotype: true,
            stereotype_filter: None,
            stereotype_colors: vec![],
        };

        let display = display_dto_to_set(&dto);
        assert_eq!(display.cardinality, CardinalityVisibility::Off);
        assert!(display.show_cardinality);
        let normalized = display_set_to_dto(&display);
        assert!(
            !normalized.show_attribute_multiplicity,
            "the internal enum must derive the legacy wire compatibility field"
        );
        assert!(normalized.show_cardinality);
    }
}
