use std::sync::LazyLock;

use regex::Regex;

use crate::model::RelationshipKind;

/// Addresses a model element for `Op` manipulation. `Node`/`Rel` mirror the
/// three `NoteAnchor` forms (`Classifier`, `NamedAssoc`, `EndpointAssoc`) so
/// future `annotates` resolution can reuse [`parse_selector`]; `Attr`/`Value`
/// extend addressing to sub-elements that notes never reach.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selector {
    /// A classifier/diagram slug, e.g. `order`.
    Node(String),
    /// A relationship reachable from `source`, identified either by name or
    /// by its verb + target endpoint.
    Rel { source: String, by: RelBy },
    /// An attribute of a node.
    Attr { node: String, name: String },
    /// An enum value literal of a node.
    Value { node: String, literal: String },
}

/// How a [`Selector::Rel`] identifies which relationship it addresses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelBy {
    /// By its `as "name"` label.
    Named(String),
    /// By its verb (`RelationshipKind`) and target endpoint slug.
    Endpoint { kind: RelationshipKind, target: String },
}

// Leading `[Title](./slug.md)` anchor link, capturing the slug; then an
// optional tail describing what's addressed relative to that node.
static SEL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\[[^\]]+\]\(\./(?P<src>[^)]+?)\.md\)(?P<tail>.*)$").unwrap()
});
// Tail: ` <verb> [Title](./slug.md)` — an endpoint-form relationship.
static ENDPOINT_TAIL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"^\s+(?P<verb>\w+)\s+\[[^\]]+\]\(\./(?P<tgt>[^)]+?)\.md\)$"#).unwrap()
});
// Tail: ` as "name"` — a named-form relationship.
static NAMED_TAIL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^\s+as\s+"(?P<name>[^"]*)"$"#).unwrap());
// Tail: ` attr <name>` — an attribute extension.
static ATTR_TAIL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s+attr\s+(?P<name>.+)$").unwrap());
// Tail: ` value <literal>` — a value extension.
static VALUE_TAIL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s+value\s+(?P<lit>.+)$").unwrap());

/// Slugs in anchor links are relative paths (`./order.md`); a selector only
/// stores the basename, so strip any directory components.
fn basename(slug: &str) -> String {
    slug.rsplit(['/', '\\']).next().unwrap_or(slug).to_string()
}

/// Parses the anchor **link** form of a selector, i.e. what appears in a note
/// line: `[Title](./slug.md)` optionally followed by a relationship,
/// attribute, or value tail.
pub fn parse_selector(s: &str) -> Option<Selector> {
    let s = s.trim();
    let caps = SEL_RE.captures(s)?;
    let src = basename(&caps["src"]);
    let tail = &caps["tail"];
    if tail.is_empty() {
        return Some(Selector::Node(src));
    }
    if let Some(t) = ENDPOINT_TAIL.captures(tail) {
        let kind = RelationshipKind::parse(&t["verb"])?;
        return Some(Selector::Rel {
            source: src,
            by: RelBy::Endpoint { kind, target: basename(&t["tgt"]) },
        });
    }
    if let Some(t) = NAMED_TAIL.captures(tail) {
        return Some(Selector::Rel { source: src, by: RelBy::Named(t["name"].to_string()) });
    }
    if let Some(t) = ATTR_TAIL.captures(tail) {
        return Some(Selector::Attr { node: src, name: t["name"].trim().to_string() });
    }
    if let Some(t) = VALUE_TAIL.captures(tail) {
        return Some(Selector::Value { node: src, literal: t["lit"].trim().to_string() });
    }
    None
}

/// Renders a compact display form for error messages. This is NOT the
/// inverse of [`parse_selector`]: a `Selector` only stores slugs, kinds, and
/// names, never titles, so it cannot round-trip to the anchor link form.
pub fn render_selector(sel: &Selector) -> String {
    match sel {
        Selector::Node(s) => s.clone(),
        Selector::Rel { source, by: RelBy::Endpoint { kind, target } } => {
            format!("{source} {} {target}", kind.as_str())
        }
        Selector::Rel { source, by: RelBy::Named(name) } => format!("{source} as \"{name}\""),
        Selector::Attr { node, name } => format!("{node} attr {name}"),
        Selector::Value { node, literal } => format!("{node} value {literal}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_classifier_anchor_as_node() {
        assert_eq!(parse_selector("[Order](./order.md)"), Some(Selector::Node("order".into())));
    }

    #[test]
    fn parses_endpoint_anchor_as_rel() {
        let got = parse_selector("[Order](./order.md) composes [OrderLine](./order-line.md)");
        assert_eq!(got, Some(Selector::Rel {
            source: "order".into(),
            by: RelBy::Endpoint { kind: RelationshipKind::Composes, target: "order-line".into() },
        }));
    }

    #[test]
    fn parses_named_anchor_as_rel() {
        let got = parse_selector("[Order](./order.md) as \"places\"");
        assert_eq!(got, Some(Selector::Rel { source: "order".into(), by: RelBy::Named("places".into()) }));
    }

    #[test]
    fn parses_attr_and_value_extensions() {
        assert_eq!(parse_selector("[Order](./order.md) attr total"),
            Some(Selector::Attr { node: "order".into(), name: "total".into() }));
        assert_eq!(parse_selector("[OrderStatus](./order-status.md) value PLACED"),
            Some(Selector::Value { node: "order-status".into(), literal: "PLACED".into() }));
    }

    #[test]
    fn rejects_garbage() {
        assert_eq!(parse_selector("not selector"), None);
        assert_eq!(parse_selector("[Order](./order.md) frobnicates [X](./x.md)"), None);
    }

    #[test]
    fn renders_compact_display_form() {
        assert_eq!(render_selector(&Selector::Node("order".into())), "order");
        assert_eq!(render_selector(&Selector::Rel {
            source: "order".into(),
            by: RelBy::Endpoint { kind: RelationshipKind::Composes, target: "order-line".into() },
        }), "order composes order-line");
        assert_eq!(render_selector(&Selector::Rel { source: "order".into(), by: RelBy::Named("places".into()) }),
            "order as \"places\"");
        assert_eq!(render_selector(&Selector::Attr { node: "order".into(), name: "total".into() }), "order attr total");
        assert_eq!(render_selector(&Selector::Value { node: "order-status".into(), literal: "PLACED".into() }),
            "order-status value PLACED");
    }
}
