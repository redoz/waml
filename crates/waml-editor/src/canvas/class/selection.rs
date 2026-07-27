use crate::scene::SceneNode;
use std::collections::HashSet;

/// What constraint veils the canvas draws (spec §1). Persisted in view state and
/// driven by the view bar's constraints toggle.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) enum ConstraintVisibility {
    /// No constraint marks — pure diagram.
    None,
    /// Selecting a node lights every constraint touching it (sticky). Default.
    #[default]
    Selected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SelectionPolicy {
    Clear,
    Preserve,
}

pub(crate) struct SelectionState {
    selected_key: Option<String>,
    selected_index: Option<usize>,
    constraint_visibility: ConstraintVisibility,
    conflict_focus_keys: Option<HashSet<String>>,
    show_hidden_borders: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SelectionSnapshot {
    pub(crate) selected_key: Option<String>,
    pub(crate) selected_index: Option<usize>,
    pub(crate) constraint_visibility: ConstraintVisibility,
    pub(crate) conflict_focus_keys: Option<HashSet<String>>,
    pub(crate) show_hidden_borders: bool,
}

impl Default for SelectionState {
    fn default() -> Self {
        Self {
            selected_key: None,
            selected_index: None,
            constraint_visibility: ConstraintVisibility::Selected,
            conflict_focus_keys: None,
            show_hidden_borders: false,
        }
    }
}

impl SelectionState {
    pub(crate) fn select(&mut self, key: &str, nodes: &[SceneNode]) -> bool {
        let selected_index = nodes.iter().position(|node| node.key == key);
        let selected_key = selected_index.map(|_| key.to_string());
        let changed = self.selected_key != selected_key || self.selected_index != selected_index;
        self.selected_key = selected_key;
        self.selected_index = selected_index;
        changed
    }

    pub(crate) fn clear(&mut self) -> bool {
        let changed = self.selected_key.is_some() || self.selected_index.is_some();
        self.selected_key = None;
        self.selected_index = None;
        changed
    }

    pub(crate) fn reconcile(&mut self, nodes: &[SceneNode], policy: SelectionPolicy) {
        match policy {
            SelectionPolicy::Clear => {
                self.clear();
                self.conflict_focus_keys = None;
            }
            SelectionPolicy::Preserve => {
                let key = self.selected_key.clone();
                match key {
                    Some(key) => {
                        self.select(&key, nodes);
                    }
                    None => self.selected_index = None,
                }
                if let Some(keys) = &mut self.conflict_focus_keys {
                    let node_keys: HashSet<&str> =
                        nodes.iter().map(|node| node.key.as_str()).collect();
                    keys.retain(|key| node_keys.contains(key.as_str()));
                    if keys.is_empty() {
                        self.conflict_focus_keys = None;
                    }
                }
            }
        }
    }

    pub(crate) fn selected_key(&self) -> Option<&str> {
        self.selected_key.as_deref()
    }

    #[cfg(test)]
    pub(crate) fn selected_index(&self) -> Option<usize> {
        self.selected_index
    }

    pub(crate) fn has_selection(&self) -> bool {
        self.selected_key.is_some()
    }

    pub(crate) fn set_constraint_visibility(&mut self, mode: ConstraintVisibility) {
        self.constraint_visibility = mode;
    }

    pub(crate) fn set_conflict_focus_keys(&mut self, keys: Option<Vec<String>>) {
        self.conflict_focus_keys = keys
            .map(|keys| keys.into_iter().collect::<HashSet<_>>())
            .filter(|keys| !keys.is_empty());
    }

    pub(crate) fn set_show_hidden_borders(&mut self, on: bool) {
        self.show_hidden_borders = on;
    }

    pub(crate) fn snapshot(&self) -> SelectionSnapshot {
        SelectionSnapshot {
            selected_key: self.selected_key.clone(),
            selected_index: self.selected_index,
            constraint_visibility: self.constraint_visibility,
            conflict_focus_keys: self.conflict_focus_keys.clone(),
            show_hidden_borders: self.show_hidden_borders,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn nodes(keys: &[&str]) -> Vec<crate::scene::SceneNode> {
        use waml::model::{ElementType, UmlMetaclass};

        keys.iter()
            .enumerate()
            .map(|(index, key)| crate::scene::SceneNode {
                key: (*key).to_string(),
                title: (*key).to_string(),
                element_type: ElementType::Uml(UmlMetaclass::Class),
                stereotypes: Vec::new(),
                stereotype_visible: true,
                attributes: Vec::new(),
                operations: Vec::new(),
                header: crate::scene::HeaderStyle::Plain,
                ports: false,
                rect: waml::solve::Rect {
                    x: index as f64 * 100.0,
                    y: 0.0,
                    w: 80.0,
                    h: 60.0,
                },
                emphasized: false,
                collapsed: false,
                expanded: false,
            })
            .collect()
    }

    #[test]
    fn preserve_viewport_re_resolves_the_selected_key() {
        let mut state = SelectionState::default();
        state.select("b", &nodes(&["a", "b"]));
        state.reconcile(&nodes(&["b", "c"]), SelectionPolicy::Preserve);
        assert_eq!(state.selected_key(), Some("b"));
        assert_eq!(state.selected_index(), Some(0));
    }

    #[test]
    fn a_missing_selected_key_clears_both_key_and_index() {
        let mut state = SelectionState::default();
        state.select("b", &nodes(&["a", "b"]));
        state.reconcile(&nodes(&["a"]), SelectionPolicy::Preserve);
        assert_eq!(state.selected_key(), None);
        assert_eq!(state.selected_index(), None);
    }

    #[test]
    fn stale_conflict_focus_keys_are_removed() {
        let mut state = SelectionState::default();
        state.set_conflict_focus_keys(Some(vec!["a".into(), "missing".into()]));
        state.reconcile(&nodes(&["a", "b"]), SelectionPolicy::Preserve);
        assert_eq!(
            state.snapshot().conflict_focus_keys,
            Some(HashSet::from(["a".to_string()])),
        );
    }
}
