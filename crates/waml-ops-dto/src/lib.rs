use serde::{Deserialize, Serialize};
use waml::grammar::{parse_ends, render_ends};
use waml::model::{ClassifierType, RelEnd, RelationshipKind, Visibility};
use waml::multiplicity::Multiplicity;
use waml::ops::{DiagramDisplaySet, NameSpec, Op, RelBy, Selector};

fn one() -> u32 {
    1
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
        #[serde(default)]
        mult: Option<String>,
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
    #[serde(rename = "diagram.set")]
    DiagramSet {
        #[serde(default = "one")]
        v: u32,
        key: String,
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        desc: Option<String>,
        #[serde(default)]
        display: Option<DisplayDto>,
    },
}

/// A fully-specified display block on the wire. Mirrors `waml::ops::DiagramDisplaySet`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayDto {
    pub show_attributes: bool,
    pub attribute_detail: String,
    pub show_attribute_visibility: bool,
    pub show_attribute_multiplicity: bool,
    #[serde(default)]
    pub max_attributes: Option<u32>,
    pub association_labels: String,
    pub emphasize_multiplicity: bool,
    pub show_stereotype: bool,
    #[serde(default)]
    pub stereotype_filter: Option<Vec<String>>,
    #[serde(default)]
    pub stereotype_colors: Vec<String>,
}

fn display_dto_to_set(d: &DisplayDto) -> DiagramDisplaySet {
    DiagramDisplaySet {
        show_attributes: d.show_attributes,
        attribute_detail: d.attribute_detail.clone(),
        show_attribute_visibility: d.show_attribute_visibility,
        show_attribute_multiplicity: d.show_attribute_multiplicity,
        max_attributes: d.max_attributes,
        association_labels: d.association_labels.clone(),
        emphasize_multiplicity: d.emphasize_multiplicity,
        show_stereotype: d.show_stereotype,
        stereotype_filter: d.stereotype_filter.clone(),
        stereotype_colors: d.stereotype_colors.clone(),
    }
}

