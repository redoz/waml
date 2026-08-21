//! The `[title](href)` link-ref parser and slug resolution shared by
//! projection, behavior lowering and `sequence::lower`.

use std::collections::BTreeSet;

pub(crate) fn resolve_slug(path: &str, slug: &str, claimed: &BTreeSet<&str>) -> Option<String> {
    let target = crate::okf::resolve_href(path, slug);
    claimed.contains(target.as_str()).then_some(target)
}

pub(crate) fn resolve_describes(
    okf: &crate::okf::Concept,
    path: &str,
    claimed: &BTreeSet<&str>,
) -> Option<String> {
    okf.extra
        .get_str("describes")
        .and_then(parse_link_ref)
        .and_then(|link| resolve_slug(path, &link.slug, claimed))
}

/// The one `describes`/link-ref parser. Both `sequence::lower` and this
/// module's own `resolve_describes` call through here — do not hand-split
/// the `[title](href)` syntax again.
pub(crate) fn parse_link_ref(value: &str) -> Option<crate::layout::LinkRef> {
    let value = value.trim();
    let title_end = value.find("](")?;
    let title = value.strip_prefix('[')?.get(..title_end - 1)?;
    let href = value.get(title_end + 2..)?.strip_suffix(')')?;
    Some(crate::layout::LinkRef {
        title: title.to_owned(),
        slug: href.to_owned(),
    })
}

pub(crate) fn normalize_slot_value(value: &str) -> String {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(value)
        .to_owned()
}

pub(crate) fn parse_link_in_text(value: &str) -> Option<crate::layout::LinkRef> {
    let start = value.find('[')?;
    let relative_end = value[start..].find(')')?;
    parse_link_ref(&value[start..=start + relative_end])
}

#[cfg(test)]
mod link_ref_tests {
    use super::parse_link_ref;

    // Task 3: `sequence::lower` and `resolve_describes` both go through
    // `parse_link_ref` now — this table asserts the single parser's
    // tolerance for whitespace and malformed input.
    #[test]
    fn parses_well_formed_link_ref() {
        let link = parse_link_ref("[t](x)").expect("well-formed link parses");
        assert_eq!(link.title, "t");
        assert_eq!(link.slug, "x");
    }

    #[test]
    fn trims_surrounding_whitespace() {
        let link = parse_link_ref(" [t](x) ").expect("surrounding whitespace is trimmed");
        assert_eq!(link.title, "t");
        assert_eq!(link.slug, "x");
    }

    #[test]
    fn rejects_missing_open_bracket() {
        assert!(parse_link_ref("t](x)").is_none());
    }

    #[test]
    fn rejects_missing_close_paren() {
        assert!(parse_link_ref("[t](x").is_none());
    }
}
