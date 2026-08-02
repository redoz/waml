use super::*;

fn tab(id: LiveId, key: &str, title: &str, category: TreeKind, preview: bool) -> DocTab {
    DocTab {
        id,
        concept_id: key.into(),
        kind: crate::view_history::DocumentKind::Primary,
        title: title.into(),
        presentation: DocumentPresentation {
            icon: IconSet::icon_for(category).unwrap(),
            accent: None,
            category,
        },
        preview,
    }
}

#[test]
fn document_switcher_items_preserve_order_and_tab_identity() {
    let diagram = LiveId::from_str("diagram");
    let customer = LiveId::from_str("customer");
    let order = LiveId::from_str("order");
    let tabs = OpenTabs {
        tabs: vec![
            tab(diagram, "d", "Diagram", TreeKind::Diagram, false),
            tab(customer, "customer", "Customer", TreeKind::Class, false),
            tab(order, "order", "Order", TreeKind::Class, true),
        ],
        active: order,
    };

    let items = doc_switcher_items(&tabs.tabs);
    assert_eq!(
        items.iter().map(|item| item.id).collect::<Vec<_>>(),
        tabs.tabs.iter().map(|tab| tab.id).collect::<Vec<_>>()
    );
    assert_eq!(
        items
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>(),
        vec!["Diagram", "Customer", "Order"]
    );
    assert!(items.iter().all(|item| item.enabled && !item.danger));
}

#[test]
fn breakpoint_enters_below_640_and_leaves_above_680() {
    assert!(next_narrow(false, 639.9));
    assert!(next_narrow(true, 680.0));
    assert!(!next_narrow(true, 680.1));
}

#[test]
fn breakpoint_preserves_mode_through_the_hysteresis_band() {
    for width in [640.0, 650.0, 680.0] {
        assert!(!next_narrow(false, width));
        assert!(next_narrow(true, width));
    }
}

#[test]
fn only_the_open_narrow_panel_counts_as_inside() {
    let canvas = Rect {
        pos: dvec2(0.0, 66.0),
        size: dvec2(390.0, 700.0),
    };
    let tree = Rect {
        pos: dvec2(0.0, 66.0),
        size: dvec2(280.0, 700.0),
    };
    let inspector = Rect {
        pos: dvec2(70.0, 66.0),
        size: dvec2(320.0, 700.0),
    };
    assert!(open_overlay_contains(
        dvec2(100.0, 200.0),
        DockState::Pinned,
        tree,
        DockState::Flag,
        inspector
    ));
    assert!(!open_overlay_contains(
        dvec2(300.0, 200.0),
        DockState::Pinned,
        tree,
        DockState::Flag,
        inspector
    ));
    assert!(should_dismiss_narrow_dock(
        dvec2(300.0, 200.0),
        canvas,
        DockState::Pinned,
        tree,
        DockState::Flag,
        inspector
    ));
    assert!(!should_dismiss_narrow_dock(
        dvec2(16.0, 50.0),
        canvas,
        DockState::Pinned,
        tree,
        DockState::Flag,
        inspector
    ));
}

#[test]
fn conflict_delete_maps_to_place_rm() {
    let action = ConflictListAction::Delete {
        subject: "order".to_string(),
        reference: "payment-gateway".to_string(),
    };
    let op = place_rm_for("dia", &action);
    assert_eq!(
        op,
        Some(waml::uml::Op::PlacementRemove {
            diagram: "dia".to_string(),
            subject_slug: "order".to_string(),
            reference_slug: "payment-gateway".to_string(),
        })
    );
}

#[test]
fn conflict_focus_never_maps_to_an_op() {
    let action = ConflictListAction::Focus {
        subject: "order".to_string(),
        reference: "payment-gateway".to_string(),
    };
    assert_eq!(place_rm_for("dia", &action), None);
    assert_eq!(place_rm_for("dia", &ConflictListAction::None), None);
}

// End-to-end at the ops layer (no live `Cx`/`App` needed): the mapped
// `Op::PlaceRm` removes ONLY the targeted placement from the re-serialized
// bundle, leaving an unrelated one intact. The solver's dropped/
// conflicts_with reporting is already covered by Task 1's `waml::ops`
// tests and `scene.rs`'s `project_conflicts` tests.
#[test]
fn conflict_delete_removes_only_the_targeted_placement() {
    let source = waml::source::SourceBundle::try_from_pairs([(
        "shop/dia.md".to_string(),
        "---\ntype: Diagram\ntitle: D\nprofile: uml-domain\n---\n# D\n\n## Layout\n\
         - [Order](./order.md) left of [PaymentGateway](./payment-gateway.md)\n\
         - [Customer](./customer.md) below [Order](./order.md)\n"
            .to_string(),
    )])
    .unwrap();
    let prepared = waml::analysis::prepare_candidate(source.clone(), None, 1).unwrap();
    let action = ConflictListAction::Delete {
        subject: "order".to_string(),
        reference: "payment-gateway".to_string(),
    };
    let op = place_rm_for("dia", &action).expect("Delete maps to an Op");
    let out = waml::edit::EditBatch::lower(
        &waml::uml::Batch(vec![op]),
        waml::edit::EditContext {
            source: &source,
            okf_analysis: prepared.okf(),
            session_revision: prepared.revision(),
            uml: prepared.uml(),
        },
    )
    .unwrap();
    let markdown = out.document_by_concept_id("shop/dia").unwrap().text();
    assert!(
        !markdown.contains("left of"),
        "the deleted placement is gone: {markdown}"
    );
    assert!(
        markdown.contains("below"),
        "the OTHER placement survives: {markdown}"
    );
}

#[test]
fn logo_command_for_maps_ids_and_rejects_others() {
    assert_eq!(
        logo_command_for(live_id!(properties)),
        Some(LogoCommand::Properties)
    );
    assert_eq!(logo_command_for(live_id!(about)), Some(LogoCommand::About));
    assert_eq!(logo_command_for(live_id!(fonts)), Some(LogoCommand::Fonts));
    assert_eq!(logo_command_for(live_id!(icons)), Some(LogoCommand::Icons));
    assert_eq!(
        logo_command_for(live_id!(colors)),
        Some(LogoCommand::Colors)
    );
    assert_eq!(logo_command_for(live_id!(exit)), Some(LogoCommand::Exit));
    // Cancel maps to nothing (the radial just closes on commit).
    assert_eq!(logo_command_for(live_id!(cancel)), None);
    // A node-radial id / unknown id is not ours.
    assert_eq!(logo_command_for(live_id!(remove)), None);
    assert_eq!(logo_command_for(live_id!(nonsense)), None);
}
