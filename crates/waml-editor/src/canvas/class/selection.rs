use crate::scene::SceneNode;
use std::collections::{BTreeMap, HashSet};

pub(crate) const SELECTION_TICK: f64 = 1.0 / 144.0;
const LIFT_SECS: f64 = 0.140;
const RELEASE_SECS: f64 = 0.180;

#[derive(Clone, Copy, Debug, PartialEq)]
struct LiftTween {
    value: f64,
    from: f64,
    target: f64,
    elapsed: f64,
}

impl LiftTween {
    fn new(target: f64) -> Self {
        Self {
            value: 0.0,
            from: 0.0,
            target,
            elapsed: 0.0,
        }
    }

    fn retarget(&mut self, target: f64) {
        self.from = self.value;
        self.target = target;
        self.elapsed = 0.0;
    }

    fn active(self) -> bool {
        (self.value - self.target).abs() > f64::EPSILON
    }
}

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
    lifts: BTreeMap<String, LiftTween>,
    lift_last_time: f64,
    lift_target_changed: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SelectionSnapshot {
    pub(crate) selected_key: Option<String>,
    pub(crate) selected_index: Option<usize>,
    pub(crate) constraint_visibility: ConstraintVisibility,
    pub(crate) conflict_focus_keys: Option<HashSet<String>>,
    pub(crate) show_hidden_borders: bool,
    pub(crate) lifts: BTreeMap<String, f64>,
}

impl SelectionSnapshot {
    pub(crate) fn lift_for(&self, key: &str) -> f64 {
        self.lifts.get(key).copied().unwrap_or(0.0)
    }
}

impl Default for SelectionState {
    fn default() -> Self {
        Self {
            selected_key: None,
            selected_index: None,
            constraint_visibility: ConstraintVisibility::Selected,
            conflict_focus_keys: None,
            show_hidden_borders: false,
            lifts: BTreeMap::new(),
            lift_last_time: 0.0,
            lift_target_changed: false,
        }
    }
}

impl SelectionState {
    pub(crate) fn select(&mut self, key: &str, nodes: &[SceneNode]) -> bool {
        let selected_index = nodes.iter().position(|node| node.key == key);
        let selected_key = selected_index.map(|_| key.to_string());
        let changed = self.selected_key != selected_key || self.selected_index != selected_index;
        if self.selected_key != selected_key {
            if let Some(key) = self.selected_key.clone() {
                self.retarget_lift(&key, 0.0);
            }
            if let Some(key) = selected_key.as_deref() {
                self.retarget_lift(key, 1.0);
            }
        }
        self.selected_key = selected_key;
        self.selected_index = selected_index;
        changed
    }

    pub(crate) fn clear(&mut self) -> bool {
        let changed = self.selected_key.is_some() || self.selected_index.is_some();
        if let Some(key) = self.selected_key.clone() {
            self.retarget_lift(&key, 0.0);
        }
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
        let node_keys: HashSet<&str> = nodes.iter().map(|node| node.key.as_str()).collect();
        let stale_keys: Vec<String> = self
            .lifts
            .keys()
            .filter(|key| !node_keys.contains(key.as_str()))
            .cloned()
            .collect();
        for key in stale_keys {
            self.retarget_lift(&key, 0.0);
        }
    }

    pub(crate) fn selected_key(&self) -> Option<&str> {
        self.selected_key.as_deref()
    }

    pub(crate) fn lift_for(&self, key: &str) -> f64 {
        self.lifts.get(key).map_or(0.0, |lift| lift.value)
    }

    pub(crate) fn lift_animation_active(&self) -> bool {
        self.lifts.values().any(|lift| lift.active())
    }

    pub(crate) fn take_lift_target_changed(&mut self) -> bool {
        std::mem::take(&mut self.lift_target_changed)
    }

