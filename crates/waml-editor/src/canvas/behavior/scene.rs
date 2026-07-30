//! `BehaviorScene`: what `BehaviorSurface` draws (spec §5.3). Only `Empty` is
//! declared this task -- `Flow`/`Interaction` land in Tasks 7 and 8, together
//! with their renderer and constructor (an unconstructed variant is dead_code
//! pressure in this binary crate).

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum BehaviorScene {
    Empty { message: String },
}

impl Default for BehaviorScene {
    fn default() -> Self {
        BehaviorScene::Empty {
            message: String::new(),
        }
    }
}
