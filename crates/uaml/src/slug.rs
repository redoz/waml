use std::sync::LazyLock;
use regex::Regex;

static CAMEL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"([a-z0-9])([A-Z])").unwrap());
static ACRONYM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"([A-Z]+)([A-Z][a-z])").unwrap());
static NON_ALNUM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^a-z0-9]+").unwrap());
static EDGE_DASH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^-+|-+$").unwrap());

/// Slugify a title. Mirrors packages/okf/src/slug.ts.
pub fn slugify(text: &str, fallback: &str) -> String {
    let s = CAMEL.replace_all(text, "$1-$2");
    let s = ACRONYM.replace_all(&s, "$1-$2");
    let s = s.to_lowercase();
    let s = NON_ALNUM.replace_all(&s, "-");
    let s = EDGE_DASH.replace_all(&s, "").into_owned();
    if s.is_empty() { fallback.to_string() } else { s }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lowercases_and_hyphenates_spaces() {
        assert_eq!(slugify("Order Line", ""), "order-line");
    }

    #[test]
    fn splits_camel_case_boundary() {
        assert_eq!(slugify("OrderStatus", ""), "order-status");
    }

    #[test]
    fn splits_acronym_boundary() {
        assert_eq!(slugify("HTTPServer", ""), "http-server");
    }

    #[test]
    fn trims_and_collapses_punctuation() {
        assert_eq!(slugify("  Order__Line!! ", ""), "order-line");
    }

    #[test]
    fn empty_uses_fallback() {
        assert_eq!(slugify("---", "doc"), "doc");
    }
}
