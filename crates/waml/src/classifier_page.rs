//! A pure markdown page for one classifier: prose identity, a definition list
//! of properties, and every relationship written as a directional sentence.
//!
//! Model in, markdown out. No editor dependency — a CLI subcommand can emit
//! the identical page.

use crate::multiplicity::Multiplicity;

/// Cardinal numbers spelled out through ten; above ten prose reads worse than
/// digits, so digits win.
// Wired into the association sentences in Task 4, via `spell_multiplicity`.
#[allow(dead_code)]
fn number_word(n: u64) -> String {
    const WORDS: [&str; 11] = [
        "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
    ];
    match WORDS.get(n as usize) {
        Some(word) => (*word).to_string(),
        None => n.to_string(),
    }
}

/// A UML multiplicity as English. `None` when `raw` is not a multiplicity this
/// crate can parse — the caller then omits the count entirely rather than
/// printing notation into a sentence.
// Wired into the association sentences in Task 4.
#[allow(dead_code)]
fn spell_multiplicity(raw: &str) -> Option<String> {
    let parsed = Multiplicity::parse(raw)?;
    let raw = parsed.as_str();
    if raw == "*" {
        return Some("zero or more".to_string());
    }
    let Some((lo, hi)) = raw.split_once("..") else {
        // An exact count. `1` is the ordinary case and reads as a plain
        // article; anything else is worth calling out.
        let n: u64 = raw.parse().ok()?;
        return Some(if n == 1 {
            "one".to_string()
        } else {
            format!("exactly {}", number_word(n))
        });
    };
    let lo: u64 = lo.parse().ok()?;
    if hi == "*" {
        return Some(match lo {
            0 => "zero or more".to_string(),
            1 => "one or more".to_string(),
            lo => format!("{} or more", number_word(lo)),
        });
    }
    let hi: u64 = hi.parse().ok()?;
    if lo == 0 && hi == 1 {
        return Some("zero or one".to_string());
    }
    Some(format!("{} to {}", number_word(lo), number_word(hi)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_row_of_the_prose_table_spells_out() {
        let cases = [
            ("1", "one"),
            ("0..1", "zero or one"),
            ("1..*", "one or more"),
            ("0..*", "zero or more"),
            ("*", "zero or more"),
            ("3", "exactly three"),
            ("2..5", "two to five"),
            ("2..*", "two or more"),
        ];
        for (raw, expected) in cases {
            assert_eq!(
                spell_multiplicity(raw).as_deref(),
                Some(expected),
                "multiplicity {raw}"
            );
        }
    }

    #[test]
    fn numbers_spell_out_through_ten_and_show_digits_above_it() {
        assert_eq!(number_word(0), "zero");
        assert_eq!(number_word(10), "ten");
        assert_eq!(number_word(11), "11");
        // Both boundaries, as read through the speller itself.
        assert_eq!(spell_multiplicity("10").as_deref(), Some("exactly ten"));
        assert_eq!(spell_multiplicity("11").as_deref(), Some("exactly 11"));
    }

    #[test]
    fn an_unparseable_multiplicity_spells_nothing() {
        // `Multiplicity::parse` rejects each of these, so the sentence must
        // omit the count rather than invent one.
        for raw in ["", "0", "many", "1..", "5..2", "-1"] {
            assert_eq!(spell_multiplicity(raw), None, "multiplicity {raw:?}");
        }
    }
}
