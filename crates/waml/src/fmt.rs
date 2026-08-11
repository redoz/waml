//! Fmt planning shared by the CLI today and by any future editor/LSP format
//! command. `plan_fmt` encodes model rules any frontend offering "format
//! document" needs or it deletes user text: skip errored files byte-for-byte,
//! skip files with prose between the H1 and the first `##` (a data-loss guard
//! with a regression test), and pass index docs through verbatim.

use crate::action::SyntaxChangeBatch;
use crate::analysis::{prepare_candidate, PreviousAnalyses};
use crate::diagnostic::Severity;
use crate::edit::{EditBatch, EditContext};
use crate::source::{BundlePath, SourceBundle};
use crate::uml::{ActionContext, Formatter};

pub struct FmtResult {
    pub path: String,
    pub formatted: String,
    pub changed: bool,
    pub skipped: bool,
}

pub fn plan_fmt(files: &[(String, String)]) -> Result<Vec<FmtResult>, String> {
    let prepared = crate::validate::prepare(files)?;
    let action_context = ActionContext::from_prepared(&prepared).map_err(|e| e.to_string())?;
    let mut candidate_pairs: std::collections::BTreeMap<String, String> =
        prepared.source().to_pairs().into_iter().collect();

    for source_document in prepared.source().documents() {
        let document = prepared
            .okf()
            .catalog
            .id_for_path(source_document.path())
            .ok_or_else(|| format!("catalog has no document: {}", source_document.path()))?;
        let version = prepared
            .okf()
            .catalog
            .document(document)
            .ok_or_else(|| format!("catalog has no document: {}", source_document.path()))?;
        if prepared.uml().syntax.document(document).is_none() {
            continue;
        }
        let action = Formatter
            .format(
                ActionContext::new(
                    action_context.okf(),
                    action_context.uml(),
                    action_context.session_revision(),
                )
                .map_err(|e| e.to_string())?,
                document,
            )
            .map_err(|e| e.to_string())?;
        let formatted = SyntaxChangeBatch::new(action)
            .map_err(|e| e.to_string())?
            .lower(EditContext {
                source: prepared.source(),
                okf_analysis: prepared.okf(),
                session_revision: prepared.revision(),
                uml: prepared.uml(),
            })
            .map_err(|e| e.to_string())?;
        let formatted_document = formatted
            .document(version.path())
            .ok_or_else(|| format!("formatted document disappeared: {}", version.path()))?;
        if formatted_document.text() != version.text().shared().as_str() {
            candidate_pairs.insert(
                version.path().to_string(),
                formatted_document.text().to_owned(),
            );
        }
    }

    let validated = prepare_candidate(
        SourceBundle::try_from_pairs(candidate_pairs).map_err(|e| e.to_string())?,
        Some(PreviousAnalyses {
            okf: prepared.okf(),
            uml: prepared.uml(),
        }),
        1,
    )
    .map_err(|e| e.to_string())?;
    let diagnostics = crate::validate::diagnostics(&validated, &std::collections::BTreeMap::new());
    Ok(files
        .iter()
        .map(|(path, original)| {
            let bundle_path = BundlePath::parse(path.clone()).expect("prepared path is valid");
            let document = validated
                .okf()
                .catalog
                .id_for_path(&bundle_path)
                .expect("validated catalog document exists");
            let formatted = validated
                .source()
                .document(&bundle_path)
                .expect("validated document exists")
                .text()
                .to_owned();
            let is_index = crate::index_md::is_index_basename(path);
            let claimed = prepared.uml().syntax.document(document);
            let skipped = !is_index
                && (diagnostics.iter().any(|diagnostic| {
                    diagnostic.file == *path && diagnostic.severity == Severity::Error
                }) || claimed
                    .is_some_and(|snapshot| !snapshot.syntax().diagnostics().is_empty())
                    || (claimed.is_some()
                        && prepared
                            .uml()
                            .markdown
                            .document(document)
                            .is_some_and(|snapshot| {
                                let structure = snapshot.structure();
                                let Some(h1) =
                                    structure.headings.iter().find(|heading| heading.level == 1)
                                else {
                                    return false;
                                };
                                let after_h1 = original[h1.range.end().to_usize()..]
                                    .find('\n')
                                    .map(|offset| h1.range.end().to_usize() + offset + 1)
                                    .unwrap_or_else(|| h1.range.end().to_usize());
                                let first_h2 = structure
                                    .headings
                                    .iter()
                                    .find(|heading| heading.level == 2)
                                    .map(|heading| heading.range.start().to_usize())
                                    .unwrap_or(original.len());
                                after_h1 < first_h2
                                    && !original[after_h1..first_h2].trim().is_empty()
                            })));
            FmtResult {
                path: path.clone(),
                changed: !skipped && formatted != *original,
                formatted: if skipped { original.clone() } else { formatted },
                skipped,
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_fmt_leaves_index_docs_untouched() {
        // An index doc's navigation is not round-trippable through the node
        // serializer (it would be dropped), so fmt must pass it through verbatim
        // rather than skip it as errored or rewrite it and lose the nav.
        let files = vec![
            (
                "alerts/index.md".to_string(),
                "# Alerts\n\nAlert docs.\n\n* [Alert](./alert.md) - a signal\n".to_string(),
            ),
            (
                "alerts/alert.md".to_string(),
                "---\ntype: uml.Class\ntitle: Alert\n---\n# Alert\n".to_string(),
            ),
        ];
        let out = plan_fmt(&files).unwrap();
        let ix = out.iter().find(|r| r.path == "alerts/index.md").unwrap();
        assert!(!ix.skipped, "index doc must not be skipped as errored");
        assert!(!ix.changed, "index nav must be preserved verbatim");
        assert_eq!(
            ix.formatted, files[0].1,
            "index content must round-trip unchanged"
        );
    }

    #[test]
    fn plan_fmt_still_skips_error_files_byte_for_byte() {
        let original = "---\ntype: uml.Class\ntitle: A\n---\n# A\n\nDo not lose this sentence.\n\n## Attributes\n- id: AId\n";
        let files = vec![("x/a.md".to_string(), original.to_string())];
        let plan = plan_fmt(&files).unwrap();
        assert!(plan[0].skipped);
        assert_eq!(plan[0].formatted, original);
    }

    #[test]
    fn formats_a_clean_file_and_detects_change() {
        // An authored default `{1}` is preserved by canonical form.
        let files = vec![(
            "x/a.md".to_string(),
            "---\ntype: uml.Class\ntitle: A\n---\n# A\n\n## Attributes\n- id: AId {1}\n"
                .to_string(),
        )];
        let plan = plan_fmt(&files).unwrap();
        assert_eq!(plan.len(), 1);
        assert!(!plan[0].skipped);
        assert!(plan[0].changed);
        assert!(plan[0].formatted.contains("- id: AId {1}\n"));
    }

    #[test]
    fn skips_a_file_with_errors() {
        let files = vec![(
            "x/a.md".to_string(),
            "---\ntype: uml.Class\ntitle: A\n---\n# A\n\n## Attributes\n- broken line\n"
                .to_string(),
        )];
        let plan = plan_fmt(&files).unwrap();
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
        let plan = plan_fmt(&files).unwrap();
        assert_eq!(plan.len(), 1);
        assert!(
            plan[0].skipped,
            "expected the file to be skipped, not silently rewritten"
        );
        assert!(!plan[0].changed);
        assert_eq!(
            plan[0].formatted, original,
            "skipped content must be byte-for-byte untouched"
        );
    }
}
