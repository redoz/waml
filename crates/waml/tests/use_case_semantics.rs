use std::sync::Arc;

use waml::{
    analysis::{prepare_candidate, PreviousAnalyses},
    diagnostic::DiagCode,
    model::DiagramGroupRole,
    source::SourceBundle,
};

fn document(ty: &str, title: &str, body: &str) -> String {
    format!("---\ntype: {ty}\n---\n# {title}\n{body}")
}

fn candidate(
    diagram_body: &str,
    documents: &[(&str, &str, &str)],
) -> waml::analysis::PreparedCandidate {
    let mut pairs = vec![(
        "diagram.md".to_string(),
        document("uml.UseCaseDiagram", "Use cases", diagram_body),
    )];
    pairs.extend(
        documents
            .iter()
            .map(|(path, ty, title)| ((*path).to_string(), document(ty, title, ""))),
    );
    prepare_candidate(SourceBundle::try_from_pairs(pairs).unwrap(), None, 0).unwrap()
}

fn diagram(candidate: &waml::analysis::PreparedCandidate) -> &waml::model::Diagram {
    candidate
        .uml()
        .projection
        .diagrams
        .iter()
        .find(|diagram| diagram.key == "diagram")
        .expect("valid use-case diagram projection")
}

fn assert_code(candidate: &waml::analysis::PreparedCandidate, code: DiagCode) {
    assert!(
        candidate
            .uml()
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == code),
        "missing {code:?}; got {:#?}",
        candidate.uml().diagnostics
    );
}

fn assert_rejected(candidate: &waml::analysis::PreparedCandidate) {
    assert!(
        candidate
            .uml()
            .projection
            .diagrams
            .iter()
            .all(|diagram| diagram.key != "diagram"),
        "invalid use-case diagram was projected"
    );
}

#[test]
fn valid_actor_group_has_external_actor_role() {
    let found = candidate(
        "\n## Members\n\n### People\n- [Buyer](./buyer.md)\n",
        &[("buyer.md", "uml.Actor", "Buyer")],
    );

    assert_eq!(
        diagram(&found).groups[0].role,
        DiagramGroupRole::ExternalActors
    );
}

#[test]
fn valid_direct_system_boundary_has_boundary_role() {
    let found = candidate(
        "\n## Members\n\n### Checkout\n- [Pay](./pay.md)\n",
        &[("pay.md", "uml.UseCase", "Pay")],
    );

    assert_eq!(
        diagram(&found).groups[0].role,
        DiagramGroupRole::SystemBoundary
    );
}

#[test]
fn boundary_accepts_use_cases_only_in_valid_bands() {
    let found = candidate(
        "\n## Members\n\n### Checkout\n\n#### Purchase\n- [Pay](./pay.md)\n\n#### Support\n- [Refund](./refund.md)\n",
        &[
            ("pay.md", "uml.UseCase", "Pay"),
            ("refund.md", "uml.UseCase", "Refund"),
        ],
    );

    let boundary = &diagram(&found).groups[0];
    assert_eq!(boundary.role, DiagramGroupRole::SystemBoundary);
    assert_eq!(
        boundary
            .children
            .iter()
            .map(|group| group.role)
            .collect::<Vec<_>>(),
        [DiagramGroupRole::Band, DiagramGroupRole::Band]
    );
}

#[test]
fn recursively_resolved_actor_package_is_allowed_in_actor_group() {
    let mut pairs = vec![
        (
            "diagram.md".to_string(),
            document(
                "uml.UseCaseDiagram",
                "Use cases",
                "\n## Members\n\n### People\n- [Accounts](./accounts.md)\n",
            ),
        ),
        (
            "accounts.md".to_string(),
            document(
                "uml.Package",
                "Accounts",
                "\n## Members\n- [Nested](./nested.md)\n- [Reminder](./reminder.md)\n",
            ),
        ),
        (
            "nested.md".to_string(),
            document(
                "uml.Package",
                "Nested",
                "\n## Members\n- [Buyer](./buyer.md)\n",
            ),
        ),
        ("buyer.md".to_string(), document("uml.Actor", "Buyer", "")),
        (
            "reminder.md".to_string(),
            document("uml.Note", "Reminder", ""),
        ),
    ];
    let found = prepare_candidate(
        SourceBundle::try_from_pairs(pairs.drain(..)).unwrap(),
        None,
        0,
    )
    .unwrap();

    assert_eq!(
        diagram(&found).groups[0].role,
        DiagramGroupRole::ExternalActors
    );
}

