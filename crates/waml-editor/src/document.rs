use crate::doc_view::DocView;
use crate::icons::Icon;
use makepad_widgets::{LiveId, Vec4};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavCategory {
    Directory,
    OkfDocument,
    Class,
    Interface,
    Enum,
    DataType,
    Diagram,
    Behavior,
    Sequence,
    Note,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DocumentPresentation {
    pub icon: Icon,
    pub accent: Option<Vec4>,
    pub category: NavCategory,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DocumentCapabilities {
    pub can_edit_classifier: bool,
    pub can_delete_classifier: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DocumentDescriptor {
    pub presentation: DocumentPresentation,
    pub capabilities: DocumentCapabilities,
}

pub struct OpenDocument {
    pub tab_id: LiveId,
    pub concept_id: String,
    pub title: String,
    pub presentation: DocumentPresentation,
    pub view: Box<dyn DocView>,
}

impl OpenDocument {
    pub fn into_tab(self, preview: bool) -> (crate::doc_tabs::DocTab, Box<dyn DocView>) {
        (
            crate::doc_tabs::DocTab {
                id: self.tab_id,
                concept_id: self.concept_id,
                title: self.title,
                presentation: self.presentation,
                preview,
            },
            self.view,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directories_and_generic_documents_are_distinct_presentations() {
        assert_ne!(NavCategory::Directory, NavCategory::OkfDocument);
    }
}
