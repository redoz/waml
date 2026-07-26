mod selection;
mod widget;

pub(crate) use selection::ConstraintVisibility;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum SceneUpdate {
    Replace,
    Focus { key: String },
    PreserveViewport,
}

pub(crate) use widget::{
    script_mod, zone_arrow, zone_id, zone_of_id, zone_placed, ClassDiagramSurface,
    ClassDiagramSurfaceAction, DialPlacement, Placed, Zone, COMPASS_ZONES, DIAL_ZONES,
};
