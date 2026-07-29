#![no_main]

mod support;

use libfuzzer_sys::fuzz_target;
use waml_syntax::{
    parse_okf_markdown, reparse_okf_markdown, MarkdownDialect, ReparseOutcome, TextChange,
};

fuzz_target!(|data: &[u8]| {
    let Some(value) = support::valid_utf8(data) else {
        return;
    };
    let previous = parse_okf_markdown(support::source(value), MarkdownDialect::CommonMarkCurrent)
        .expect("bounded UTF-8 shell parses");
    let (start, end, replacement) = support::derived_valid_edit(data, value);
    let mut candidate = value.to_owned();
    candidate.replace_range(start..end, &replacement);
    let change = TextChange {
        old_range: support::range(start, end),
        replacement,
    };
    let reparsed = reparse_okf_markdown(
        &previous.tree,
        support::source(&candidate),
        std::slice::from_ref(&change),
    )
    .expect("valid edit reparses");
    let incremental = match reparsed {
        ReparseOutcome::Incremental { tree, .. } | ReparseOutcome::Full { tree, .. } => tree,
    };
    let full = parse_okf_markdown(
        support::source(&candidate),
        MarkdownDialect::CommonMarkCurrent,
    )
    .expect("candidate fully parses");

    support::assert_tree_ranges(&incremental, &candidate);
    support::assert_tree_ranges(&full.tree, &candidate);
    assert_eq!(incremental.write_to_string(), candidate);
    assert_eq!(full.tree.write_to_string(), candidate);
    assert_eq!(
        support::syntax_fingerprint(&incremental),
        support::syntax_fingerprint(&full.tree)
    );
    assert_eq!(
        support::diagnostic_fingerprint(&incremental),
        support::diagnostic_fingerprint(&full.tree)
    );
});
