use serde::Serialize;
use uaml::diagnostic::{Diagnostic, Severity};

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
}
