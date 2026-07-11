use serde::Serialize;
use uaml::diagnostic::{Diagnostic, Severity};
use uaml::parse::parse_document;
use uaml::serialize::serialize_document;
use uaml::validate::validate;

#[derive(Serialize)]
struct DiagDto<'a> {
    severity: &'a str,
    code: &'a str,
    message: &'a str,
    file: &'a str,
    line: usize,
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
        lines.push(format!(
            "{}:{}: {}[{}]: {}",
            d.file,
            d.line,
            severity_str(d.severity),
            d.code.as_str(),
            d.message
        ));
    }
    let errors = diags.iter().filter(|d| d.severity == Severity::Error).count();
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
        })
        .collect();
    serde_json::to_string_pretty(&dtos).unwrap_or_else(|_| "[]".to_string())
}

pub fn check_exit_code(diags: &[Diagnostic]) -> i32 {
    if diags.iter().any(|d| d.severity == Severity::Error) {
        1
    } else {
        0
    }
}

pub struct FmtResult {
    pub path: String,
    pub formatted: String,
    pub changed: bool,
    pub skipped: bool,
}

pub fn plan_fmt(files: &[(String, String)]) -> Vec<FmtResult> {
    let diags = validate(files);
    let mut out = Vec::new();
    for (path, text) in files {
        let has_error = diags
            .iter()
            .any(|d| d.file == *path && d.severity == Severity::Error);
        if has_error {
            out.push(FmtResult { path: path.clone(), formatted: text.clone(), changed: false, skipped: true });
            continue;
        }
        let formatted = serialize_document(&parse_document(text));
        let changed = formatted != *text;
        out.push(FmtResult { path: path.clone(), formatted, changed, skipped: false });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use uaml::diagnostic::DiagCode;

    fn sample() -> Vec<Diagnostic> {
        vec![
            Diagnostic::new(DiagCode::UnresolvedTarget, "no doc './ghost.md'", "a/order.md", 8),
            Diagnostic::warn(DiagCode::UnknownType, "unknown type 'bpmn.Task'", "a/x.md", 2),
        ]
    }

    #[test]
    fn human_output_has_file_line_and_code() {
        let out = render_human(&sample());
        assert!(out.contains("a/order.md:8: error[unresolved-target]: no doc './ghost.md'"));
        assert!(out.contains("a/x.md:2: warning[unknown-type]:"));
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
    fn exit_code_is_one_with_errors_zero_with_only_warnings() {
        assert_eq!(check_exit_code(&sample()), 1);
        let only_warn = vec![Diagnostic::warn(DiagCode::UnknownType, "w", "a.md", 1)];
        assert_eq!(check_exit_code(&only_warn), 0);
        assert_eq!(check_exit_code(&[]), 0);
    }

    #[test]
    fn formats_a_clean_file_and_detects_change() {
        // A default `[1]` is dropped by canonical form → the file changes.
        let files = vec![("x/a.md".to_string(),
            "---\ntype: uml.Class\ntitle: A\n---\n# A\n\n## Attributes\n- id: AId [1]\n".to_string())];
        let plan = plan_fmt(&files);
        assert_eq!(plan.len(), 1);
        assert!(!plan[0].skipped);
        assert!(plan[0].changed);
        assert!(plan[0].formatted.contains("- id: AId\n"));
        assert!(!plan[0].formatted.contains("[1]"));
    }

    #[test]
    fn skips_a_file_with_errors() {
        let files = vec![("x/a.md".to_string(),
            "---\ntype: uml.Class\ntitle: A\n---\n# A\n\n## Attributes\n- broken line\n".to_string())];
        let plan = plan_fmt(&files);
        assert!(plan[0].skipped);
        assert!(!plan[0].changed);
    }

    #[test]
    fn skips_a_file_with_pre_section_prose_instead_of_dropping_it() {
        // Regression: prose between the H1 title and the first `## ` section
        // used to be silently dropped by parse -> serialize with no
        // diagnostic, so `fmt` would rewrite the file and delete it. Now
        // `validate` flags it as an Error, so `plan_fmt` must skip the file
        // and leave its content byte-for-byte untouched.
        let original = "---\ntype: uml.Class\ntitle: A\n---\n# A\n\nDo not lose this sentence.\n\n## Attributes\n- id: AId\n";
        let files = vec![("x/a.md".to_string(), original.to_string())];
        let plan = plan_fmt(&files);
        assert_eq!(plan.len(), 1);
        assert!(plan[0].skipped, "expected the file to be skipped, not silently rewritten");
        assert!(!plan[0].changed);
        assert_eq!(plan[0].formatted, original, "skipped content must be byte-for-byte untouched");
    }
}
