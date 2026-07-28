pub use makepad_widgets;
use makepad_widgets::*;

mod accent;
mod action_link;
mod agent_mark;
mod app;
mod attr_row;
mod canvas;
mod card;
mod class_diagram_view;
mod classifier_preview_view;
mod cli;
mod colors_overlay;
mod config;
mod conflict_badge;
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
mod editor_session;
mod fonts;
mod fonts_overlay;
mod fps_meter;
mod frame;
mod generic_okf_view;
mod icon_button;
mod icons;
mod icons_overlay;
mod inspector;
mod inspector_panel;
mod load;
mod logo;
mod markdown_surface;
#[cfg(not(target_arch = "wasm32"))]
mod native_save;
mod nav;
mod navigation;
mod node_design_editor;
mod node_style;
mod okf_documents;
mod overlay_shell;
mod popup;
mod property_controls;
mod recent_row;
mod ref_card;
mod scene;
mod section_heading;
mod select_box;
mod selection_toolbar;
mod shortcuts_overlay;
mod sizing;
mod source_view;
mod start_screen;
mod statusbar;
mod theme_atlas;
mod tool_dock;
mod tree;
mod tree_panel;
mod uml_documents;
mod view_bar;

#[cfg(test)]
mod script_gate;

use app::App;

app_main!(App);

#[cfg(test)]
mod edge_labels_tests {
    use crate::diagram_display::ResolvedDiagramDisplay;
    use crate::edge_labels::{edge_end_labels, LabelAlign};
    use crate::scene::SceneEdge;
    use waml::model::{CardinalityVisibility, RelEnd, RelationshipKind};
    use waml::multiplicity::Multiplicity;
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

    fn texts(labels: Vec<crate::edge_labels::EdgeLabel>) -> Vec<String> {
        labels.into_iter().map(|label| label.text).collect()
    }

    #[test]
    fn relationship_cardinality_is_independent_and_never_synthesized() {
        let edge = ended_edge(None, Multiplicity::parse("0..*"));
        assert_eq!(
            texts(edge_end_labels(
                &edge,
                &display(CardinalityVisibility::Off, true)
            )),
            vec!["{0..*}"]
        );
        assert!(
            edge_end_labels(&edge, &display(CardinalityVisibility::All, false)).is_empty(),
            "the relationship toggle must be authoritative"
        );
        assert_eq!(
            texts(edge_end_labels(
                &edge,
                &display(CardinalityVisibility::All, true)
            )),
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
        assert!(edge_end_labels(&edge, &display(CardinalityVisibility::All, true)).is_empty());
    }

    #[test]
    fn roles_and_relationship_names_follow_their_display_switches() {
        let mut edge = ended_edge(None, None);
        edge.from_end.role = Some("orders".into());
        edge.name = Some(waml::model::AssocName::Label("places".into()));
        let mut display = display(CardinalityVisibility::Off, false);
        display.show_roles = false;
        display.show_labels = false;
        assert!(edge_end_labels(&edge, &display).is_empty());

        display.show_roles = true;
        assert_eq!(texts(edge_end_labels(&edge, &display)), vec!["orders"]);

        display.show_labels = true;
        assert_eq!(
            texts(edge_end_labels(&edge, &display)),
            vec!["orders", "places"]
        );
    }

    #[test]
    fn horizontal_terminal_labels_move_into_open_space() {
        let labels = edge_end_labels(
            &ended_edge(Multiplicity::parse("1"), Multiplicity::parse("0..*")),
            &display(CardinalityVisibility::All, true),
        );
        assert_eq!(labels[0].align, LabelAlign::Right);
        assert!(labels[0].anchor.0 > 20.0);
        assert_eq!(labels[1].align, LabelAlign::Left);
        assert!(labels[1].anchor.0 < 100.0);
    }

    #[test]
    fn vertical_terminal_labels_move_into_open_space() {
        let mut edge = ended_edge(Multiplicity::parse("1"), Multiplicity::parse("0..*"));
        edge.points = vec![(10.0, 20.0), (10.0, 100.0)];
        let labels = edge_end_labels(&edge, &display(CardinalityVisibility::All, true));
        assert_eq!(labels[0].align, LabelAlign::Below);
        assert!(labels[0].anchor.1 > 20.0);
        assert_eq!(labels[1].align, LabelAlign::Above);
        assert!(labels[1].anchor.1 < 100.0);
    }
}
