//! Word tokenizer: splits on anything that is not alphanumeric or `_`,
//! lowercases each run, and records byte offsets into the source text.

/// A single lowercased word with its byte span in the original text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    pub text: String,
    pub start: u32,
    pub end: u32,
}

/// Splits `text` into lowercased word tokens on runs of
/// `char::is_alphanumeric() || c == '_'`. Byte offsets are into `text`.
pub fn tokenize(text: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut run_start: Option<usize> = None;

    for (idx, ch) in text.char_indices() {
        let is_word_char = ch.is_alphanumeric() || ch == '_';
        match (is_word_char, run_start) {
            (true, None) => run_start = Some(idx),
            (false, Some(start)) => {
                tokens.push(make_token(text, start, idx));
                run_start = None;
            }
            _ => {}
        }
    }
    if let Some(start) = run_start {
        tokens.push(make_token(text, start, text.len()));
    }

    tokens
}

fn make_token(text: &str, start: usize, end: usize) -> Token {
    Token {
        text: text[start..end].to_lowercase(),
        start: start as u32,
        end: end as u32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_lowercase_with_byte_offsets() {
        let tokens = tokenize("Payment is Captured");
        assert_eq!(
            tokens
                .iter()
                .map(|t| (t.text.as_str(), t.start, t.end))
                .collect::<Vec<_>>(),
            vec![("payment", 0, 7), ("is", 8, 10), ("captured", 11, 19)]
        );
    }

    #[test]
    fn punctuation_splits_and_identifiers_survive() {
        let texts: Vec<String> = tokenize("id: payment-capture (v2)")
            .into_iter()
            .map(|t| t.text)
            .collect();
        assert_eq!(texts, vec!["id", "payment", "capture", "v2"]);
    }

    #[test]
    fn non_ascii_words_are_kept_whole() {
        let texts: Vec<String> = tokenize("Zahlung erfasst")
            .into_iter()
            .map(|t| t.text)
            .collect();
        assert_eq!(texts, vec!["zahlung", "erfasst"]);
    }
}
