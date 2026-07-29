#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DocumentKind {
    Primary,
    Source,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DocumentLocator {
    pub concept_id: String,
    pub kind: DocumentKind,
}

impl DocumentLocator {
    pub fn new(concept_id: impl Into<String>, kind: DocumentKind) -> Self {
        Self {
            concept_id: concept_id.into(),
            kind,
        }
    }

    pub fn primary(concept_id: impl Into<String>) -> Self {
        Self::new(concept_id, DocumentKind::Primary)
    }

    pub fn source(concept_id: impl Into<String>) -> Self {
        Self::new(concept_id, DocumentKind::Source)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DiagramCameraAnchor {
    pub pan_x: f64,
    pub pan_y: f64,
    pub zoom: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ViewAnchor {
    None,
    Markdown {
        fragment: Option<String>,
        scroll_y: f64,
    },
    Diagram {
        selected_key: Option<String>,
        camera: DiagramCameraAnchor,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct ViewLocation {
    pub document: DocumentLocator,
    pub anchor: ViewAnchor,
}
