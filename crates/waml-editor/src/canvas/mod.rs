use makepad_widgets::*;

mod behavior;
mod class;
mod geometry;
mod linework;
pub(crate) mod primitives;
mod stale_badge;
mod viewport;

pub(crate) use behavior::hit::BehaviorTarget;
pub(crate) use behavior::scene::{
    ActivationGeo, BehaviorScene, FlowEdgeGeo, FlowNodeGeo, FlowOffPageGeo, FragmentGeo,
    LifelineGeo, MessageGeo, OperandGeo,
};
pub(crate) use behavior::{BehaviorSurface, BehaviorSurfaceAction};
pub(crate) use class::{
    zone_arrow, zone_id, zone_of_id, zone_placed, ClassDiagramSurface, ClassDiagramSurfaceAction,
    ConstraintVisibility, COMPASS_ZONES, DIAL_ZONES,
};
// Approved façade types can be consumed through method signatures and type
// inference without a direct named use in this binary crate.
#[allow(unused_imports)]
pub(crate) use class::{DialPlacement, Placed, Zone};
pub(crate) use viewport::ZOOM_STEP;

/// Registers every canvas-family widget's `script_mod` before the app DSL
/// evaluates (spec: registration order matters -- see `App::script_mod`).
pub(crate) fn script_mod(vm: &mut ScriptVm) {
    class::script_mod(vm);
    behavior::script_mod(vm);
}
