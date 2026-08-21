#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiagramName {
    pub(crate) display: &'static str,
    pub(crate) value: &'static str,
}

impl DiagramName {
    // `value` is the row's semantic value: the concept id, which the loader
    // derives from the fixture file name (mini/orders-diagram.md).
    pub const ORDERS: Self = Self {
        display: "Orders",
        value: "orders-diagram",
    };

    /// The `Behavior` fixture's one document: a state machine whose `Active`
    /// node carries both a self-loop and a back edge.
    pub const LIGHT_CYCLE: Self = Self {
        display: "Light Cycle",
        value: "light-cycle",
    };

    pub const fn new(display: &'static str, value: &'static str) -> Self {
        Self { display, value }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewKind {
    Diagram,
    Source,
}

/// One of the seven mutually-exclusive surfaces that can own the editor's
/// centre. Named semantically rather than by widget id: a scenario says
/// "the folder listing has the centre", not `folder_view_surface`.
///
/// `ViewKind` above is a coarser question -- "is a diagram or the raw source
/// showing" -- and it cannot express the other five surfaces at all, which
/// is why a route that crosses them needs this instead.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DocumentSurface {
    /// The class-diagram canvas.
    Canvas,
    /// The activity/state-machine/sequence canvas.
    BehaviorCanvas,
    /// The raw-markdown editor.
    Source,
    /// The rendered reading view.
    Reading,
    /// A folder's projected row listing.
    Folder,
    /// A query's full results tab.
    Results,
    /// A folder read as one continuous scroll.
    Book,
}

impl DocumentSurface {
    pub(crate) fn description(self) -> &'static str {
        match self {
            Self::Canvas => "diagram canvas",
            Self::BehaviorCanvas => "behavior canvas",
            Self::Source => "raw source",
            Self::Reading => "reading",
            Self::Folder => "folder listing",
            Self::Results => "search results",
            Self::Book => "book",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::DiagramName;

    #[test]
    fn diagram_name_constructor_keeps_catalog_semantics_together() {
        let diagram = DiagramName::new("Missing", "missing");

        assert_eq!(diagram.display, "Missing");
        assert_eq!(diagram.value, "missing");
    }
}
