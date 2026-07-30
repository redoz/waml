//! Behavior-canvas hit-testing (spec §5.1). `BehaviorTarget` has no variants
//! this task -- `Empty` scenes hit nothing, and Tasks 7-9 add the
//! flow/interaction variants together with the geometry that makes them
//! reachable.

use super::scene::BehaviorScene;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum BehaviorTarget {}

pub(crate) fn hit_test(scene: &BehaviorScene, world: (f64, f64)) -> Option<BehaviorTarget> {
    let _ = world;
    match scene {
        BehaviorScene::Empty { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_scene_hits_nothing() {
        let scene = BehaviorScene::Empty {
            message: "No renderable elements".into(),
        };
        assert_eq!(hit_test(&scene, (10.0, 10.0)), None);
    }
}
