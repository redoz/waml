use serde::Serialize;
use waml::diagnostic::{Diagnostic, Severity};

#[derive(Serialize)]
struct DiagDto<'a> {
    severity: &'a str,
    code: &'a str,
    message: &'a str,
    file: &'a str,
    line: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    span: Option<(usize, usize)>,
}

fn severity_str(s: Severity) -> &'static str {
    match s {
        Severity::Error => "error",
        Severity::Warning => "warning",
    }
}

fn sorted(diags: &[Diagnostic]) -> Vec<&Diagnostic> {
    let mut v: Vec<&Diagnostic> = diags.iter().collect();
    v.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));
    v
}

pub fn render_human(diags: &[Diagnostic]) -> String {
    if diags.is_empty() {
        return "No problems found.".to_string();
    }
    let mut lines = Vec::new();
    for d in sorted(diags) {
        // `span` is 0-based byte columns within the line; print 1-based
        // columns to match the 1-based `line`.
        let location = match d.span {
            Some((start, end)) => format!("{}:{}:{}-{}", d.file, d.line, start + 1, end + 1),
            None => format!("{}:{}", d.file, d.line),
        };
        lines.push(format!(
            "{location}: {}[{}]: {}",
            severity_str(d.severity),
            d.code.as_str(),
            d.message
        ));
    }
    let errors = diags
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .count();
    let warnings = diags.len() - errors;
    lines.push(format!("\n{errors} error(s), {warnings} warning(s)."));
    lines.join("\n")
}

pub fn render_json(diags: &[Diagnostic]) -> String {
    let dtos: Vec<DiagDto> = sorted(diags)
        .into_iter()
        .map(|d| DiagDto {
            severity: severity_str(d.severity),
            code: d.code.as_str(),
            message: &d.message,
            file: &d.file,
            line: d.line,
            span: d.span,
        })
        .collect();
    serde_json::to_string_pretty(&dtos).unwrap_or_else(|_| "[]".to_string())
}

fn diff_lines(a: &str, b: &str) -> String {
    let al: Vec<&str> = a.lines().collect();
    let bl: Vec<&str> = b.lines().collect();
    let mut s = 0;
    while s < al.len() && s < bl.len() && al[s] == bl[s] {
        s += 1;
    }
    let (mut ea, mut eb) = (al.len(), bl.len());
    while ea > s && eb > s && al[ea - 1] == bl[eb - 1] {
        ea -= 1;
        eb -= 1;
    }
    let mut out = String::new();
    for l in &al[s..ea] {
        out.push_str(&format!("-{l}\n"));
    }
    for l in &bl[s..eb] {
        out.push_str(&format!("+{l}\n"));
    }
    out
}

/// Render a human-readable summary of changes between an old and new bundle:
/// `~ path` (changed, with unified-ish added/removed lines), `+ path (new)`,
/// `- path (deleted)`.
pub fn render_diff(old: &[(String, String)], new: &[(String, String)]) -> String {
    use std::collections::BTreeMap;
    let om: BTreeMap<&str, &str> = old.iter().map(|(p, c)| (p.as_str(), c.as_str())).collect();
    let nm: BTreeMap<&str, &str> = new.iter().map(|(p, c)| (p.as_str(), c.as_str())).collect();
    let mut out = String::new();
    for (p, c) in &nm {
        match om.get(p) {
            Some(old_c) if old_c == c => {}
            Some(old_c) => {
                out.push_str(&format!("~ {p}\n"));
                out.push_str(&diff_lines(old_c, c));
            }
            None => {
                out.push_str(&format!("+ {p} (new)\n"));
                out.push_str(&diff_lines("", c));
            }
        }
    }
    for p in om.keys() {
        if !nm.contains_key(p) {
            out.push_str(&format!("- {p} (deleted)\n"));
        }
    }
    if out.is_empty() {
        out.push_str("no changes\n");
    }
    out
}

pub fn check_exit_code(diags: &[Diagnostic]) -> i32 {
    if diags.iter().any(|d| d.severity == Severity::Error) {
        1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use waml::diagnostic::DiagCode;

    fn sample() -> Vec<Diagnostic> {
        vec![
            Diagnostic::new(
                DiagCode::UnresolvedTarget,
                "no doc './ghost.md'",
                "a/order.md",
                8,
            ),
            Diagnostic::warn(
                DiagCode::UnknownType,
                "unknown type 'bpmn.Task'",
                "a/x.md",
                2,
            ),
        ]
    }

    #[test]
    fn human_output_has_file_line_and_code() {
        let out = render_human(&sample());
        assert!(out.contains("a/order.md:8: error[unresolved-target]: no doc './ghost.md'"));
        assert!(out.contains("a/x.md:2: warning[unknown-type]:"));
    }

    #[test]
    fn human_output_includes_the_column_span_when_present() {
        let diags = vec![
            Diagnostic::new(DiagCode::MalformedAttribute, "bad", "a.md", 8).with_span((2, 20)),
        ];
        let out = render_human(&diags);
        assert!(
            out.contains("a.md:8:3-21: error[malformed-attribute]: bad"),
            "{out}"
        );
    }

    #[test]
    fn json_output_is_an_array_of_diagnostics() {
        let out = render_json(&sample());
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v.as_array().unwrap().len(), 2);
        assert_eq!(v[0]["code"], "unresolved-target");
        assert_eq!(v[0]["line"], 8);
    }

    #[test]
    fn json_output_includes_span_when_present() {
        let diags = vec![
            Diagnostic::new(DiagCode::MalformedAttribute, "bad", "a.md", 8).with_span((2, 20)),
        ];
        let out = render_json(&diags);
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v[0]["span"][0], 2);
        assert_eq!(v[0]["span"][1], 20);
    }

    #[test]
    fn exit_code_is_one_with_errors_zero_with_only_warnings() {
        assert_eq!(check_exit_code(&sample()), 1);
        let only_warn = vec![Diagnostic::warn(DiagCode::UnknownType, "w", "a.md", 1)];
        assert_eq!(check_exit_code(&only_warn), 0);
        assert_eq!(check_exit_code(&[]), 0);
    }

    #[test]
    fn render_diff_shows_added_changed_deleted() {
        let old = vec![
            ("a.md".to_string(), "x\ny\n".to_string()),
            ("gone.md".to_string(), "z\n".to_string()),
        ];
        let new = vec![
            ("a.md".to_string(), "x\nY\n".to_string()),
            ("new.md".to_string(), "q\n".to_string()),
        ];
        let d = render_diff(&old, &new);
        assert!(d.contains("a.md"));
        assert!(d.contains("-y"));
        assert!(d.contains("+Y"));
        assert!(d.contains("new.md")); // added
        assert!(d.contains("gone.md")); // deleted
    }
}