#[test]
fn note_only_top_level_group_is_rejected() {
    let found = candidate(
        "\n## Members\n\n### Information\n- [Reminder](./reminder.md)\n",
        &[("reminder.md", "uml.Note", "Reminder")],
    );

    assert_code(&found, DiagCode::InvalidUseCaseGroup);
    assert_rejected(&found);
}

#[test]
fn empty_top_level_group_is_rejected() {
    let found = candidate("\n## Members\n\n### Empty\n", &[]);

    assert_code(&found, DiagCode::InvalidUseCaseGroup);
    assert_rejected(&found);
}

#[test]
fn band_without_use_case_is_rejected() {
    let found = candidate(
        "\n## Members\n\n### Checkout\n- [Pay](./pay.md)\n\n#### Empty band\n- [Reminder](./reminder.md)\n",
        &[
            ("pay.md", "uml.UseCase", "Pay"),
            ("reminder.md", "uml.Note", "Reminder"),
        ],
    );

    assert_code(&found, DiagCode::EmptyUseCaseBand);
    assert_rejected(&found);
}

#[test]
fn actor_group_cannot_have_child_group() {
    let found = candidate(
        "\n## Members\n\n### People\n- [Buyer](./buyer.md)\n\n#### Details\n- [Reminder](./reminder.md)\n",
        &[
            ("buyer.md", "uml.Actor", "Buyer"),
            ("reminder.md", "uml.Note", "Reminder"),
        ],
    );

    assert_code(&found, DiagCode::InvalidUseCaseGroup);
    assert_rejected(&found);
}

#[test]
fn band_cannot_have_child_group() {
    let found = candidate(
        "\n## Members\n\n### Checkout\n\n#### Purchase\n- [Pay](./pay.md)\n\n##### Details\n- [Reminder](./reminder.md)\n",
        &[
            ("pay.md", "uml.UseCase", "Pay"),
            ("reminder.md", "uml.Note", "Reminder"),
        ],
    );

    assert_code(&found, DiagCode::InvalidUseCaseGroup);
    assert_rejected(&found);
}

#[test]
fn actor_inside_system_boundary_is_rejected() {
    let found = candidate(
        "\n## Members\n\n### Checkout\n- [Buyer](./buyer.md)\n- [Pay](./pay.md)\n",
        &[
            ("buyer.md", "uml.Actor", "Buyer"),
            ("pay.md", "uml.UseCase", "Pay"),
        ],
    );

    assert_code(&found, DiagCode::ActorInsideSystemBoundary);
    assert_rejected(&found);
}

#[test]
fn actor_package_inside_system_boundary_is_rejected_as_actor_containment() {
    let pairs = [
        (
            "diagram.md",
            document(
                "uml.UseCaseDiagram",
                "Use cases",
                "\n## Members\n\n### Checkout\n- [People](./people.md)\n- [Pay](./pay.md)\n",
            ),
        ),
        (
            "people.md",
            document(
                "uml.Package",
                "People",
                "\n## Members\n- [Buyer](./buyer.md)\n",
            ),
        ),
        ("buyer.md", document("uml.Actor", "Buyer", "")),
        ("pay.md", document("uml.UseCase", "Pay", "")),
    ];
    let found = prepare_candidate(SourceBundle::try_from_pairs(pairs).unwrap(), None, 0).unwrap();

    assert_code(&found, DiagCode::ActorInsideSystemBoundary);
    assert_rejected(&found);
}

#[test]
fn use_case_outside_system_boundary_is_rejected() {
    let found = candidate(
        "\n## Members\n- [Loose](./loose.md)\n",
        &[("loose.md", "uml.UseCase", "Loose")],
    );

    assert_code(&found, DiagCode::UseCaseOutsideSystemBoundary);
    assert_rejected(&found);
}

