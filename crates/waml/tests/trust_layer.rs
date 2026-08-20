//! The trust layer's one job: "I could not analyse this" must never be
//! reported as "I analysed this and it is fine".

use waml::diagnostic::{DiagCode, Severity};

#[test]
fn a_clean_bundle_reports_no_errors() {
    let bundle = vec![(
        "order.md".to_string(),
        "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string(),
    )];
    let diagnostics = waml::validate::validate(&bundle);
    let errors: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .collect();
    assert!(
        errors.is_empty(),
        "clean bundle reported errors: {errors:?}"
    );
}

#[test]
fn an_unanalyzable_bundle_is_distinguishable_from_a_clean_one() {
    // A duplicate path cannot form a bundle at all, so analysis never runs.
    // The old contract returned an empty vec here — byte-identical to "clean".
    let bundle = vec![
        ("order.md".to_string(), "# a".to_string()),
        ("order.md".to_string(), "# b".to_string()),
    ];
    let diagnostics = waml::validate::validate(&bundle);
    assert!(
        !diagnostics.is_empty(),
        "a bundle that could not be analysed must not report clean"
    );
    assert!(
        diagnostics
            .iter()
            .any(|d| d.code == DiagCode::AnalysisFailed && d.severity == Severity::Error),
        "expected an analysis-failed error, got: {diagnostics:?}"
    );
}