fn display_set_to_dto(ds: &DiagramDisplaySet) -> DisplayDto {
    DisplayDto {
        show_attributes: ds.show_attributes,
        attribute_detail: ds.attribute_detail.clone(),
        show_attribute_visibility: ds.show_attribute_visibility,
        show_attribute_multiplicity: ds.show_attribute_multiplicity,
        max_attributes: ds.max_attributes,
        association_labels: ds.association_labels.clone(),
        emphasize_multiplicity: ds.emphasize_multiplicity,
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
fn mult_req(s: &Option<String>) -> Result<Multiplicity, String> {
    match s {
        None => Ok(Multiplicity::default()),
        Some(m) => Multiplicity::parse(m).ok_or_else(|| format!("bad multiplicity '{m}'")),
    }
}
fn mult_opt(s: &Option<String>) -> Result<Option<Multiplicity>, String> {
    s.as_ref()
        .map(|m| Multiplicity::parse(m).ok_or_else(|| format!("bad multiplicity '{m}'")))
        .transpose()
}
fn vis_opt(s: &Option<String>) -> Result<Option<Visibility>, String> {
    match s {
        None => Ok(None),
        Some(c) => {
            let ch = c.chars().next().ok_or("empty visibility")?;
            Visibility::from_marker(ch).map(Some).ok_or_else(|| format!("bad visibility '{c}'"))
        }
    }
}
fn ends_opt(s: &Option<String>) -> Result<Option<(RelEnd, RelEnd)>, String> {
    s.as_ref().map(|e| parse_ends(e).ok_or_else(|| format!("bad ends '{e}'"))).transpose()
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
) -> Result<Selector, String> {
    let by = match (kind, target, as_sel) {
        (Some(k), Some(t), _) => RelBy::Endpoint { kind: kind_req(k)?, target: t.clone() },
        (_, _, Some(n)) => RelBy::Named(n.clone()),
        _ => return Err("relationship selector needs kind+target or as".into()),
    };
    Ok(Selector::Rel { source: source.to_string(), by })
}

impl OpDto {
    pub fn to_op(&self) -> Result<Op, String> {
        match self {
            OpDto::NodeNew { v, slug, dir, ty, title, stereotype, desc, abstract_ } => {
                check_v(*v, "node.new")?;
                Ok(Op::NodeNew {
                    slug: slug.clone(),
                    dir: dir.clone(),
                    ty: ClassifierType::parse(ty),
                    title: title.clone(),
                    stereotype: stereotype.clone(),
                    description: desc.clone(),
                    abstract_: *abstract_,
                })
            }
            OpDto::NodeRename { v, from, to } => {
                check_v(*v, "node.rename")?;
                Ok(Op::NodeRename { from: from.clone(), to: to.clone() })
            }
            OpDto::NodeSet { v, slug, title, desc, stereotype, abstract_, ty } => {
                check_v(*v, "node.set")?;
                Ok(Op::NodeSet {
                    slug: slug.clone(),
                    title: title.clone(),
                    description: desc.clone(),
                    stereotype: stereotype.clone(),
                    abstract_: *abstract_,
                    ty: ty.as_ref().map(|t| ClassifierType::parse(t)),
                })
            }
            OpDto::NodeRm { v, slug, cascade } => {
                check_v(*v, "node.rm")?;
                Ok(Op::NodeRm { slug: slug.clone(), cascade: *cascade })
            }
            OpDto::AttrAdd { v, node, name, ty, mult, vis } => {
                check_v(*v, "attr.add")?;
                Ok(Op::AttrAdd {
                    node: node.clone(),
                    name: name.clone(),
                    ty_token: ty.clone(),
                    multiplicity: mult_req(mult)?,
                    visibility: vis_opt(vis)?,
                })
            }
            OpDto::AttrSet { v, node, name, ty, mult, vis, rename } => {
                check_v(*v, "attr.set")?;
                Ok(Op::AttrSet {
                    node: node.clone(),
                    name: name.clone(),
                    ty_token: ty.clone(),
                    multiplicity: mult_opt(mult)?,
                    visibility: vis_opt(vis)?,
                    rename: rename.clone(),
                })
            }
            OpDto::AttrRm { v, node, name } => {
                check_v(*v, "attr.rm")?;
                Ok(Op::AttrRm { node: node.clone(), name: name.clone() })
            }
            OpDto::ValueAdd { v, node, literal } => {
                check_v(*v, "value.add")?;
                Ok(Op::ValueAdd { node: node.clone(), literal: literal.clone() })
            }
            OpDto::ValueRm { v, node, literal } => {
                check_v(*v, "value.rm")?;
                Ok(Op::ValueRm { node: node.clone(), literal: literal.clone() })
            }
            OpDto::RelAdd { v, source, kind, target, as_label, as_ref, ends } => {
                check_v(*v, "rel.add")?;
                Ok(Op::RelAdd {
                    source: source.clone(),
                    kind: kind_req(kind)?,
                    target: target.clone(),
                    name: name_of(as_label, as_ref),
                    ends: ends_opt(ends)?,
                })
            }
            OpDto::RelSet { v, source, kind, target, as_sel, ends, set_label, set_as_ref } => {
                check_v(*v, "rel.set")?;
                Ok(Op::RelSet {
                    selector: rel_sel(source, kind, target, as_sel)?,
                    ends: ends_opt(ends)?,
                    name: name_of(set_label, set_as_ref),
                })
            }
            OpDto::RelRm { v, source, kind, target, as_sel } => {
                check_v(*v, "rel.rm")?;
                Ok(Op::RelRm { selector: rel_sel(source, kind, target, as_sel)? })
            }
            OpDto::PkgMove { v, slug, to_dir } => {
                check_v(*v, "pkg.move")?;
                Ok(Op::PkgMove { slug: slug.clone(), to_dir: to_dir.clone() })
            }
            OpDto::PkgRename { v, from, to } => {
                check_v(*v, "pkg.rename")?;
                Ok(Op::PkgRename { from: from.clone(), to: to.clone() })
            }
            OpDto::PkgDelete { v, path, cascade } => {
                check_v(*v, "pkg.delete")?;
                Ok(Op::PkgDelete { path: path.clone(), cascade: *cascade })
            }
            OpDto::PkgReorder { v, path, order } => {
                check_v(*v, "pkg.reorder")?;
                Ok(Op::PkgReorder { path: path.clone(), order: order.clone() })
            }
            OpDto::PkgSort { v, path } => {
                check_v(*v, "pkg.sort")?;
                Ok(Op::PkgSort { path: path.clone() })
            }
            OpDto::DiagramSet { v, key, title, desc, display } => {
                check_v(*v, "diagram.set")?;
                Ok(Op::DiagramSet {
                    key: key.clone(),
                    title: title.clone(),
                    description: desc.clone(),
                    display: display.as_ref().map(display_dto_to_set),
                })
            }
        }
    }

    /// Reverse of `to_op`; not yet wired to any CLI surface (future `waml serve`/`--emit`
    /// round-trip work) but kept alongside `to_op` and covered by its own round-trip test.
    #[allow(dead_code)]
    pub fn from_op(op: &Op) -> OpDto {
        let ends_str = |e: &Option<(RelEnd, RelEnd)>| e.as_ref().map(|(f, t)| render_ends(f, t));
        let name_parts = |n: &Option<NameSpec>| match n {
            Some(NameSpec::Label(l)) => (Some(l.clone()), None),
            Some(NameSpec::Ref(s)) => (None, Some(s.clone())),
            None => (None, None),
        };
        match op {
            Op::NodeNew { slug, dir, ty, title, stereotype, description, abstract_ } => OpDto::NodeNew {
                v: 1,
                slug: slug.clone(),
                dir: dir.clone(),
                ty: ty.as_str(),
                title: title.clone(),
                stereotype: stereotype.clone(),
                desc: description.clone(),
                abstract_: *abstract_,
            },
            Op::NodeRename { from, to } => OpDto::NodeRename { v: 1, from: from.clone(), to: to.clone() },
            Op::NodeSet { slug, title, description, stereotype, abstract_, ty } => OpDto::NodeSet {
                v: 1,
                slug: slug.clone(),
                title: title.clone(),
                desc: description.clone(),
                stereotype: stereotype.clone(),
                abstract_: *abstract_,
                ty: ty.as_ref().map(|t| t.as_str()),
            },
            Op::NodeRm { slug, cascade } => OpDto::NodeRm { v: 1, slug: slug.clone(), cascade: *cascade },
            Op::AttrAdd { node, name, ty_token, multiplicity, visibility } => OpDto::AttrAdd {
                v: 1,
                node: node.clone(),
                name: name.clone(),
                ty: ty_token.clone(),
                mult: Some(multiplicity.as_str().to_string()),
                vis: visibility.map(|x| x.marker().to_string()),
            },
            Op::AttrSet { node, name, ty_token, multiplicity, visibility, rename } => OpDto::AttrSet {
                v: 1,
                node: node.clone(),
                name: name.clone(),
                ty: ty_token.clone(),
                mult: multiplicity.as_ref().map(|m| m.as_str().to_string()),
                vis: visibility.map(|x| x.marker().to_string()),
                rename: rename.clone(),
            },
            Op::AttrRm { node, name } => OpDto::AttrRm { v: 1, node: node.clone(), name: name.clone() },
            Op::ValueAdd { node, literal } => OpDto::ValueAdd { v: 1, node: node.clone(), literal: literal.clone() },
            Op::ValueRm { node, literal } => OpDto::ValueRm { v: 1, node: node.clone(), literal: literal.clone() },
            Op::RelAdd { source, kind, target, name, ends } => {
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
            Op::RelSet { selector, ends, name } => {
                let (source, kind, target, as_sel) = sel_parts(selector);
                let (set_label, set_as_ref) = name_parts(name);
                OpDto::RelSet { v: 1, source, kind, target, as_sel, ends: ends_str(ends), set_label, set_as_ref }
            }
            Op::RelRm { selector } => {
                let (source, kind, target, as_sel) = sel_parts(selector);
                OpDto::RelRm { v: 1, source, kind, target, as_sel }
            }
            Op::PkgMove { slug, to_dir } => OpDto::PkgMove { v: 1, slug: slug.clone(), to_dir: to_dir.clone() },
            Op::PkgRename { from, to } => OpDto::PkgRename { v: 1, from: from.clone(), to: to.clone() },
            Op::PkgDelete { path, cascade } => OpDto::PkgDelete { v: 1, path: path.clone(), cascade: *cascade },
            Op::PkgReorder { path, order } => OpDto::PkgReorder { v: 1, path: path.clone(), order: order.clone() },
            Op::PkgSort { path } => OpDto::PkgSort { v: 1, path: path.clone() },
            Op::DiagramSet { key, title, description, display } => OpDto::DiagramSet {
                v: 1,
                key: key.clone(),
                title: title.clone(),
                desc: description.clone(),
                display: display.as_ref().map(display_set_to_dto),
            },
        }
    }
}

/// Decompose a `Selector::Rel` into wire fields for `from_op`.
#[allow(dead_code)]
fn sel_parts(sel: &Selector) -> (String, Option<String>, Option<String>, Option<String>) {
    match sel {
        Selector::Rel { source, by: RelBy::Endpoint { kind, target } } =>
            (source.clone(), Some(kind.as_str().to_string()), Some(target.clone()), None),
        Selector::Rel { source, by: RelBy::Named(n) } => (source.clone(), None, None, Some(n.clone())),
        // node/attr/value selectors never reach a rel op; render source-only as a defensive default
        Selector::Node(s) => (s.clone(), None, None, None),
        Selector::Attr { node, .. } | Selector::Value { node, .. } => (node.clone(), None, None, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use waml::ops::Op;

    fn round_trip(line: &str) -> Op {
        let dto: OpDto = serde_json::from_str(line).unwrap();
        dto.to_op().unwrap()
    }

    #[test]
    fn parses_attr_add_line() {
        let op = round_trip(r#"{"v":1,"op":"attr.add","node":"order","name":"total","ty":"Money","mult":"0..1"}"#);
        match op {
            Op::AttrAdd { node, name, ty_token, multiplicity, .. } => {
                assert_eq!((node.as_str(), name.as_str(), ty_token.as_str()), ("order", "total", "Money"));
                assert_eq!(multiplicity.as_str(), "0..1");
            }
            _ => panic!("wrong op"),
        }
    }

    #[test]
    fn version_defaults_to_one_and_rejects_unknown() {
        // absent v defaults to 1
        let _ = round_trip(r#"{"op":"value.add","node":"e","literal":"X"}"#);
        // explicit unknown v is rejected
        let dto: OpDto = serde_json::from_str(r#"{"v":2,"op":"value.add","node":"e","literal":"X"}"#).unwrap();
        assert!(dto.to_op().is_err());
    }

    #[test]
    fn rel_add_line_parses_kind_and_ends() {
        let op = round_trip(
            r#"{"v":1,"op":"rel.add","source":"order","kind":"composes","target":"order-line","ends":"1 to 1..* lines"}"#,
        );
        assert!(matches!(op, Op::RelAdd { .. }));
    }

    #[test]
    fn from_op_round_trips_through_to_op() {
        // build a core Op, render to a DTO line, parse back, expect equality
        let op = Op::AttrAdd {
            node: "order".into(),
            name: "total".into(),
            ty_token: "Money".into(),
            multiplicity: waml::multiplicity::Multiplicity::parse("0..1").unwrap(),
            visibility: None,
        };
        let line = serde_json::to_string(&OpDto::from_op(&op)).unwrap();
        let back = {
            let dto: OpDto = serde_json::from_str(&line).unwrap();
            dto.to_op().unwrap()
        };
        assert_eq!(op, back);
    }

    #[test]
    fn every_op_survives_a_wire_round_trip() {
        use waml::grammar::parse_ends;
        use waml::model::{ClassifierType, RelationshipKind};
        use waml::multiplicity::Multiplicity;
        use waml::ops::{NameSpec, Op, RelBy, Selector};

        let ops = vec![
            Op::NodeNew {
                slug: "order".into(),
                dir: "sales".into(),
                ty: ClassifierType::parse("uml.Class"),
                title: "Order".into(),
                stereotype: vec!["entity".into()],
                description: Some("x".into()),
                abstract_: true,
            },
            Op::NodeRename { from: "a".into(), to: "b".into() },
            Op::NodeSet {
                slug: "order".into(),
                title: Some("O".into()),
                description: None,
                stereotype: Some(vec!["e".into()]),
                abstract_: Some(false),
                ty: None,
            },
            Op::NodeRm { slug: "x".into(), cascade: true },
            Op::AttrAdd {
                node: "order".into(),
                name: "total".into(),
                ty_token: "Money".into(),
                multiplicity: Multiplicity::parse("0..1").unwrap(),
                visibility: Some(waml::model::Visibility::Private),
            },
            Op::AttrSet {
                node: "order".into(),
                name: "total".into(),
                ty_token: Some("Cash".into()),
                multiplicity: Some(Multiplicity::default()),
                visibility: None,
                rename: Some("amount".into()),
            },
            Op::AttrRm { node: "order".into(), name: "total".into() },
            Op::ValueAdd { node: "e".into(), literal: "PLACED".into() },
            Op::ValueRm { node: "e".into(), literal: "DRAFT".into() },
            Op::RelAdd {
                source: "order".into(),
                kind: RelationshipKind::Composes,
                target: "order-line".into(),
                name: Some(NameSpec::Label("has".into())),
                ends: parse_ends("1 to 1..* lines"),
            },
            Op::RelSet {
                selector: Selector::Rel {
                    source: "order".into(),
                    by: RelBy::Endpoint { kind: RelationshipKind::Composes, target: "order-line".into() },
                },
                ends: parse_ends("1 to *"),
                name: None,
            },
            Op::RelRm { selector: Selector::Rel { source: "order".into(), by: RelBy::Named("has".into()) } },
            Op::PkgMove { slug: "order".into(), to_dir: "billing".into() },
            Op::PkgRename { from: "a".into(), to: "b".into() },
            Op::PkgDelete { path: "sales".into(), cascade: false },
            Op::PkgReorder { path: "sales".into(), order: vec!["a".into()] },
            Op::PkgSort { path: "sales".into() },
            Op::DiagramSet {
                key: "dia".into(),
                title: Some("D".into()),
                description: None,
                display: None,
            },
            Op::DiagramSet {
                key: "dia".into(),
                title: None,
                description: Some("notes".into()),
                display: Some(DiagramDisplaySet {
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
                }),
            },
        ];
        for op in &ops {
            let line = serde_json::to_string(&OpDto::from_op(op)).unwrap();
            let back: OpDto = serde_json::from_str(&line).unwrap();
            assert_eq!(&back.to_op().unwrap(), op, "wire round-trip changed op: {line}");
        }
    }

    #[test]
    fn diagram_set_wire_tag_is_diagram_dot_set() {
        let dto = OpDto::from_op(&Op::DiagramSet {
            key: "dia".into(),
            title: Some("D".into()),
            description: None,
            display: None,
        });
        let line = serde_json::to_string(&dto).unwrap();
        assert!(line.contains("\"op\":\"diagram.set\""), "unexpected wire tag: {line}");
    }

    #[test]
    fn diagram_set_display_dto_round_trips_absent_nullable_fields() {
        let op = Op::DiagramSet {
            key: "dia".into(),
            title: None,
            description: None,
            display: Some(DiagramDisplaySet {
                show_attributes: true,
                attribute_detail: "name-type".into(),
                show_attribute_visibility: true,
                show_attribute_multiplicity: true,
                max_attributes: None,
                association_labels: "all".into(),
                emphasize_multiplicity: false,
                show_stereotype: true,
                stereotype_filter: None,
                stereotype_colors: vec![],
            }),
        };
        let dto = OpDto::from_op(&op);
        let line = serde_json::to_string(&dto).unwrap();
        let back: OpDto = serde_json::from_str(&line).unwrap();
        assert_eq!(back.to_op().unwrap(), op, "absent maxAttributes/stereotypeFilter must round-trip as None: {line}");
    }
}
