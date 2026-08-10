//! The single owner of WAML's closed keyword vocabularies.
//!
//! Before this module the layout keyword list existed twice -- as literal match
//! arms in `uml::syntax::parser` and as a private `KEYWORDS` const in
//! `uml::format` -- and the two drifted. The parser, the formatter and
//! `uml::complete` all read the tables here now; a third copy anywhere is a
//! defect. Vocabularies a model enum already owns (`RelationshipKind`,
//! `FlowNodeKind`, `FragmentKind`) are *derived* from that enum rather than
//! retyped, so the enum stays the authority and this module stays the single
//! lookup surface.

use crate::model::{FlowNodeKind, FragmentKind, RelationshipKind, UmlMetaclass};

/// Every word the `## Layout` grammar treats as a keyword rather than a name.
/// Sorted, so the table is greppable and the sortedness test is meaningful.
pub const LAYOUT_KEYWORDS: &[&str] = &[
    "above",
    "aligned",
    "and",
    "as",
    "below",
    "bottom",
    "box",
    "center",
    "collapsed",
    "column",
    "emphasized",
    "frame",
    "large",
    "left",
    "margin",
    "margins",
    "medium",
    "no",
    "of",
    "right",
    "row",
    "shrink",
    "small",
    "top",
    "with",
];

/// Hint words that stand alone after `with`.
pub const LAYOUT_SHAPE_HINTS: &[&str] = &["frame", "box", "shrink", "emphasized", "collapsed"];

/// Hint words that must be followed by `margin` or `margins`.
pub const LAYOUT_MARGIN_SIZES: &[&str] = &["no", "small", "medium", "large"];

/// Complete hint phrases, exactly as a completion inserts them.
pub const LAYOUT_HINT_PHRASES: &[&str] = &[
    "frame",
    "box",
    "shrink",
    "emphasized",
    "collapsed",
    "no margin",
    "small margin",
    "medium margin",
    "large margin",
];

/// Words that open an alignment edge; always followed by `of`.
pub const LAYOUT_EDGE_WORDS: &[&str] = &["top", "bottom", "left", "right", "center"];

/// Words that open an inline group; always followed by `of`.
pub const LAYOUT_AXIS_WORDS: &[&str] = &["row", "column"];

/// Words that open a direction clause and may take an optional lateral word
/// before `of` (`above left of`).
pub const LAYOUT_DIRECTION_VERTICALS: &[&str] = &["above", "below"];

/// Words that open a direction clause on their own and must be followed by
/// `of`; they are also the optional second word of a vertical direction.
pub const LAYOUT_DIRECTION_LATERALS: &[&str] = &["left", "right"];

/// Complete direction phrases, exactly as a completion inserts them.
pub const LAYOUT_DIRECTION_PHRASES: &[&str] = &[
    "above",
    "below",
    "left of",
    "right of",
    "above left of",
    "above right of",
    "below left of",
    "below right of",
];

/// The five message verbs the `## Messages` grammar accepts.
pub const MESSAGE_VERBS: &[&str] = &["calls", "returns", "signals", "creates", "destroys"];

/// Every relationship kind, longest keyword first. `InstanceOf` leads because
/// `instance of` is two words: matched after the one-word kinds it would be
/// truncated to `instance`. `uml::lower` depends on this order.
pub const RELATIONSHIP_KINDS: &[RelationshipKind] = &[
    RelationshipKind::InstanceOf,
    RelationshipKind::Associates,
    RelationshipKind::Aggregates,
    RelationshipKind::Composes,
    RelationshipKind::Specializes,
    RelationshipKind::Implements,
    RelationshipKind::Depends,
    RelationshipKind::Annotates,
    RelationshipKind::Includes,
    RelationshipKind::Extends,
    RelationshipKind::Links,
];

/// Every flow node kind that has a heading keyword. `Plain` is the absence of
/// one and is deliberately not here.
pub const FLOW_NODE_KINDS: &[FlowNodeKind] = &[
    FlowNodeKind::Initial,
    FlowNodeKind::Final,
    FlowNodeKind::Decision,
    FlowNodeKind::Merge,
    FlowNodeKind::Fork,
    FlowNodeKind::Join,
    FlowNodeKind::Object,
];

/// Every combined-fragment kind.
pub const FRAGMENT_KINDS: &[FragmentKind] = &[
    FragmentKind::Alt,
    FragmentKind::Opt,
    FragmentKind::Loop,
    FragmentKind::Par,
    FragmentKind::Break,
    FragmentKind::Critical,
    FragmentKind::Assert,
    FragmentKind::Neg,
];

pub fn relationship_keywords() -> impl Iterator<Item = &'static str> {
    RELATIONSHIP_KINDS.iter().map(|kind| kind.as_str())
}

pub fn flow_node_keywords() -> impl Iterator<Item = &'static str> {
    FLOW_NODE_KINDS.iter().filter_map(|kind| kind.keyword())
}

pub fn fragment_keywords() -> impl Iterator<Item = &'static str> {
    FRAGMENT_KINDS.iter().map(|kind| kind.as_str())
}

/// The canonical spelling of `word` when it is a layout keyword, `None` when it
/// is a name. `margins` folds to `margin`; everything else lower-cases.
pub fn canonical_layout_keyword(word: &str) -> Option<&'static str> {
    let lower = word.to_ascii_lowercase();
    if lower == "margins" {
        return Some("margin");
    }
    LAYOUT_KEYWORDS
        .iter()
        .copied()
        .find(|keyword| *keyword == lower.as_str())
}

/// Every spelling the frontmatter `type:` key accepts, in offer order.
///
/// Derived from [`UmlMetaclass::ALL`] and [`crate::model::DiagramKind::ALL`] rather than
/// written out again: this module exists so a keyword has exactly one home, and
/// the element types are no exception.
pub fn element_type_names() -> impl Iterator<Item = String> {
    UmlMetaclass::ALL
        .iter()
        .map(|metaclass| format!("uml.{}", metaclass.name()))
        .chain(
            crate::model::DiagramKind::ALL
                .iter()
                .map(|kind| kind.as_str().to_owned()),
        )
}

#[cfg(test)]
mod element_type_tests {
    use super::*;
    use crate::model::ElementType;

    /// The deliberate-update guard. Adding a variant forces the compiler to
    /// extend `name()`, but nothing forces `ALL` to grow with it, so the counts
    /// are pinned here: bump them in the same change that adds the variant.
    #[test]
    fn every_element_type_is_listed_exactly_once() {
        assert_eq!(UmlMetaclass::ALL.len(), 10);
        let names = element_type_names().collect::<Vec<_>>();
        assert_eq!(names.len(), 15);
        let mut sorted = names.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), names.len(), "duplicate spelling in {names:?}");
    }

    /// Every offered spelling must round-trip through the parser the guarding
    /// code uses. An offer the analysis would call `Unknown` is a bug.
    #[test]
    fn every_offered_spelling_parses_back_to_itself() {
        for name in element_type_names() {
            let parsed = ElementType::parse(&name);
            assert!(
                !matches!(parsed, ElementType::Unknown(_)),
                "{name} does not parse"
            );
            assert_eq!(parsed.as_str(), name);
        }
    }
}