#[test]
fn use_case_in_two_boundaries_is_rejected() {
    let found = candidate(
        "\n## Members\n\n### First\n- [Pay](./pay.md)\n\n### Second\n- [Pay](./pay.md)\n",
        &[("pay.md", "uml.UseCase", "Pay")],
    );

    assert_code(&found, DiagCode::UseCaseInMultipleSystemBoundaries);
    assert_rejected(&found);
}

#[test]
fn incompatible_group_member_is_rejected() {
    let found = candidate(
        "\n## Members\n\n### People\n- [Buyer](./buyer.md)\n- [Account](./account.md)\n",
        &[
            ("buyer.md", "uml.Actor", "Buyer"),
            ("account.md", "uml.Class", "Account"),
        ],
    );

    assert_code(&found, DiagCode::InvalidUseCaseGroup);
    assert_rejected(&found);
}

#[test]
fn unresolved_layout_reference_keeps_existing_diagnostic() {
    let found = candidate(
        "\n## Members\n\n### Checkout\n- [Pay](./pay.md)\n\n## Layout\n- Missing left of Checkout\n",
        &[("pay.md", "uml.UseCase", "Pay")],
    );

    assert_code(&found, DiagCode::UnresolvedLayoutRef);
    assert_eq!(
        diagram(&found).groups[0].role,
        DiagramGroupRole::SystemBoundary
    );
}

#[test]
fn roles_depend_on_resolved_contents_not_english_group_titles() {
    let found = candidate(
        "\n## Members\n\n### Copper\n- [Buyer](./buyer.md)\n\n### Forty two\n\n#### Azure\n- [Pay](./pay.md)\n",
        &[
            ("buyer.md", "uml.Actor", "Buyer"),
            ("pay.md", "uml.UseCase", "Pay"),
        ],
    );

    let groups = &diagram(&found).groups;
    assert_eq!(groups[0].role, DiagramGroupRole::ExternalActors);
    assert_eq!(groups[1].role, DiagramGroupRole::SystemBoundary);
    assert_eq!(groups[1].children[0].role, DiagramGroupRole::Band);
}

#[test]
fn class_diagram_groups_keep_generic_role() {
    let source = SourceBundle::try_from_pairs([
        (
            "diagram.md",
            document(
                "uml.ClassDiagram",
                "Classes",
                "\n## Members\n\n### Domain\n- [Account](./account.md)\n",
            ),
        ),
        ("account.md", document("uml.Class", "Account", "")),
    ])
    .unwrap();
    let found = prepare_candidate(source, None, 0).unwrap();

    assert_eq!(diagram(&found).groups[0].role, DiagramGroupRole::Generic);
}

#[test]
fn invalid_edit_rejects_projection_and_retains_last_valid_scene() {
    let baseline_source = SourceBundle::try_from_pairs([
        (
            "diagram.md",
            document(
                "uml.UseCaseDiagram",
                "Use cases",
                "\n## Members\n\n### Checkout\n- [Pay](./pay.md)\n",
            ),
        ),
        ("pay.md", document("uml.UseCase", "Pay", "")),
        ("reminder.md", document("uml.Note", "Reminder", "")),
    ])
    .unwrap();
    let baseline = prepare_candidate(baseline_source, None, 1).unwrap();
    let retained = baseline.uml().diagram("diagram").unwrap().clone();
    let invalid_source = SourceBundle::try_from_pairs([
        (
            "diagram.md",
            document(
                "uml.UseCaseDiagram",
                "Use cases",
                "\n## Members\n\n### Information\n- [Reminder](./reminder.md)\n",
            ),
        ),
        ("pay.md", document("uml.UseCase", "Pay", "")),
        ("reminder.md", document("uml.Note", "Reminder", "")),
    ])
    .unwrap();
    let invalid = prepare_candidate(
        invalid_source,
        Some(PreviousAnalyses {
            okf: baseline.okf(),
            uml: baseline.uml(),
        }),
        2,
    )
    .unwrap();

    assert_rejected(&invalid);
    assert!(Arc::ptr_eq(
        invalid.uml().diagram("diagram").expect("retained diagram"),
        &retained
    ));
}
