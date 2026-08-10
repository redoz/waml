mod interaction;
mod placement;
mod render;
pub use render::use_case_groups::{commands as use_case_group_commands, UseCaseGroupCommand};
pub use render::use_case_nodes::{commands as use_case_node_commands, UseCaseNodeCommand};
mod selection;
pub(crate) mod use_case_geometry;
pub(crate) mod visual;
mod widget;

use makepad_widgets::DVec2;

pub(crate) use selection::ConstraintVisibility;

pub(super) const SELECT_SLOP: f64 = 4.0;

#[derive(Clone, Debug, PartialEq)]
pub(super) enum ReleaseIntent {
    NotClick,
    Select { key: String },
    Deselect,
    ToggleExpand { key: String },
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum SurfaceIntent {
    NodeMenu {
        abs: DVec2,
        key: String,
    },
    NodeSelect {
        key: String,
    },
    NodeDeselect,
    ToggleExpand {
        key: String,
    },
    CompassArmed {
        subject_key: String,
        reference_key: String,
        center: DVec2,
    },
    DialDismiss,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum TimerCommand {
    Keep,
    StartTimeout(f64),
    RestartTimeout(f64),
    Stop,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FrameCommand {
    Keep,
    Request,
    Stop,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct InteractionEffects {
    pub(super) consumed: bool,
    pub(super) redraw: bool,
    pub(super) dwell_timer: TimerCommand,
    pub(super) preview_frame: FrameCommand,
    pub(super) intent: Option<SurfaceIntent>,
}

impl Default for InteractionEffects {
    fn default() -> Self {
        Self {
            consumed: false,
            redraw: false,
            dwell_timer: TimerCommand::Keep,
            preview_frame: FrameCommand::Keep,
            intent: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum SceneUpdate {
    Replace,
    Focus { key: String },
    PreserveViewport,
}

pub(crate) use placement::{
    zone_arrow, zone_id, zone_of_id, zone_placed, DialPlacement, Placed, Zone, COMPASS_ZONES,
    DIAL_ZONES,
};
pub(crate) use widget::{script_mod, ClassDiagramSurface, ClassDiagramSurfaceAction};
