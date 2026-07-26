mod class;
mod geometry;
mod viewport;

pub(crate) use class::script_mod;
pub(crate) use class::{
    zone_arrow, zone_id, zone_of_id, zone_placed, ClassDiagramSurface, ClassDiagramSurfaceAction,
    ConstraintVisibility, COMPASS_ZONES, DIAL_ZONES,
};
// Approved façade types can be consumed through method signatures and type
// inference without a direct named use in this binary crate.
#[allow(unused_imports)]
pub(crate) use class::{DialPlacement, Placed, Zone};
pub(crate) use viewport::ZOOM_STEP;
