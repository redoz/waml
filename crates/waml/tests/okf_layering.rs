//! The okf substrate's layering rule, enforced instead of asserted.
//!
//! okf.rs's header promises that a later `okf-core` crate split stays
//! mechanical. That promise is only worth anything if something checks it —
//! the previous violation (`Index` embedding a view type, `resolved_view`
//! taking the middleware registry) was invisible precisely because the rule
//! lived in a comment.

use std::fs;
use std::path::Path;

/// Import paths that would make an `okf-core` split non-mechanical.
const FORBIDDEN: [&str; 3] = ["crate::view", "crate::uml", "crate::model"];

fn production_source(path: &Path) -> String {
    let text = fs::read_to_string(path).expect("okf source is readable");
    // Tests may reach up into the tiers above: they exercise the substrate
    // *through* them. Only the shipped code is bound by the layering rule.
    match text.find("\n#[cfg(test)]\n") {
        Some(at) => text[..at].to_string(),
        None => text,
    }
}

#[test]
fn the_okf_substrate_does_not_import_the_tiers_above_it() {
    let mut sources = vec![Path::new("src/okf.rs").to_path_buf()];
    for entry in fs::read_dir("src/okf").expect("src/okf exists") {
        let path = entry.expect("readable entry").path();
        if path.extension().is_some_and(|e| e == "rs") {
            sources.push(path);
        }
    }
    assert!(sources.len() > 1, "expected okf submodules to be found");

    let mut violations = Vec::new();
    for path in sources {
        let source = production_source(&path);
        for (number, line) in source.lines().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            for forbidden in FORBIDDEN {
                if code.contains(forbidden) {
                    violations.push(format!(
                        "{}:{}: {}",
                        path.display(),
                        number + 1,
                        line.trim()
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "okf must not depend on the UML or view tiers — a crate split would not be mechanical:\n{}",
        violations.join("\n")
    );
}
