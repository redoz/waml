use makepad_widgets::*;

mod behavior;
mod class;
pub use class::use_case_geometry::{
    measure_node, ActorGeometry, MeasuredNodeGeometry, MonoTextMeasurer, Point, Segment,
    TextMeasurer, UseCaseGeometry,
};
pub use class::visual::{
    EdgeLineStyle, EdgeNotation, GroupVisualKind, NodeVisualKind, StructuralVisualKind,
    StructuralVisualPolicy,
};
mod geometry;
// `pub`: `crates/waml-editor/src/bin/node_editor_harness.rs` registers
// `canvas::pen::script_mod` directly (see `lib.rs`'s `pub mod canvas`).
pub mod pen;
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
