use std::sync::LazyLock;
use regex::Regex;

static MULT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:[1-9]\d*|\*|(?:0|[1-9]\d*)\.\.(?:[1-9]\d*|\*))$").unwrap()
});
static RANGE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\d+)\.\.(\d+)$").unwrap());

/// A UML multiplicity, validated against the BNF and stored in canonical string form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Multiplicity(String);

impl Multiplicity {
    pub fn parse(s: &str) -> Option<Multiplicity> {
        if !MULT_RE.is_match(s) {
            return None;
        }
        if let Some(c) = RANGE_RE.captures(s) {
            let lo: u64 = c[1].parse().ok()?;
            let hi: u64 = c[2].parse().ok()?;
            if lo > hi {
                return None;
            }
        }
        Some(Multiplicity(s.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for Multiplicity {
    fn default() -> Self {
        Multiplicity("1".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_forms() {
        for s in ["1", "0..1", "*", "1..*", "2..5", "10"] {
            assert!(Multiplicity::parse(s).is_some(), "{s} should be valid");
        }
    }

    #[test]
    fn rejects_invalid_forms() {
        for s in ["0", "", "1..", "..5", "-1", "1..2..3", "a", "5..2"] {
            assert!(Multiplicity::parse(s).is_none(), "{s} should be invalid");
        }
    }

    #[test]
    fn round_trips_the_source_string() {
        assert_eq!(Multiplicity::parse("1..*").unwrap().as_str(), "1..*");
    }

    #[test]
    fn default_is_one() {
        assert_eq!(Multiplicity::default().as_str(), "1");
    }
}
