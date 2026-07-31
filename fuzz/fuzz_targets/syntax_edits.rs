#![no_main]

mod support;

use libfuzzer_sys::fuzz_target;
use waml_syntax::{
    parse_markdown, reparse_markdown, DocumentRevision, MarkdownDialect, MarkdownReparseOutcome,
    TextChange,
};

fuzz_target!(|data: &[u8]| {
    let Some(value) = support::valid_utf8(data) else {
        return;
    };
    let previous = parse_markdown(
        DocumentRevision::INITIAL,
        support::source(value),
        MarkdownDialect::WAML_DEFAULT,
    )
    .expect("bounded UTF-8 markdown parses");
    let (start, end, replacement) = support::derived_valid_edit(data, value);
    let mut candidate = value.to_owned();
    candidate.replace_range(start..end, &replacement);
    let update = reparse_markdown(
        &previous,
        DocumentRevision::new(2),
        support::source(&candidate),
        &[TextChange {
            old_range: support::range(start, end),
            replacement,
        }],
    )
    .expect("valid edit reparses");
    let full = parse_markdown(
        DocumentRevision::INITIAL,
        support::source(&candidate),
        MarkdownDialect::WAML_DEFAULT,
    )
    .expect("candidate fully parses");
    assert_eq!(update.snapshot.tree().write_to_string(), candidate);
    assert_eq!(full.tree().write_to_string(), candidate);
    assert_eq!(
        support::syntax_fingerprint(update.snapshot.tree()),
        support::syntax_fingerprint(full.tree())
    );
    assert_eq!(
        support::diagnostic_fingerprint(update.snapshot.tree()),
        support::diagnostic_fingerprint(full.tree())
    );
    if let MarkdownReparseOutcome::Full { reason } = update.outcome {
        assert!(
            !format!("{reason:?}").is_empty(),
            "full reparse has a named reason"
        );
    }
});
