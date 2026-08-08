use crate::doc_view::DocView;
use crate::editor_history::EditMergeKey;
use crate::icons::Icon;
use crate::navigation::DocumentLocator;
use crate::view_history::ViewLocation;
use makepad_widgets::{LiveId, Vec4};
use waml::edit::PendingEdit;

#[derive(Clone)]
pub struct EditIntent {
    pub edit: PendingEdit,
    pub label: String,
    pub merge_key: Option<EditMergeKey>,
    pub after_location: Option<ViewLocation>,
}

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

impl From<waml::view::kind::RowKind> for NavCategory {
    fn from(kind: waml::view::kind::RowKind) -> Self {
        use waml::view::kind::RowKind;
        match kind {
            RowKind::Directory => NavCategory::Directory,
            RowKind::OkfDocument => NavCategory::OkfDocument,
            RowKind::Class => NavCategory::Class,
            RowKind::Interface => NavCategory::Interface,
            RowKind::Enum => NavCategory::Enum,
            RowKind::DataType => NavCategory::DataType,
            RowKind::Diagram => NavCategory::Diagram,
            RowKind::Behavior => NavCategory::Behavior,
            RowKind::Sequence => NavCategory::Sequence,
            RowKind::Note => NavCategory::Note,
        }
    }
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
    pub locator: DocumentLocator,
    pub title: String,
    pub presentation: DocumentPresentation,
    pub view: Box<dyn DocView>,
}

impl OpenDocument {
    #[cfg(test)]
    pub fn locator(&self) -> DocumentLocator {
        self.locator.clone()
    }

    pub fn into_tab(self, preview: bool) -> (crate::doc_tabs::DocTab, Box<dyn DocView>) {
        (
            crate::doc_tabs::DocTab {
                id: self.tab_id,
                locator: self.locator,
                title: self.title,
                presentation: self.presentation,
                preview,
                resolved: true,
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
