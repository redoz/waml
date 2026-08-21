use std::{collections::HashSet, sync::Arc};

use waml_syntax::{
    parse_markdown, reparse_markdown, DocumentRevision, FullReparseReason, GreenElement, GreenText,
    MarkdownDialect, MarkdownLinkKind, MarkdownReparseOutcome, SourceText, SyntaxElement,
    SyntaxIdentity, SyntaxNode, TextChange, TextRange, TextSize,
};

fn text(value: &str) -> SourceText {
    SourceText::new(value).unwrap()
}

fn range(start: usize, end: usize) -> TextRange {
    TextRange::new(
        TextSize::try_from_usize(start).unwrap(),
        TextSize::try_from_usize(end).unwrap(),
    )
    .unwrap()
}

fn intersects(left: TextRange, right: TextRange) -> bool {
    left.start() < right.end() && right.start() < left.end()
}

fn reference_destinations(snapshot: &waml_syntax::MarkdownSyntaxSnapshot) -> Vec<String> {
    snapshot
        .queries()
        .links()
        .filter(|link| link.kind == MarkdownLinkKind::Reference)
        .map(|link| link.destination.to_string())
        .collect()
}

fn source_independent_addresses(
    element: &GreenElement<waml_syntax::OkfMarkdownLanguage>,
    addresses: &mut HashSet<usize>,
) {
    match element {
        GreenElement::Node(node) => {
            if node.is_source_independent() {
                addresses.insert(Arc::as_ptr(node) as usize);
            }
            for child in node.children() {
                source_independent_addresses(child, addresses);
            }
        }
        GreenElement::Token(token) if token.is_source_independent() => {
            addresses.insert(Arc::as_ptr(token) as usize);
        }
        GreenElement::Token(_) => {}
    }
}

fn has_shared_source_independent_green(
    previous: &waml_syntax::MarkdownSyntaxSnapshot,
    next: &waml_syntax::MarkdownSyntaxSnapshot,
) -> bool {
    let mut old = HashSet::new();
    let mut new = HashSet::new();
    source_independent_addresses(
        &GreenElement::Node(previous.tree().root_green().clone()),
        &mut old,
    );
    source_independent_addresses(
        &GreenElement::Node(next.tree().root_green().clone()),
        &mut new,
    );
    !old.is_disjoint(&new)
}

fn assert_source_backed_green_uses(
    element: &GreenElement<waml_syntax::OkfMarkdownLanguage>,
    source: &Arc<String>,
) {
    match element {
        GreenElement::Node(node) => {
            for child in node.children() {
                assert_source_backed_green_uses(child, source);
            }
        }
        GreenElement::Token(token) => {
            for text in std::iter::once(token.text()).chain(
                token
                    .leading_trivia()
                    .iter()
                    .chain(token.trailing_trivia())
                    .map(|trivia| &trivia.text),
            ) {
                if let GreenText::SourceSlice {
                    source: token_source,
                    ..
                } = text
                {
                    assert!(Arc::ptr_eq(source, token_source.shared()));
                }
            }
        }
    }
}

fn node_for_identity(
    node: SyntaxNode<waml_syntax::OkfMarkdownLanguage>,
    identity: SyntaxIdentity,
) -> Option<SyntaxNode<waml_syntax::OkfMarkdownLanguage>> {
    if waml_syntax::syntax_identity(&node) == Some(identity) {
        return Some(node);
    }
    node.children().find_map(|child| match child {
        SyntaxElement::Node(child) => node_for_identity(child, identity),
        SyntaxElement::Token(_) => None,
    })
}

fn structural_fingerprint(
    snapshot: &waml_syntax::MarkdownSyntaxSnapshot,
) -> Vec<(String, TextRange)> {
    fn collect(
        node: SyntaxNode<waml_syntax::OkfMarkdownLanguage>,
        out: &mut Vec<(String, TextRange)>,
    ) {
        out.push((format!("{:?}", node.kind()), node.range()));
        for child in node.children() {
            match child {
                SyntaxElement::Node(child) => collect(child, out),
                SyntaxElement::Token(token) => {
                    out.push((format!("{:?}", token.kind()), token.range()));
                }
            }
        }
    }
    let mut out = Vec::new();
    collect(snapshot.tree().root(), &mut out);
    out
}

fn semantic_fingerprint(
    snapshot: &waml_syntax::MarkdownSyntaxSnapshot,
) -> (Vec<String>, Vec<String>, Vec<String>, Vec<String>) {
    let links = snapshot
        .queries()
        .links()
        .map(|link| {
            // `destination_range` is part of the fingerprint, not an extra: it
            // is the one link value read from bytes that may sit outside the
            // link -- a reference use reads it from its definition -- so it is
            // the one a reparse that reuses the use's green can get wrong
            // while every other value still agrees.
            format!(
                "{:?}:{:?}:{:?}:{:?}:{:?}",
                link.kind, link.source_range, link.destination, link.destination_range, link.title
            )
        })
        .collect();
    let diagnostics = snapshot
        .diagnostics()
        .iter()
        .map(|diagnostic| {
            format!(
                "{:?}:{:?}:{:?}:{}",
                diagnostic.code, diagnostic.severity, diagnostic.range, diagnostic.message
            )
        })
        .collect();
    let structure = vec![format!(
        "{:?}:{:?}:{:?}:{:?}:{:?}:{:?}",
        snapshot.structure().headings,
        snapshot.structure().nested_headings,
        snapshot.structure().protected_ranges,
        snapshot.structure().list_item_lines,
        snapshot.structure().tab_indented_item_lines,
        snapshot.structure().opaque_ranges,
    )];
    let islands = snapshot
        .structure()
        .islands
        .iter()
        .map(|island| {
            let owner = node_for_identity(snapshot.tree().root(), island.owner)
                .map(|node| (node.kind(), node.range()));
            format!(
                "{:?}:{:?}:{:?}:{owner:?}",
                island.kind, island.heading_range, island.content_range,
            )
        })
        .collect();
    (links, diagnostics, structure, islands)
}

