pub use makepad_widgets;

// M-8 dead-code posture: a module stays plain `mod` (crate-private) unless it
// is consumed from outside the crate -- by `src/bin/*` harnesses, `tests/`,
// or `main.rs`. `pub` items lose clippy's dead-code checking under
// `-D warnings`, so widening visibility trims the coverage this project
// values; widen only as far as a harness/test import actually demands, never
// preemptively (2026-08-04). A `pub mod` is not a licence for its contents:
// only the items a harness/test/`main.rs` names stay `pub`, everything else in
// it is `pub(crate)`, or the module-wide public reachability silences dead-code
// for the whole file.
mod accent;
mod action_link;
mod agent_mark;
mod api_save;
pub mod app;
mod attr_row;
mod behavior_doc_view;
mod browser_boot;
mod bundle_export;
// `pub`: `crates/waml-editor/src/bin/node_editor_harness.rs` must register
// `canvas::pen::script_mod` in its own DSL chain (it does not go through
// `App::script_mod`). Only `pen` inside stays reachable from outside the
// crate; every other item in `canvas` keeps its existing `pub(crate)` grain.
pub mod canvas;
pub use canvas::{
    measure_node, ActorGeometry, EdgeLineStyle, EdgeNotation, GroupVisualKind,
    MeasuredNodeGeometry, MonoTextMeasurer, NodeVisualKind, Point, Segment, StructuralVisualKind,
    StructuralVisualPolicy, TextMeasurer, UseCaseGeometry,
};
mod book_documents;
mod book_layout;
mod book_model;
mod book_surface;
mod book_view;
mod card;
mod chrome_seam;
mod class_diagram_view;
mod classifier_preview_view;
// Native argv parsing; the wasm build boots from the URL instead.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
mod cli;
mod colors_overlay;
mod config;
mod conflict_badge;
mod cursor;
mod diagram_display;
mod diagram_properties;
mod diagram_switcher;
mod doc_tabs;
mod doc_view;
mod dock;
mod document;
mod document_header;
mod document_host;
mod documents;
mod edge_labels;
pub mod editor_history;
mod editor_session;
mod extension_editor;
mod find_strip;
mod folder_documents;
mod folder_list;
mod folder_projection;
mod folder_view;
pub mod fonts;
mod fonts_overlay;
mod fps_meter;
pub mod frame;
mod generic_okf_view;
mod icon_button;
pub mod icons;
mod icons_overlay;
mod inspector;
mod inspector_panel;
// Native filesystem bundle loading; the wasm build fetches over HTTP.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
mod load;
pub mod logo;
mod markdown_analysis;
// Dead-code checked on native (the gate target); the wasm build never reaches
// the native filesystem plumbing in here (2026-08-04, review M-8 -- this
// replaced an unconditional module-wide `#[allow(dead_code)]`).
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
mod markdown_hosts;
#[cfg(not(target_arch = "wasm32"))]
mod native_save;
mod nav;
mod navigation;
pub mod node_design_editor;
mod node_style;
mod okf_documents;
mod overlay_shell;
mod panel_splitter;
mod platform_browser;
mod popup;
mod project_config;
mod project_settings;
mod property_controls;
mod reading_view;
mod recent_row;
mod ref_card;
mod scene;
mod search_results_view;
mod search_session;
mod search_state;
mod section_heading;
mod select_box;
mod selection_toolbar;
mod shortcuts;
mod shortcuts_overlay;
mod sizing;
mod source_toggle_view;
mod source_view;
mod splitter;
mod start_screen;
mod statusbar;
mod telemetry;
pub mod theme_atlas;
mod tool_dock;
mod tree;
mod tree_layout;
mod tree_panel;
mod tree_row_draw;
mod uml_documents;
mod view_bar;
pub mod view_history;

// Staged: consumers land with the header control (plan
// 2026-08-11-viewer-font-size-control, Tasks 2/8/9 remove this).
#[cfg_attr(not(test), allow(dead_code))]
mod zoom;

#[cfg(test)]
mod script_gate;

