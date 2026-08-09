//! Query parser: whitespace-split chunks, where a `name:value` chunk with a
//! KNOWN filter name (`kind`, `in`) becomes a [`QueryFilter`] and every other
//! chunk falls through the tokenizer as plain [`QueryTerm`]s. Unknown filter
//! names are never an error — they degrade to terms.

use super::tokenize::tokenize;

/// A structured restriction parsed out of the query text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QueryFilter {
    /// `kind:actor` — restrict to elements/documents of this UML kind.
    Kind(String),
    /// `in:docs/guides/` — restrict to documents whose path starts with this.
    InPath(String),
}

/// A single lowercased search term.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueryTerm {
    pub text: String,
}

/// A parsed query: the ANDed terms plus any structured filters.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ParsedQuery {
    pub terms: Vec<QueryTerm>,
    pub filters: Vec<QueryFilter>,
}

impl ParsedQuery {
    pub fn is_empty(&self) -> bool {
        self.terms.is_empty() && self.filters.is_empty()
    }
}

/// Parses `input` into terms and filters. See module docs for the grammar.
pub fn parse_query(input: &str) -> ParsedQuery {
    let mut terms = Vec::new();
    let mut filters = Vec::new();

    for chunk in input.split_whitespace() {
        if let Some(value) = chunk.strip_prefix("kind:") {
            if !value.is_empty() {
                filters.push(QueryFilter::Kind(value.to_lowercase()));
                continue;
            }
        }
        if let Some(value) = chunk.strip_prefix("in:") {
            if !value.is_empty() {
                filters.push(QueryFilter::InPath(value.to_string()));
                continue;
            }
        }
        for token in tokenize(chunk) {
            terms.push(QueryTerm { text: token.text });
        }
    }

    ParsedQuery { terms, filters }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_terms_are_anded_terms() {
        let q = parse_query("payment capture");
        assert_eq!(q.terms.len(), 2);
        assert!(q.filters.is_empty());
    }

    #[test]
    fn known_filters_parse_and_unknown_filters_degrade_to_terms() {
        let q = parse_query("kind:actor in:docs/guides/ payment sev:high");
        assert_eq!(
            q.filters,
            vec![
                QueryFilter::Kind("actor".into()),
                QueryFilter::InPath("docs/guides/".into()),
            ]
        );
        let texts: Vec<&str> = q.terms.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, vec!["payment", "sev", "high"]);
    }

    #[test]
    fn empty_and_whitespace_queries_are_empty() {
        assert!(parse_query("").is_empty());
        assert!(parse_query("   ").is_empty());
    }
}
