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

use crate::model::{FlowNodeKind, FragmentKind, RelationshipKind};

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

/// Words that open a direction clause.
pub const LAYOUT_DIRECTION_HEADS: &[&str] = &["above", "below", "left", "right"];

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