// Terminal/mid-route GEOMETRY (offset, align, slide, collision) moved to the
// solver with placement itself and is covered there (`waml::solve::label`).
// What is left here is text-composition policy, already covered by
// `edge_labels`'s own inline tests (`requests_follow_the_display_switches_and_carry_slot_identity`,
// `association_reference_is_not_painted_as_relationship_name`); this module's
// former duplicate coverage (cardinality independence, non-ended relationships,
// role/name display switches) is folded into those.
#[cfg(test)]
mod edge_labels_tests {
    use crate::diagram_display::ResolvedDiagramDisplay;
    use crate::edge_labels::label_requests;
    use crate::scene::SceneEdge;
    use waml::model::{CardinalityVisibility, RelEnd, RelationshipKind};
    use waml::multiplicity::Multiplicity;
    use waml::solve::label::LabelSlot;
    use waml::solve::Rect;

    fn display(
        cardinality: CardinalityVisibility,
        show_cardinality: bool,
    ) -> ResolvedDiagramDisplay {
        ResolvedDiagramDisplay {
            cardinality,
            show_cardinality,
            ..Default::default()
        }
    }

    fn edge(kind: RelationshipKind, from_end: RelEnd, to_end: RelEnd) -> SceneEdge {
        SceneEdge {
            source: Rect {
                x: 0.0,
                y: 0.0,
                w: 20.0,
                h: 20.0,
            },
            target: Rect {
                x: 100.0,
                y: 0.0,
                w: 20.0,
                h: 20.0,
            },
            kind,
            name: None,
            from_end,
            to_end,
            points: vec![(20.0, 10.0), (100.0, 10.0)],
        }
    }

    fn ended_edge(from: Option<Multiplicity>, to: Option<Multiplicity>) -> SceneEdge {
        edge(
            RelationshipKind::Associates,
            RelEnd {
                multiplicity: from,
                ..Default::default()
            },
            RelEnd {
                multiplicity: to,
                ..Default::default()
            },
        )
    }

    fn texts(edges: &[SceneEdge], display: &ResolvedDiagramDisplay) -> Vec<String> {
        label_requests(edges, display)
            .into_iter()
            .map(|r| r.text)
            .collect()
    }

    #[test]
    fn relationship_cardinality_is_independent_and_never_synthesized() {
        let edge = ended_edge(None, Multiplicity::parse("0..*"));
        assert_eq!(
            texts(
                std::slice::from_ref(&edge),
                &display(CardinalityVisibility::Off, true)
            ),
            vec!["{0..*}"]
        );
        assert!(
            texts(
                std::slice::from_ref(&edge),
                &display(CardinalityVisibility::All, false)
            )
            .is_empty(),
            "the relationship toggle must be authoritative"
        );
        assert_eq!(
            texts(&[edge], &display(CardinalityVisibility::All, true)),
            vec!["{0..*}"],
            "enabling relationship cardinality must not synthesize an implicit one"
        );
    }

    #[test]
    fn non_ended_relationships_never_synthesize_default_ends() {
        let edge = edge(
            RelationshipKind::Specializes,
            RelEnd::default(),
            RelEnd::default(),
        );
        assert!(texts(&[edge], &display(CardinalityVisibility::All, true)).is_empty());
    }

    #[test]
    fn roles_and_relationship_names_follow_their_display_switches() {
        let mut edge = ended_edge(None, None);
        edge.from_end.role = Some("orders".into());
        edge.name = Some(waml::model::AssocName::Label("places".into()));
        let mut display = display(CardinalityVisibility::Off, false);
        display.show_roles = false;
        display.show_labels = false;
        assert!(texts(std::slice::from_ref(&edge), &display).is_empty());

        display.show_roles = true;
        assert_eq!(texts(std::slice::from_ref(&edge), &display), vec!["orders"]);

        display.show_labels = true;
        assert_eq!(
            texts(std::slice::from_ref(&edge), &display),
            vec!["orders", "places"]
        );
    }

    #[test]
    fn terminal_requests_carry_the_slot_each_end_belongs_to() {
        let edge = ended_edge(Multiplicity::parse("1"), Multiplicity::parse("0..*"));
        let reqs = label_requests(&[edge], &display(CardinalityVisibility::All, true));
        let slots: Vec<_> = reqs.iter().map(|r| r.slot).collect();
        assert_eq!(slots, vec![LabelSlot::TerminalFrom, LabelSlot::TerminalTo]);
    }
}
