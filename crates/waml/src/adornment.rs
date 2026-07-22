//! Terminal adornment selection: which standard-UML glyph sits at a
//! relationship's endpoint. Pure and frontend-agnostic — the makepad canvas and
//! the web renderer both call [`end_marker`] to decide the shape, then draw the
//! geometry themselves. Notation-policy switching (crowsfoot vs UML) is a future
//! concern (a `DiagramDisplay::notation` field, not built yet); this module
//! encodes the default UML notation only.

use crate::model::RelationshipKind;

/// Which end of a relationship an adornment sits on. `From` is the `from:` side
/// (the near end), `To` the `to:` side (the far end).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum End {
    From,
    To,
}

/// A standard-UML terminal glyph. The frontend owns geometry and stroke; this
/// only names the shape to draw at an endpoint (or `None` for a bare line end).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Marker {
    /// No adornment.
    None,
    /// Open ("stick") arrowhead — association navigability, dependency.
    OpenArrow,
    /// Hollow (unfilled) triangle — generalization/realization, at the general end.
    HollowTriangle,
    /// Hollow diamond — shared aggregation, at the aggregate (whole) end.
    HollowDiamond,
    /// Filled diamond — composite aggregation, at the composite (whole) end.
    FilledDiamond,
}

/// Pick the standard-UML glyph for one end of a relationship.
///
/// Direction follows WAML's `from … to …`:
/// - `specializes`/`implements`: `from` is the subtype/class, `to` the
///   supertype/interface — a hollow triangle sits at the `to` (general) end.
/// - `composes`/`aggregates`: `from` is the whole, `to` the part — a filled
///   (composition) or hollow (aggregation) diamond sits at the `from` (whole)
///   end.
/// - `depends`: an open arrow points at the `to` (depended-upon) end.
/// - `associates`: an open arrow marks a navigable end (`navigable == Some(true)`),
///   otherwise nothing.
///
/// `navigable` is the decorated end's [`RelEnd::navigable`](crate::model::RelEnd);
/// it only affects `associates`. Every other relationship kind ignores it.
pub fn end_marker(kind: RelationshipKind, end: End, navigable: Option<bool>) -> Marker {
    use End::*;
    use RelationshipKind::*;
    match (kind, end) {
        (Specializes | Implements, To) => Marker::HollowTriangle,
        (Composes, From) => Marker::FilledDiamond,
        (Aggregates, From) => Marker::HollowDiamond,
        (Depends, To) => Marker::OpenArrow,
        (Associates, _) if navigable == Some(true) => Marker::OpenArrow,
        _ => Marker::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use RelationshipKind::*;

    #[test]
    fn generalization_triangle_at_general_end() {
        // Subtype `from` is bare; the supertype `to` end carries the triangle.
        assert_eq!(end_marker(Specializes, End::From, None), Marker::None);
        assert_eq!(end_marker(Specializes, End::To, None), Marker::HollowTriangle);
    }

    #[test]
    fn realization_triangle_at_interface_end() {
        assert_eq!(end_marker(Implements, End::From, None), Marker::None);
        assert_eq!(end_marker(Implements, End::To, None), Marker::HollowTriangle);
    }

    #[test]
    fn composition_filled_diamond_at_whole_end() {
        // Whole is the `from` end; the part `to` end is bare.
        assert_eq!(end_marker(Composes, End::From, None), Marker::FilledDiamond);
        assert_eq!(end_marker(Composes, End::To, None), Marker::None);
    }

    #[test]
    fn aggregation_hollow_diamond_at_whole_end() {
        assert_eq!(end_marker(Aggregates, End::From, None), Marker::HollowDiamond);
        assert_eq!(end_marker(Aggregates, End::To, None), Marker::None);
    }

    #[test]
    fn dependency_open_arrow_at_target() {
        assert_eq!(end_marker(Depends, End::From, None), Marker::None);
        assert_eq!(end_marker(Depends, End::To, None), Marker::OpenArrow);
    }

    #[test]
    fn association_arrow_only_on_navigable_end() {
        // Unspecified or non-navigable ends stay bare; a navigable end gets an arrow.
        assert_eq!(end_marker(Associates, End::To, None), Marker::None);
        assert_eq!(end_marker(Associates, End::To, Some(false)), Marker::None);
        assert_eq!(end_marker(Associates, End::To, Some(true)), Marker::OpenArrow);
        assert_eq!(end_marker(Associates, End::From, Some(true)), Marker::OpenArrow);
    }

    #[test]
    fn unadorned_kinds_are_bare_both_ends() {
        for kind in [Annotates, Includes, Extends, InstanceOf, Links] {
            for end in [End::From, End::To] {
                assert_eq!(end_marker(kind, end, Some(true)), Marker::None);
                assert_eq!(end_marker(kind, end, None), Marker::None);
            }
        }
    }
}