    pub(crate) fn tick_lift(&mut self, now: f64) {
        let dt = if self.lift_last_time == 0.0 || now <= self.lift_last_time {
            SELECTION_TICK
        } else {
            now - self.lift_last_time
        };
        self.lift_last_time = now;
        for lift in self.lifts.values_mut() {
            if !lift.active() {
                continue;
            }
            let duration = if lift.target > 0.0 {
                LIFT_SECS
            } else {
                RELEASE_SECS
            };
            lift.elapsed = (lift.elapsed + dt).min(duration);
            if duration - lift.elapsed <= 1.0e-9 {
                lift.elapsed = duration;
            }
            let t = lift.elapsed / duration;
            let eased = if lift.target > 0.0 {
                1.0 - (1.0 - t).powi(3)
            } else {
                t.powi(3)
            };
            lift.value = lift.from + (lift.target - lift.from) * eased;
            if lift.elapsed >= duration {
                lift.value = lift.target;
            }
        }
        self.lifts
            .retain(|_, lift| lift.target > 0.0 || lift.value > 0.0);
        if !self.lift_animation_active() {
            self.lift_last_time = 0.0;
        }
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
            lifts: self
                .lifts
                .iter()
                .map(|(key, lift)| (key.clone(), lift.value))
                .collect(),
        }
    }

    fn retarget_lift(&mut self, key: &str, target: f64) {
        if target == 0.0 && self.lift_for(key) == 0.0 {
            self.lifts.remove(key);
            return;
        }
        if !self.lifts.contains_key(key) {
            self.lifts.insert(key.to_string(), LiftTween::new(target));
            self.lift_last_time = 0.0;
            self.lift_target_changed = true;
            return;
        }
        let lift = self
            .lifts
            .entry(key.to_string())
            .or_insert_with(|| LiftTween::new(target));
        if (lift.target - target).abs() <= f64::EPSILON {
            return;
        }
        lift.retarget(target);
        self.lift_last_time = 0.0;
        self.lift_target_changed = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1.0e-9,
            "expected {expected}, got {actual}"
        );
    }

    fn tick_for(state: &mut SelectionState, start: f64, elapsed: f64) {
        state.tick_lift(start);
        if elapsed > SELECTION_TICK {
            state.tick_lift(start + elapsed - SELECTION_TICK);
        }
    }

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

    #[test]
    fn selecting_starts_a_lift_without_delaying_logical_selection() {
        let mut state = SelectionState::default();

        assert!(state.select("a", &nodes(&["a"])));

        assert_eq!(state.selected_key(), Some("a"));
        assert_close(state.lift_for("a"), 0.0);
        assert!(state.lift_animation_active());
        assert!(state.take_lift_target_changed());
    }

    #[test]
    fn deselecting_releases_from_the_sampled_lift() {
        let mut state = SelectionState::default();
        state.select("a", &nodes(&["a"]));
        tick_for(&mut state, 1.0, LIFT_SECS);
        assert_close(state.lift_for("a"), 1.0);

        assert!(state.clear());

        assert_eq!(state.selected_key(), None);
        assert_close(state.lift_for("a"), 1.0);
        tick_for(&mut state, 2.0, RELEASE_SECS / 2.0);
        assert_close(state.lift_for("a"), 0.875);
        assert!(state.lift_animation_active());
    }

    #[test]
    fn changing_selection_mid_lift_is_continuous_and_animates_both_nodes() {
        let scene = nodes(&["a", "b"]);
        let mut state = SelectionState::default();
        state.select("a", &scene);
        tick_for(&mut state, 1.0, LIFT_SECS / 2.0);
        assert_close(state.lift_for("a"), 0.875);

        state.select("b", &scene);

        assert_eq!(state.selected_key(), Some("b"));
        assert_close(state.lift_for("a"), 0.875);
        assert_close(state.lift_for("b"), 0.0);
        tick_for(&mut state, 2.0, RELEASE_SECS / 2.0);
        assert_close(state.lift_for("a"), 0.765625);
        assert!(state.lift_for("b") > 0.0);
    }

    #[test]
    fn lift_and_release_finish_at_exact_endpoints() {
        let mut state = SelectionState::default();
        state.select("a", &nodes(&["a"]));
        tick_for(&mut state, 1.0, LIFT_SECS);
        assert_close(state.lift_for("a"), 1.0);
        assert!(!state.lift_animation_active());

        state.clear();
        tick_for(&mut state, 2.0, RELEASE_SECS);
        assert_close(state.lift_for("a"), 0.0);
        assert!(!state.lift_animation_active());
    }

    #[test]
    fn reconcile_releases_then_removes_stale_lift_keys() {
        let mut state = SelectionState::default();
        state.select("a", &nodes(&["a"]));
        tick_for(&mut state, 1.0, LIFT_SECS);

        state.reconcile(&nodes(&["b"]), SelectionPolicy::Preserve);

        assert_eq!(state.selected_key(), None);
        assert_close(state.lift_for("a"), 1.0);
        tick_for(&mut state, 2.0, RELEASE_SECS);
        assert_close(state.lift_for("a"), 0.0);
        assert!(state.snapshot().lifts.is_empty());
    }
}
