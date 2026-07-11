use crate::model::{Attribute, TypeRef, Visibility};
use crate::multiplicity::Multiplicity;
use crate::parse::parse_document;
use crate::serialize::serialize_document;
use crate::syntax::{Document, Section};

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
    }
}

// ---- shared helpers (reused by every later op) ----

pub(crate) fn slug_of(path: &str) -> String {
    let seg = path.rsplit(['/', '\\']).next().unwrap_or(path);
    seg.strip_suffix(".md").unwrap_or(seg).to_string()
}

pub(crate) fn find_doc(work: &Bundle, slug: &str, op: &str) -> Result<usize, OpError> {
    work.iter()
        .position(|(p, _)| slug_of(p) == slug)
        .ok_or_else(|| OpError::at(op, format!("no document '{slug}'")))
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
pub(crate) fn attrs_mut(doc: &mut Document) -> &mut Vec<Attribute> {
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

/// Forward-ref-safe: a token matching an existing doc slug links to it (using
/// that doc's title); otherwise it is a bare type token. Mirrors build_model.
pub(crate) fn resolve_type(work: &Bundle, token: &str) -> TypeRef {
    if let Some((_, text)) = work.iter().find(|(p, _)| slug_of(p) == token) {
        let title = parse_document(text)
            .frontmatter
            .get_str("title")
            .map(String::from)
            .unwrap_or_else(|| token.to_string());
        TypeRef { name: title, ref_: Some(token.to_string()) }
    } else {
        TypeRef { name: token.to_string(), ref_: None }
    }
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
        if attrs.iter().any(|a| a.name == name) {
            return Err(OpError::at("attr.add", format!("attribute '{name}' already exists in {node}")));
        }
        attrs.push(Attribute {
            name: name.to_string(),
            ty,
            multiplicity: multiplicity.clone(),
            visibility,
            description: None,
        });
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
            if new != name && attrs.iter().any(|a| a.name == *new) {
                return Err(OpError::at("attr.set", format!("attribute '{new}' already exists in {node}")));
            }
        }
        let a = attrs
            .iter_mut()
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
        attrs.retain(|a| a.name != name);
        if attrs.len() == before {
            return Err(OpError::at("attr.rm", format!("no attribute '{name}' in {node}")));
        }
        Ok(())
    })
}

pub mod selector;
pub use selector::{parse_selector, render_selector, RelBy, Selector};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::multiplicity::Multiplicity;

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
        assert!(out[0].1.contains("- id: String [0..1]"));
        let doc = parse_document(&out[0].1);
        let attrs = doc.sections.iter().find_map(|s| match s {
            Section::Attributes(a) => Some(a),
            _ => None,
        }).expect("attributes section present");
        let id = attrs.iter().find(|a| a.name == "id").expect("id attribute present");
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
}
