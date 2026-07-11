use serde::{Deserialize, Serialize};
use uaml::grammar::{parse_ends, render_ends};
use uaml::model::{ClassifierType, RelEnd, RelationshipKind, Visibility};
use uaml::multiplicity::Multiplicity;
use uaml::ops::{NameSpec, Op, RelBy, Selector};

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
            OpDto::NodeNew { v, slug, ty, title, stereotype, desc, abstract_ } => {
                check_v(*v, "node.new")?;
                Ok(Op::NodeNew {
                    slug: slug.clone(),
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
        }
    }

    pub fn from_op(op: &Op) -> OpDto {
        let ends_str = |e: &Option<(RelEnd, RelEnd)>| e.as_ref().map(|(f, t)| render_ends(f, t));
        let name_parts = |n: &Option<NameSpec>| match n {
            Some(NameSpec::Label(l)) => (Some(l.clone()), None),
            Some(NameSpec::Ref(s)) => (None, Some(s.clone())),
            None => (None, None),
        };
        match op {
            Op::NodeNew { slug, ty, title, stereotype, description, abstract_ } => OpDto::NodeNew {
                v: 1,
                slug: slug.clone(),
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
        }
    }
}

/// Decompose a `Selector::Rel` into wire fields for `from_op`.
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
    use uaml::ops::Op;

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
            multiplicity: uaml::multiplicity::Multiplicity::parse("0..1").unwrap(),
            visibility: None,
        };
        let line = serde_json::to_string(&OpDto::from_op(&op)).unwrap();
        let back = {
            let dto: OpDto = serde_json::from_str(&line).unwrap();
            dto.to_op().unwrap()
        };
        assert_eq!(op, back);
    }
}
