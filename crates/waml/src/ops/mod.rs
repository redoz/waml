use crate::layout::Direction;
use crate::model::{ElementType, RelEnd, RelationshipKind, Visibility};
use crate::multiplicity::Multiplicity;
use crate::source::SourceBundle;

pub type Bundle = Vec<(String, String)>;

pub type OpError = crate::edit::EditError;

pub use crate::uml::{DiagramDisplaySet, FieldEdit, NameSpec};

/// One mutation. One variant per sugar command; grows task by task.
#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    AttrAdd {
        node: String,
        name: String,
        ty_token: String,
        multiplicity: Option<Multiplicity>,
        visibility: Option<Visibility>,
    },
    AttrSet {
        node: String,
        name: String,
        ty_token: Option<String>,
        multiplicity: FieldEdit<Multiplicity>,
        visibility: Option<Visibility>,
        rename: Option<String>,
    },
    AttrRm {
        node: String,
        name: String,
    },
    ValueAdd {
        node: String,
        literal: String,
    },
    ValueRm {
        node: String,
        literal: String,
    },
    RelAdd {
        source: String,
        kind: RelationshipKind,
        target: String,
        name: Option<NameSpec>,
        ends: Option<(RelEnd, RelEnd)>,
    },
    RelSet {
        selector: Selector,
        ends: Option<(RelEnd, RelEnd)>,
        name: Option<NameSpec>,
    },
    RelRm {
        selector: Selector,
    },
    NodeNew {
        slug: String,
        /// Target package directory ("" = root). File written at `<dir>/<slug>.md`.
        dir: String,
        ty: ElementType,
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
        ty: Option<ElementType>,
    },
    NodeRm {
        slug: String,
        cascade: bool,
    },
    NodeRename {
        from: String,
        to: String,
    },
    PkgMove {
        slug: String,
        to_dir: String,
    },
    PkgRename {
        from: String,
        to: String,
    },
    PkgDelete {
        path: String,
        cascade: bool,
    },
    PkgReorder {
        path: String,
        order: Vec<String>,
    },
    PkgSort {
        path: String,
    },
    PkgRetitle {
        path: String,
        title: String,
    },
    PkgInsert {
        parent_path: String,
        name: String,
        docs: Vec<(String, String)>,
    },
    DiagramSet {
        key: String,                        // diagram doc id (full-path or bare slug)
        title: Option<String>,              // None = leave unchanged
        description: Option<String>,        // None = leave unchanged
        clear_description: bool,            // true = remove authored description
        display: Option<DiagramDisplaySet>, // None = leave display untouched
    },
    PlaceSet {
        diagram: String,
        subject_title: String,
        subject_slug: String,
        reference_title: String,
        reference_slug: String,
        directions: Vec<Direction>,
    },
    PlaceRm {
        diagram: String,
        subject_slug: String,
        reference_slug: String,
    },
}

pub fn apply(bundle: &[(String, String)], ops: &[Op]) -> Result<Bundle, OpError> {
    let source = SourceBundle::try_from_pairs(bundle.iter().cloned())
        .map_err(|error| OpError::at("bundle", error.to_string()))?;
    apply_source(&source, ops).map(|bundle| bundle.to_pairs())
}

pub fn apply_source(bundle: &SourceBundle, ops: &[Op]) -> Result<SourceBundle, OpError> {
    let mut steps = Vec::with_capacity(ops.len());
    for (index, op) in ops.iter().cloned().enumerate() {
        let step = crate::compat::step_from_legacy(op).map_err(|mut error| {
            error.index = index;
            error
        })?;
        steps.push(step);
    }
    let batch = crate::compat::Batch::new(steps);
    crate::compat::apply(bundle, &batch)
}

pub fn referrers(bundle: &Bundle, slug: &str) -> Vec<String> {
    crate::uml::lower::referrers(bundle, slug)
}

pub mod selector {
    pub use crate::uml::selector::*;
}
pub use selector::{parse_selector, render_selector, RelBy, Selector};