fn assert_snapshot_matches_full_oracle(snapshot: &waml_syntax::MarkdownSyntaxSnapshot, new: &str) {
    let full = parse_markdown(
        DocumentRevision::new(snapshot.revision().get() + 1),
        text(new),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    assert_eq!(snapshot.text().shared().as_str(), new);
    assert_eq!(snapshot.tree().write_to_string(), new);
    assert_eq!(
        structural_fingerprint(snapshot),
        structural_fingerprint(&full)
    );
    assert_eq!(semantic_fingerprint(snapshot), semantic_fingerprint(&full));
    assert_eq!(
        reference_destinations(snapshot),
        reference_destinations(&full)
    );
}

fn assert_matches_full_oracle(old: &str, new: &str, changes: &[TextChange]) {
    let previous = parse_markdown(
        DocumentRevision::INITIAL,
        text(old),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    let update = reparse_markdown(&previous, DocumentRevision::new(1), text(new), changes).unwrap();
    assert_snapshot_matches_full_oracle(&update.snapshot, new);
    for window in update.affected_ranges.windows(2) {
        assert!(window[0].end() < window[1].start());
    }
    assert!(update
        .affected_ranges
        .iter()
        .all(|affected| affected.start() < affected.end()));
    for island in update.snapshot.structure().islands.iter() {
        assert!(node_for_identity(update.snapshot.tree().root(), island.owner).is_some());
    }
}

#[test]
fn definition_change_updates_non_contiguous_reference_dependents() {
    // This fails if definition edits only reparse their local shell window and
    // leave reference annotations in non-contiguous paragraphs unchanged.
    let old = "[id]: /one\n\nfirst [a][id]\n\nsecond [b][id]\n";
    let new = "[id]: /two\n\nfirst [a][id]\n\nsecond [b][id]\n";
    let previous = parse_markdown(
        DocumentRevision::INITIAL,
        text(old),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();

    let update = reparse_markdown(
        &previous,
        DocumentRevision::new(1),
        text(new),
        &[TextChange {
            old_range: range(6, 10),
            replacement: Arc::from("/two"),
        }],
    )
    .unwrap();
    let oracle = parse_markdown(
        DocumentRevision::new(1),
        text(new),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();

    assert!(
        matches!(update.outcome, MarkdownReparseOutcome::Incremental { .. }),
        "unexpected outcome: {:?}",
        update.outcome,
    );
    assert_eq!(
        reference_destinations(&update.snapshot),
        vec!["/two", "/two"]
    );
    assert_eq!(
        reference_destinations(&update.snapshot),
        reference_destinations(&oracle)
    );
    assert_snapshot_matches_full_oracle(&update.snapshot, new);
    assert!(
        matches!(
            &update.outcome,
            MarkdownReparseOutcome::Incremental {
                reparsed_range: None,
                ..
            }
        ),
        "unexpected outcome: {:?}",
        update.outcome
    );
    assert!(update.affected_ranges.len() >= 2);
}

#[test]
fn inline_link_before_reference_use_still_forces_reference_fallback() {
    // This fails if the reference-label scanner treats an inline link's `(...)`
    // destination as consuming the rest of the line, hiding a reference use
    // (`[b][id]`) that follows an inline link (`[a](x)`) on the same line.
    // The edit lands on the use line itself (not the definition line) so it
    // exercises `reference_labels` via `change_may_affect_reference_use`
    // directly: a buggy scanner never sees the `[b][id]` label on this line,
    // so it never forces the full-parse fallback the definition lies outside
    // a window for.
    let old = "# A\n\n[id]: /one\n\n# B\n\nsee [a](x) then [b][id]\n";
    let new = "# A\n\n[id]: /one\n\n# B\n\nsee [a](y) then [b][id]\n";
    let previous = parse_markdown(
        DocumentRevision::INITIAL,
        text(old),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    let edit_at = old.find("(x)").unwrap() + 1;
    let update = reparse_markdown(
        &previous,
        DocumentRevision::new(1),
        text(new),
        &[TextChange {
            old_range: range(edit_at, edit_at + 1),
            replacement: Arc::from("y"),
        }],
    )
    .unwrap();

    assert_eq!(reference_destinations(&update.snapshot), vec!["/one"]);
    assert_snapshot_matches_full_oracle(&update.snapshot, new);
}

#[test]
fn inline_link_before_reference_use_in_separate_paragraph_still_resolves() {
    // Mirrors the issue text: the edit lands in a paragraph that shares a
    // shell window (heading section "# B") with the mixed
    // inline-link-then-reference-use paragraph, but the window excludes the
    // definition's heading section ("# A"). This exercises
    // `window_reparse_may_lose_reference_resolution` rather than
    // `change_may_affect_reference_use`: a buggy scanner never notices the
    // window contains a `[b][id]` use, so it never forces the full-parse
    // fallback needed because the definition lies outside the window.
    let old = "# A\n\n[id]: /one\n\n# B\n\nmiddle paragraph\n\nsee [a](x) then [b][id]\n";
    let new = "# A\n\n[id]: /one\n\n# B\n\nMIDDLE paragraph\n\nsee [a](x) then [b][id]\n";
    let previous = parse_markdown(
        DocumentRevision::INITIAL,
        text(old),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    let edit_at = old.find("middle").unwrap();
    let update = reparse_markdown(
        &previous,
        DocumentRevision::new(1),
        text(new),
        &[TextChange {
            old_range: range(edit_at, edit_at + "middle".len()),
            replacement: Arc::from("MIDDLE"),
        }],
    )
    .unwrap();

    assert_eq!(reference_destinations(&update.snapshot), vec!["/one"]);
    assert_snapshot_matches_full_oracle(&update.snapshot, new);
}

#[test]
fn a_reference_use_a_failed_inline_destination_follows_still_forces_a_reparse() {
    // This fails if the label scan reads every `(` after a `]` as an inline
    // link's destination. A `(` only makes one when what follows really is a
    // destination: `[id](` never closes, and `[id](a b)` closes around two
    // words that are no destination at all, so CommonMark falls back to
    // reading `[id]` as a shortcut reference use with the parenthesised tail
    // as plain text. The scan named no label for either, so the guard waved
    // the window through, and the window reparse -- which cannot see the
    // definition in the first section -- published `Text` where a full parse
    // has a `Link`.
    assert_matches_full_oracle(
        "[id]: /one\n\n# M\n\nuse [iz](\n",
        "[id]: /one\n\n# M\n\nuse [id](\n",
        &[TextChange {
            old_range: range(23, 24),
            replacement: Arc::from("d"),
        }],
    );
    assert_matches_full_oracle(
        "[id]: /one\n\n# M\n\nuse [iz](a b)\n",
        "[id]: /one\n\n# M\n\nuse [id](a b)\n",
        &[TextChange {
            old_range: range(23, 24),
            replacement: Arc::from("d"),
        }],
    );
}

#[test]
fn a_reference_use_inside_a_failed_inline_destination_still_forces_a_reparse() {
    // The other half of the same reading. The scan skipped the parenthesised
    // tail whole, on the same assumption that it holds a destination -- and a
    // destination holds no reference uses. `(z [id])` is no destination, so
    // `[]` is plain text and the `[id]` inside the parens is a shortcut
    // reference use. Skipping the tail named no label for it, the guard waved
    // the window through, and the window reparse -- which cannot see the
    // definition in the first section -- published `Text` where a full parse
    // has a `Link`.
    assert_matches_full_oracle(
        "[id]: /one\n\n# M\n\n[](z [iz])\n",
        "[id]: /one\n\n# M\n\n[](z [id])\n",
        &[TextChange {
            old_range: range(24, 25),
            replacement: Arc::from("d"),
        }],
    );
}

#[test]
fn a_reference_use_before_an_unterminated_second_bracket_still_forces_a_reparse() {
    // This fails if the label scan gives up when a second bracket never
    // closes. `[id][` is no full reference and no collapsed one, so the
    // parser falls back to reading `[id]` as a shortcut use with the stray
    // `[` as text -- the bracket pair the scan had already read whole. The
    // scan broke out of its loop instead of naming it, the guard waved the
    // window through, and the window reparse -- which cannot see the
    // definition in the first section -- published `Text` where a full parse
    // has a `Link`.
    assert_matches_full_oracle(
        "[id]: /one\n\n# M\n\n[][][iz][\n",
        "[id]: /one\n\n# M\n\n[][][id][\n",
        &[TextChange {
            old_range: range(23, 24),
            replacement: Arc::from("d"),
        }],
    );
}

#[test]
fn local_edit_publishes_the_caller_source_and_a_single_normalized_range() {
    // This fails if direct incremental reparse allocates another source or if
    // normalization reports an unrelated shell window.
    let old = "# one\n";
    let new = "# two\n";
    let previous = parse_markdown(
        DocumentRevision::INITIAL,
        text(old),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    let next_source = text(new);
    let update = reparse_markdown(
        &previous,
        DocumentRevision::new(1),
        next_source.clone(),
        &[TextChange {
            old_range: range(2, 5),
            replacement: Arc::from("two"),
        }],
    )
    .unwrap();

    assert!(Arc::ptr_eq(
        update.snapshot.text().shared(),
        next_source.shared()
    ));
    assert_eq!(update.snapshot.tree().write_to_string(), new);
    assert_eq!(update.affected_ranges.len(), 1);
    assert!(matches!(
        update.outcome,
        MarkdownReparseOutcome::Incremental {
            shared_source_independent_green,
            reparsed_range: Some(_),
        } if shared_source_independent_green > 0
    ));
}

#[test]
fn multiline_change_detects_definition_on_an_interior_line() {
    // This fails if detection inspects only the first or last intersecting line.
    let old = "before\n\n[id]: /one\n\nafter\n\nuse [x][id]\n";
    let new = "BEFORE\n\n[id]: /two\n\nAFTER\n\nuse [x][id]\n";
    let previous = parse_markdown(
        DocumentRevision::INITIAL,
        text(old),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    let update = reparse_markdown(
        &previous,
        DocumentRevision::new(1),
        text(new),
        &[TextChange {
            old_range: range(0, old.rfind("\n\nuse").unwrap()),
            replacement: Arc::from("BEFORE\n\n[id]: /two\n\nAFTER"),
        }],
    )
    .unwrap();

    assert_eq!(reference_destinations(&update.snapshot), vec!["/two"]);
    assert!(update.affected_ranges.len() >= 2);
}

#[test]
fn changed_definition_fans_out_only_its_label_and_reuses_unaffected_greens() {
    // This fails if fan-out reparses every reference instead of only changed labels.
    let old = "# stable\n\n[id]: /one\n[other]: /keep\n\nuse [x][id]\n\nkeep [y][other]\n";
    let new = "# stable\n\n[id]: /two\n[other]: /keep\n\nuse [x][id]\n\nkeep [y][other]\n";
    let previous = parse_markdown(
        DocumentRevision::INITIAL,
        text(old),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    let update = reparse_markdown(
        &previous,
        DocumentRevision::new(1),
        text(new),
        &[TextChange {
            old_range: range(old.find("/one").unwrap(), old.find("/one").unwrap() + 4),
            replacement: Arc::from("/two"),
        }],
    )
    .unwrap();
    let unrelated = update
        .snapshot
        .queries()
        .links()
        .find(|link| link.destination.as_ref() == "/keep")
        .unwrap()
        .source_range;

    assert!(update
        .affected_ranges
        .iter()
        .all(|affected| !intersects(*affected, unrelated)));
    assert!(has_shared_source_independent_green(
        &previous,
        &update.snapshot
    ));
}

#[test]
fn a_definition_shaped_line_demoted_to_a_paragraph_still_resolves_its_own_label() {
    // This fails if the window guard trusts a `[label]: dest`-shaped line to
    // really be a definition. Appending `t` gives the last line a tail that is
    // not a valid title, so the line stops being a definition and its `[ie]`
    // becomes a shortcut reference use -- resolved against the definition on
    // line 1, which sits outside the reparse window. Found by fuzzing
    // `syntax_edits`.
    assert_matches_full_oracle(
        "[ie]:/\n#\n[ie]:/ ",
        "[ie]:/\n#\n[ie]:/ t",
        &[TextChange {
            old_range: range(16, 16),
            replacement: Arc::from("t"),
        }],
    );
}

#[test]
fn a_reference_use_in_a_definition_line_tail_is_still_a_reference_use() {
    // This fails if the window guard skips definition-shaped lines wholesale:
    // the trailing `[ie]` is a use whose definition is on line 1, outside the
    // window, whether or not the line as a whole reads as a definition.
    assert_matches_full_oracle(
        "[ie]:/one\n\n# h\n\n[other]:/two ",
        "[ie]:/one\n\n# h\n\n[other]:/two [ie]",
        &[TextChange {
            old_range: range(29, 29),
            replacement: Arc::from("[ie]"),
        }],
    );
}

#[test]
fn a_definition_created_behind_a_block_quote_prefix_still_resolves_its_uses() {
    // This fails if the definition guard decides definition-ness from the raw
    // line start. The edit inserts a line break and two block-quote markers, so
    // `[ie]:/` stops being paragraph text on line 1 and becomes a real
    // definition on the new line 2 -- but that line reads `>>[ie]:/`, which
    // starts with `>`, not `[`. The guard missed it, no oracle parse ran, and
    // the trailing `[ie]` outside the reparse window stayed plain text while a
    // full parse makes it a shortcut reference use. Found by fuzzing
    // `syntax_edits`; see fuzz/seeds/syntax_edits/definition-behind-a-container-prefix.
    assert_matches_full_oracle(
        ">?[ie]:/\n#\n[ie]",
        ">?\r>>[ie]:/\n#\n[ie]",
        &[TextChange {
            old_range: range(2, 2),
            replacement: Arc::from("\r>>"),
        }],
    );
    // The carriage return is incidental -- the container prefix is the whole
    // mechanism, so the same edit with a line feed must hold too.
    assert_matches_full_oracle(
        ">?[ie]:/\n#\n[ie]",
        ">?\n>>[ie]:/\n#\n[ie]",
        &[TextChange {
            old_range: range(2, 2),
            replacement: Arc::from("\n>>"),
        }],
    );
    // A list marker is a container prefix too, and hides a definition just as
    // a block-quote marker does.
    assert_matches_full_oracle(
        "x[a]:/one\n\n[a]",
        "- [a]:/one\n\n[a]",
        &[TextChange {
            old_range: range(0, 1),
            replacement: Arc::from("- "),
        }],
    );
}

#[test]
fn an_edit_to_a_definitions_next_line_destination_refreshes_its_uses() {
    // This fails if the definition guard reads definition-ness off the edited
    // line alone. In each case the definition's destination sits on the line
    // *after* its label, so the edited line -- `xing` -- carries no label, no
    // colon, and nothing else that reads as a definition; only the line above
    // it makes those bytes a destination. The guard saw an ordinary paragraph
    // line, skipped the fan-out, and left `[n][id]` in the first block
    // resolved against the definition the edit had just unmade (or unresolved
    // against the one it had just made).
    //
    // The heading between the two blocks is load-bearing: it splits them into
    // separate shell windows, so the window that reparses the definition does
    // not contain the use.

    // The destination goes away: `[id]: ` alone defines nothing, so the use
    // above must stop being a link.
    assert_matches_full_oracle(
        "[n][id]\n\n# M\n\n[id]: \nxing\n",
        "[n][id]\n\n# M\n\n[id]: \n\ning\n",
        &[TextChange {
            old_range: range(21, 22),
            replacement: Arc::from("\n"),
        }],
    );
    // The destination arrives: `x ing` is a destination followed by `ing`,
    // which is not a valid title, so nothing is defined until the space goes.
    assert_matches_full_oracle(
        "[n][id]\n\n# M\n\n[id]: \nx ing\n",
        "[n][id]\n\n# M\n\n[id]: \nxing\n",
        &[TextChange {
            old_range: range(22, 23),
            replacement: Arc::from(""),
        }],
    );
}

#[test]
fn escaping_a_definition_destination_moves_the_span_its_uses_cached() {
    // This fails if the definition guard decides "same definition" from the
    // *decoded* destination. `[id]: /one` and `[id]:\/one` both decode to
    // `/one`, so a guard comparing decoded values calls them equal and skips
    // the fan-out -- while the authored span the uses cached moved from `6..10`
    // to `5..10`. Every use of `id` then reported a destination span starting
    // one byte inside the escape.
    let old = "[id]: /one\n\nuse [x][id]\n";
    let new = "[id]:\\/one\n\nuse [x][id]\n";
    let previous = parse_markdown(
        DocumentRevision::INITIAL,
        text(old),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    let update = reparse_markdown(
        &previous,
        DocumentRevision::new(1),
        text(new),
        &[TextChange {
            old_range: range(5, 6),
            replacement: Arc::from("\\"),
        }],
    )
    .unwrap();

    let link = update.snapshot.queries().links().next().unwrap();
    assert_eq!(link.destination.as_ref(), "/one");
    assert_eq!(
        link.destination_range,
        Some(range(5, 10)),
        "the cached span must cover the authored `\\/one`, escape included"
    );
    assert_snapshot_matches_full_oracle(&update.snapshot, new);

    // And back the other way: unescaping shortens the authored span without
    // touching the decoded destination.
    let previous = parse_markdown(
        DocumentRevision::INITIAL,
        text(new),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    let update = reparse_markdown(
        &previous,
        DocumentRevision::new(1),
        text(old),
        &[TextChange {
            old_range: range(5, 6),
            replacement: Arc::from(" "),
        }],
    )
    .unwrap();
    assert_eq!(
        update
            .snapshot
            .queries()
            .links()
            .next()
            .unwrap()
            .destination_range,
        Some(range(6, 10))
    );
    assert_snapshot_matches_full_oracle(&update.snapshot, old);
}

#[test]
fn a_delimiter_row_edit_refreshes_the_alignment_its_cells_cached() {
    // This fails if a cell whose own text did not change may be reused across
    // an edit to the row that decides its alignment. A table cell's alignment
    // is written in the delimiter row, not in the cell, so `| a |` spans
    // byte-identical text before and after `| - |` becomes `| :-: |` -- and
    // both the pre-edit green and the pre-edit annotation rode across, leaving
    // the header cell reporting `None` for a centred column.
    let old = "| a |\n| - |\n| b |\n";
    let new = "| a |\n| :-: |\n| b |\n";
    let previous = parse_markdown(
        DocumentRevision::INITIAL,
        text(old),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    let update = reparse_markdown(
        &previous,
        DocumentRevision::new(1),
        text(new),
        &[TextChange {
            old_range: range(8, 9),
            replacement: Arc::from(":-:"),
        }],
    )
    .unwrap();

    let alignments = |snapshot: &waml_syntax::MarkdownSyntaxSnapshot| {
        let queries = snapshot.queries();
        let mut cells: Vec<_> = queries
            .spans(range(0, snapshot.text().shared().len()))
            .filter_map(|span| queries.table_cell(span.owner))
            .map(|cell| (cell.range, cell.alignment))
            .collect();
        cells.sort_by_key(|(range, _)| (range.start(), range.end()));
        cells.dedup();
        cells
    };
    let full = parse_markdown(
        DocumentRevision::new(2),
        text(new),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();

    assert!(
        alignments(&update.snapshot)
            .iter()
            .all(|(_, alignment)| *alignment == waml_syntax::TableAlignment::Center),
        "every cell of a centred column reports Center: {:?}",
        alignments(&update.snapshot)
    );
    assert_eq!(alignments(&update.snapshot), alignments(&full));
    assert_snapshot_matches_full_oracle(&update.snapshot, new);
}

#[test]
fn a_reference_use_opened_by_an_inner_bracket_still_forces_a_reparse() {
    // This fails if the label scan pairs brackets left to right. The parser
    // closes a `]` against the innermost `[` still open, so it reads
    // `use [x[id]` as the text `[x` followed by the shortcut reference
    // `[id]`; a scan that pairs the first `[` with the first `]` sees one
    // label, `x[id`, which no definition matches. The guard then let the edit
    // reparse the second section on its own, and the use never resolved
    // against the definition in the first.
    assert_matches_full_oracle(
        "[id]: /one\n\n# M\n\nuse [x[iz]\n",
        "[id]: /one\n\n# M\n\nuse [x[id]\n",
        &[TextChange {
            old_range: range(25, 26),
            replacement: Arc::from("d"),
        }],
    );
    // The same pairing rule inside a full reference's label: `use [x][[id]`
    // is the text `[x][` followed by the shortcut reference `[id]`.
    assert_matches_full_oracle(
        "[id]: /one\n\n# M\n\nuse [x][[iz]\n",
        "[id]: /one\n\n# M\n\nuse [x][[id]\n",
        &[TextChange {
            old_range: range(27, 28),
            replacement: Arc::from("d"),
        }],
    );
}

#[test]
fn a_reference_label_spelled_across_a_line_break_still_forces_a_reparse() {
    // This fails if the window guard scans its window line by line. A link
    // label is not a line: `[\nid]` is a shortcut reference use of `id`, and
    // per line it is an unclosed `[` above a stray `id]` below -- so the scan
    // named no label at all, the guard waved the window through, and the
    // window reparse resolved `[\nid]` against its own bytes only. The
    // definition sits in the first section, outside the window, so the
    // incremental tree published `Text` where a full parse has a `Link`.
    assert_matches_full_oracle(
        "[id]: /one\n\n# M\n\nuse [x\nid]\n",
        "[id]: /one\n\n# M\n\n[\nid]\n",
        &[TextChange {
            old_range: range(17, 23),
            replacement: Arc::from("["),
        }],
    );
}

#[test]
fn a_definition_label_spelled_across_a_line_break_is_still_a_definition() {
    // The same rule on the definition side. The edit turns the paragraph
    // `[\nid]` into the definition `[\nid]: ]`, whose label spans the line
    // break, so `[a][id]` in the second section becomes a link to `]`. The
    // definition guard walked the edited paragraph run line by line, saw an
    // unclosed `[` above a label-less `id]: ]` below, ran no oracle parse, and
    // left `[a][id]` plain text.
    assert_matches_full_oracle(
        "[\nid]\n\n# M\n\n[a][id]\n",
        "[\nid]: ]\n\n# M\n\n[a][id]\n",
        &[TextChange {
            old_range: range(4, 4),
            replacement: Arc::from("]: "),
        }],
    );
}

#[test]
fn unmaking_a_repeated_definition_reparses_the_repeat() {
    // This fails if a definition that repeats a label tokenises differently
    // from the first of its label. A definition may put its destination on the
    // line below, so `[id]: \nx` is a definition with destination `x` -- and it
    // has to stay one when `id` was already defined above, because CommonMark
    // parses the repeat exactly like the first and only declines to let it win
    // the label.
    //
    // The edit renames the first definition from `id` to ` d`, which unmakes
    // the repeat. The second definition is in another shell window and is
    // carried over untouched, so it only matches a full parse if the two
    // readings were the same all along.
    assert_matches_full_oracle(
        "[id]: /a\n\n# M\n\n[id]: \nx\n",
        "[ d]: /a\n\n# M\n\n[id]: \nx\n",
        &[TextChange {
            old_range: range(1, 2),
            replacement: Arc::from(" "),
        }],
    );
}

#[test]
fn renamed_definition_invalidates_old_backlinks() {
    // This fails if fan-out consults only new backlinks after a label disappears.
    let old = "[id]: /one\n\nuse [x][id]\n";
    let new = "[new]: /one\n\nuse [x][id]\n";
    let previous = parse_markdown(
        DocumentRevision::INITIAL,
        text(old),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    let old_use = previous.queries().links().next().unwrap().source_range;
    let update = reparse_markdown(
        &previous,
        DocumentRevision::new(1),
        text(new),
        &[TextChange {
            old_range: range(1, 3),
            replacement: Arc::from("new"),
        }],
    )
    .unwrap();

    assert!(reference_destinations(&update.snapshot).is_empty());
    assert_snapshot_matches_full_oracle(&update.snapshot, new);
    assert!(update
        .affected_ranges
        .iter()
        .any(|affected| intersects(*affected, old_use)));
}

#[test]
fn block_merging_edit_restamps_reused_link_owners() {
    // This fails if incremental reuse keeps a link's owner annotation pointing
    // at the block the link used to live in. Deleting the last line's tail
    // (`z\n`) lets the blank line behind it terminate the first paragraph
    // instead of separating two, so both reference uses end up in one
    // paragraph. Each reused link still named its own dead paragraph, and
    // `reference_backlinks` reported two owners where a full parse reports one.
    let old = "# M\n\n[id]: /one\n\na [x][id]\nzz\n\nb [y][id]\n";
    let new = "# M\n\n[id]: /one\n\na [x][id]\nz\nb [y][id]\n";
    let cut = old.find("zz").unwrap() + 1;
    let previous = parse_markdown(
        DocumentRevision::INITIAL,
        text(old),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    let update = reparse_markdown(
        &previous,
        DocumentRevision::new(1),
        text(new),
        &[TextChange {
            old_range: range(cut, cut + 2),
            replacement: Arc::from(""),
        }],
    )
    .unwrap();

    assert_snapshot_matches_full_oracle(&update.snapshot, new);
    let paragraph = update
        .snapshot
        .tree()
        .root()
        .children()
        .filter_map(|child| match child {
            SyntaxElement::Node(node)
                if node.kind() == waml_syntax::OkfMarkdownSyntaxKind::Paragraph =>
            {
                Some(node)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        paragraph.len(),
        1,
        "the edit merges both uses into one block"
    );
    let owner = waml_syntax::syntax_identity(&paragraph[0]).unwrap();
    assert_eq!(
        update.snapshot.queries().reference_backlinks("id").as_ref(),
        &[owner],
        "every reused link names its surviving block"
    );
    assert_eq!(
        update
            .snapshot
            .queries()
            .links()
            .map(|link| link.identity)
            .collect::<Vec<_>>(),
        vec![owner, owner]
    );
}

#[test]
fn definition_change_does_not_mask_named_bridge_fallback() {
    // This fails if reference handling replaces a bridge Full outcome.
    let old = "---\n---\n\n[id]: /one\n\nuse [x][id]\n";
    let new = "----\n---\n\n[id]: /two\n\nuse [x][id]\n";
    let previous = parse_markdown(
        DocumentRevision::INITIAL,
        text(old),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    let update = reparse_markdown(
        &previous,
        DocumentRevision::new(1),
        text(new),
        &[
            TextChange {
                old_range: range(3, 3),
                replacement: Arc::from("-"),
            },
            TextChange {
                old_range: range(old.find("/one").unwrap(), old.find("/one").unwrap() + 4),
                replacement: Arc::from("/two"),
            },
        ],
    )
    .unwrap();

    assert_eq!(
        update.outcome,
        MarkdownReparseOutcome::Full {
            reason: FullReparseReason::FrontmatterBoundaryChanged,
        }
    );
    assert_eq!(update.affected_ranges.as_ref(), &[range(0, new.len())]);
}

#[test]
fn definition_edit_preserves_an_unchanged_island_identity() {
    // This fails if a definition update replaces the entire tree and regenerates
    // identities for an island outside the affected roots.
    let old = "[id]: /one\n\nuse [x][id]\n\n# Model\n## Attributes\nname: String\n";
    let new = "[id]: /two\n\nuse [x][id]\n\n# Model\n## Attributes\nname: String\n";
    let previous = parse_markdown(
        DocumentRevision::INITIAL,
        text(old),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    assert_eq!(previous.structure().islands.len(), 1);
    let owner = previous.structure().islands[0].owner;
    let update = reparse_markdown(
        &previous,
        DocumentRevision::new(1),
        text(new),
        &[TextChange {
            old_range: range(old.find("/one").unwrap(), old.find("/one").unwrap() + 4),
            replacement: Arc::from("/two"),
        }],
    )
    .unwrap();

    assert!(
        matches!(update.outcome, MarkdownReparseOutcome::Incremental { .. }),
        "island edit outcome: {:?}",
        update.outcome,
    );
    assert_eq!(
        update.snapshot.structure().islands[0].owner,
        owner,
        "old owner node: {:?}; new island owner node: {:?}",
        node_for_identity(previous.tree().root(), owner).map(|node| (node.kind(), node.range())),
        node_for_identity(
            update.snapshot.tree().root(),
            update.snapshot.structure().islands[0].owner,
        )
        .map(|node| (node.kind(), node.range())),
    );
}

#[test]
fn deterministic_incremental_cases_match_the_full_parse_oracle() {
    let cases = [
        ("local paragraph", "alpha\n", "alpHa\n", 3, 4, "H"),
        ("list marker", "- one\n", "* one\n", 0, 1, "*"),
        ("heading boundary", "# one\n", "## one\n", 1, 1, "#"),
        (
            "fence boundary",
            "```\na\n```\n",
            "~~~~\na\n~~~~\n",
            0,
            10,
            "~~~~\na\n~~~~\n",
        ),
        (
            "table delimiter",
            "| a |\n| - |\n| b |\n",
            "| a |\n| :-: |\n| b |\n",
            8,
            9,
            ":-:",
        ),
        (
            "frontmatter fence",
            "---\ntype: uml.Class\n---\n# A\n",
            "----\ntype: uml.Class\n---\n# A\n",
            3,
            3,
            "-",
        ),
        (
            "WAML section boundary",
            "# Model\n## Attributes\none\n",
            "# Model\n## Values\none\n",
            11,
            21,
            "Values",
        ),
        ("Unicode replacement", "café\n", "cafø\n", 3, 5, "ø"),
        (
            "inline link before reference use",
            "[a](x) [b][id]\n",
            "[a](y) [b][id]\n",
            4,
            5,
            "y",
        ),
        ("EOF insertion", "tail\n", "tail\nmore\n", 5, 5, "more\n"),
    ];
    for (name, old, new, start, end, replacement) in cases {
        let changes = [TextChange {
            old_range: range(start, end),
            replacement: Arc::from(replacement),
        }];
        let result = std::panic::catch_unwind(|| assert_matches_full_oracle(old, new, &changes));
        assert!(result.is_ok(), "oracle case failed: {name}");
    }
}

#[test]
fn boundary_edits_report_named_fallbacks() {
    for (old, new, change, reason) in [
        (
            "# one\n",
            "## one\n",
            TextChange {
                old_range: range(1, 1),
                replacement: Arc::from("#"),
            },
            FullReparseReason::HeadingBoundaryChanged,
        ),
        (
            "---\ntype: uml.Class\n---\n",
            "----\ntype: uml.Class\n---\n",
            TextChange {
                old_range: range(3, 3),
                replacement: Arc::from("-"),
            },
            FullReparseReason::FrontmatterBoundaryChanged,
        ),
        (
            "# Model\n## Attributes\none\n",
            "# Model\n## Values\none\n",
            TextChange {
                old_range: range(11, 21),
                replacement: Arc::from("Values"),
            },
            FullReparseReason::IslandBoundaryChanged,
        ),
    ] {
        let previous = parse_markdown(
            DocumentRevision::INITIAL,
            text(old),
            MarkdownDialect::WAML_DEFAULT,
        )
        .unwrap();
        let update =
            reparse_markdown(&previous, DocumentRevision::new(1), text(new), &[change]).unwrap();
        assert_eq!(update.outcome, MarkdownReparseOutcome::Full { reason });
        assert_eq!(update.affected_ranges.as_ref(), &[range(0, new.len())]);
    }
}

#[test]
fn overlapping_changes_use_the_named_full_fallback() {
    let source = "abcdef\n";
    let previous = parse_markdown(
        DocumentRevision::INITIAL,
        text(source),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    let update = reparse_markdown(
        &previous,
        DocumentRevision::new(1),
        text("aXYf\n"),
        &[
            TextChange {
                old_range: range(1, 4),
                replacement: Arc::from("X"),
            },
            TextChange {
                old_range: range(3, 5),
                replacement: Arc::from("Y"),
            },
        ],
    )
    .unwrap();

    assert_eq!(
        update.outcome,
        MarkdownReparseOutcome::Full {
            reason: FullReparseReason::OverlappingChanges,
        }
    );
    assert_eq!(update.affected_ranges.as_ref(), &[range(0, 5)]);
    assert_snapshot_matches_full_oracle(&update.snapshot, "aXYf\n");
}

#[test]
fn new_text_change_mismatch_is_a_hard_error() {
    let previous = parse_markdown(
        DocumentRevision::INITIAL,
        text("old\n"),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    let error = reparse_markdown(
        &previous,
        DocumentRevision::new(1),
        text("new\n"),
        &[TextChange {
            old_range: range(0, 3),
            replacement: Arc::from("other"),
        }],
    )
    .err()
    .expect("mismatched changes must fail");

    match error {
        waml_syntax::ParseError::StructuralInvariant { reason } => assert_eq!(
            reason.as_ref(),
            "incremental changes do not reconstruct candidate source"
        ),
        other => panic!("unexpected mismatch error: {other:?}"),
    }
    assert_eq!(previous.text().shared().as_str(), "old\n");
    assert_snapshot_matches_full_oracle(
        &parse_markdown(
            DocumentRevision::new(2),
            text("new\n"),
            MarkdownDialect::WAML_DEFAULT,
        )
        .unwrap(),
        "new\n",
    );
}

#[test]
fn published_query_and_structure_owners_resolve_in_the_published_tree() {
    let old = "[id]: /old\n\nuse [x][id]\n\n# Model\n## Attributes\nname: String\n";
    let new = "[id]: /dest\n\nuse [x][id]\n\n# Model\n## Attributes\nname: String\n";
    let previous = parse_markdown(
        DocumentRevision::INITIAL,
        text(old),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    let new_text = text(new);
    let update = reparse_markdown(
        &previous,
        DocumentRevision::new(1),
        new_text.clone(),
        &[TextChange {
            old_range: range(old.find("/old").unwrap(), old.find("/old").unwrap() + 4),
            replacement: Arc::from("/dest"),
        }],
    )
    .unwrap();
    let snapshot = update.snapshot;

    assert!(Arc::ptr_eq(snapshot.text().shared(), new_text.shared()));
    assert_source_backed_green_uses(
        &GreenElement::Node(snapshot.tree().root_green().clone()),
        new_text.shared(),
    );
    for link in snapshot.queries().links() {
        assert!(node_for_identity(snapshot.tree().root(), link.owner).is_some());
        assert!(node_for_identity(snapshot.tree().root(), link.identity).is_some());
    }
    for image in snapshot.queries().images() {
        assert!(node_for_identity(snapshot.tree().root(), image.owner).is_some());
    }
    for entity in snapshot.queries().entities() {
        assert!(node_for_identity(snapshot.tree().root(), entity.identity).is_some());
    }
    for span in snapshot.queries().spans(range(0, new.len())) {
        assert!(node_for_identity(snapshot.tree().root(), span.owner).is_some());
    }
    for island in snapshot.structure().islands.iter() {
        assert!(node_for_identity(snapshot.tree().root(), island.owner).is_some());
        assert!(snapshot.queries().island(island.owner).is_some());
    }
}
