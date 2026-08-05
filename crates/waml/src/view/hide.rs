//! `hide` middleware (Phase F): drops rows whose target matches a
//! `hide: [glob, ...]` param declared in the folder's index frontmatter.
//! Paths map one-to-one onto inner paths -- `apply` forwards to the inner
//! stage unchanged for surviving rows, `hide` needs no edit code and must
//! not break editing beneath it.
//!
//! **`hide` is presentational, never a permission boundary.** A hidden row
//! is filtered out of the declared listing only; it is still a real file on
//! disk, still reachable through the raw OKF layer
//! ([`super::chain::Chain::raw`]), and still reachable by search or a direct
//! link. Nothing here implies protection or access control.
//!
//! Resolution of a hidden path is deliberately absent: `hide` never forwards
//! `resolve` for a path its own `project` would have dropped -- hidden means
//! hidden through the chain, without exception. A search hit that lands on a
//! hidden path must open through the raw OKF layer instead of the declared
//! chain, or clicking the result would silently do nothing.

use crate::diagnostic::{DiagCode, Diagnostic};
use crate::frontmatter::{FmValue, Frontmatter};
use crate::okf;

use super::projection::{
    Next, Projection, ProjectionCtx, ProjectionError, RowOp, Unresolved, Unsupported,
};
use super::row::{Row, RowPath, RowTarget};
use super::surface::SurfaceId;

/// Read and validate the `hide: [glob, ...]` param out of a folder's index
/// frontmatter. `Err` carries the message for an `InvalidViewParams`
/// diagnostic -- missing entirely, not a list, or a list containing
/// anything other than non-empty strings.
pub(crate) fn parse_hide_globs(params: &Frontmatter) -> Result<Vec<String>, String> {
    match params.get("hide") {
        None => Err("`hide` middleware requires a `hide: [glob, ...]` param".to_string()),
        Some(FmValue::List(items)) => {
            let mut globs = Vec::with_capacity(items.len());
            for item in items {
                match item {
                    FmValue::Str(s) if !s.is_empty() => globs.push(s.clone()),
                    _ => return Err("`hide` entries must be non-empty glob strings".to_string()),
                }
            }
            Ok(globs)
        }
        Some(_) => Err("`hide` must be a list of glob strings".to_string()),
    }
}

/// Build the declaration-time diagnostic `Chain::build` attaches when
/// `hide`'s params fail [`parse_hide_globs`] -- spanned on the `view:` entry
/// that named this middleware (there is no finer-grained span in this
/// diagnostic model; see the other `view:` entry diagnostics in
/// `super::chain`).
pub(crate) fn invalid_params_diagnostic(message: String, file: &str, line: usize) -> Diagnostic {
    Diagnostic::new(DiagCode::InvalidViewParams, message, file.to_string(), line)
}

/// A minimal glob matcher over a `/`-joined path: `*` matches a run of
/// characters within one path segment (never `/`), `**` (or any run of two
/// or more consecutive `*`) matches any run of characters including `/`.
/// No character classes, no `?`, no brace expansion -- `hide:` patterns are
/// simple prefix-style excludes (`"references/**"`, `"*.draft.md"`), not
/// general glob syntax.
///
/// Implemented as a bounded dynamic-programming table (`O(pattern_tokens *
/// candidate_len)`), not naive backtracking: `hide:` globs come from
/// frontmatter, which is untrusted document content, and a backtracking
/// matcher over a pattern with many `*` runs is a classic exponential-time
/// trap for hostile input.
fn glob_match(pattern: &str, candidate: &str) -> bool {
    enum Token {
        Lit(char),
        Star,
        DoubleStar,
    }

    let mut tokens = Vec::new();
    let chars: Vec<char> = pattern.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '*' {
            let mut j = i;
            while j < chars.len() && chars[j] == '*' {
                j += 1;
            }
            tokens.push(if j - i >= 2 {
                Token::DoubleStar
            } else {
                Token::Star
            });
            i = j;
        } else {
            tokens.push(Token::Lit(chars[i]));
            i += 1;
        }
    }

    let cand: Vec<char> = candidate.chars().collect();
    let m = cand.len();
    let mut dp = vec![false; m + 1];
    dp[0] = true;
    for token in &tokens {
        let mut next_dp = vec![false; m + 1];
        match token {
            Token::Lit(c) => {
                for j in 0..m {
                    if dp[j] && cand[j] == *c {
                        next_dp[j + 1] = true;
                    }
                }
            }
            // A star matches zero or more allowed characters: at every
            // position `j`, either it consumed nothing here (`dp[j]`, the
            // state before this token already reached `j`) or it consumed
            // one more allowed character extending its own previous reach
            // (`next_dp[j - 1]`) -- checked at EVERY `j`, not only `j == 0`;
            // omitting the zero-width case at interior/final positions was
            // the bug an earlier version of this function shipped with (a
            // star immediately followed by nothing never matched at all).
            Token::Star => {
                for j in 0..=m {
                    next_dp[j] = dp[j] || (j > 0 && next_dp[j - 1] && cand[j - 1] != '/');
                }
            }
            Token::DoubleStar => {
                for j in 0..=m {
                    next_dp[j] = dp[j] || (j > 0 && next_dp[j - 1]);
                }
            }
        }
        dp = next_dp;
    }
    dp[m]
}

/// The glob-space candidate string for one row's target. A folder gets a
/// trailing `/` so the common `"dir/**"` idiom hides the folder's own row
/// as well as everything nested under it, not only paths strictly beneath
/// it -- `**` accepts a zero-length match after the trailing `/`.
fn row_candidate(row: &Row) -> Option<String> {
    match &row.target {
        RowTarget::Concept(href) => Some(href.clone()),
        RowTarget::Folder(address) => {
            Some(format!("{}/", address.as_str().trim_start_matches('/')))
        }
        // Nothing in this folder resolves to a Virtual target on this
        // chain's own row-projection layer -- never a hide candidate.
        RowTarget::Virtual => None,
    }
}

fn matches_any(globs: &[String], row: &Row) -> bool {
    let Some(candidate) = row_candidate(row) else {
        return false;
    };
    globs.iter().any(|glob| glob_match(glob, &candidate))
}

pub(crate) struct Hide;

impl Projection for Hide {
    fn project(
        &self,
        ctx: &ProjectionCtx<'_>,
        next: Next<'_>,
    ) -> Result<Vec<Row>, ProjectionError> {
        // Re-validated here even though `Chain::build` already checked --
        // this stage is reachable directly (unit tests, a future caller
        // that builds a `Chain` by hand) without going through `build`'s
        // declaration-time gate.
        let globs = parse_hide_globs(ctx.params).map_err(|message| ProjectionError { message })?;
        let rows = next.project(ctx)?;
        Ok(rows
            .into_iter()
            .filter(|row| !matches_any(&globs, row))
            .collect())
    }

    /// Hidden means hidden through the chain, without exception -- never
    /// forward `resolve` for a path this stage's own `project` would have
    /// dropped. The runner falls back to the nearest resolvable prefix; the
    /// raw OKF layer (chain pinned to `[index]`) is the reachability path
    /// for a hidden path, not this one.
    fn resolve(&self, _ctx: &ProjectionCtx<'_>, _path: &RowPath) -> Result<Vec<Row>, Unresolved> {
        Err(Unresolved)
    }

    /// Paths map one-to-one onto inner paths for every *surviving* row --
    /// `hide` needs no edit code of its own and must not break editing
    /// beneath it.
    fn apply(
        &self,
        ctx: &ProjectionCtx<'_>,
        path: &RowPath,
        op: RowOp,
        next: Next<'_>,
    ) -> Result<Vec<okf::Op>, Unsupported> {
        next.apply(ctx, path, op)
    }

    fn surface(&self, ctx: &ProjectionCtx<'_>, next: Next<'_>) -> SurfaceId {
        next.surface(ctx)
    }

    /// The half of "hidden means hidden" that `resolve` alone cannot deliver.
    ///
    /// A row this stage drops keeps the `ViewId` of whichever stage minted it
    /// -- the root view, for an ordinary listing -- so a `RowId` captured
    /// while the row was visible names an owner further down the chain, and
    /// owner dispatch would sail straight past this stage. The runner asks
    /// every stage ahead of the owner this question instead.
    ///
    /// Rebuilds the row through the root view so the candidate string matches
    /// `project`'s exactly: one glob, one meaning, whether the path arrived by
    /// projection or by deep link. A path naming nothing in this folder
    /// occludes nothing -- it is not ours to hide.
    ///
    /// Presentational, never a permission boundary: the raw OKF layer resolves
    /// this same path, by design.
    fn occludes(&self, ctx: &ProjectionCtx<'_>, path: &RowPath) -> bool {
        let Ok(globs) = parse_hide_globs(ctx.params) else {
            return false;
        };
        match super::root::RootView.resolve(ctx, path) {
            Ok(rows) => rows.last().is_some_and(|row| matches_any(&globs, row)),
            Err(Unresolved) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glob_match_supports_double_star_prefix_and_single_star_suffix() {
        assert!(glob_match("references/**", "references/a/b"));
        assert!(glob_match("references/**", "references/a"));
        assert!(!glob_match("references/**", "reference"));
        assert!(glob_match("*.draft.md", "notes.draft.md"));
        assert!(!glob_match("*.draft.md", "a/notes.draft.md"));
    }

    #[test]
    fn glob_match_does_not_explode_on_many_stars() {
        // Regression guard for the DP implementation: a pathological
        // backtracking matcher goes exponential on a run like this; the DP
        // table stays O(tokens * len) regardless.
        let pattern = "*".repeat(200) + "x";
        assert!(!glob_match(&pattern, &"a".repeat(200)));
    }

    #[test]
    fn a_folder_row_matches_its_own_dir_slash_star_star_pattern() {
        // Regression guard: an earlier DP formulation only applied a
        // star/double-star's zero-width match at index 0, so a pattern
        // whose `**` sat at the very end (matching nothing further) never
        // matched at all -- exactly the `"dir/**"` idiom this middleware's
        // own doc comment calls the common case.
        use crate::view::row::{RowId, ViewId};

        let globs = vec!["references/**".to_string()];
        let row = Row::new(
            RowId {
                owner: ViewId::new("x"),
                path: RowPath::parse("references").unwrap(),
            },
            "References".to_string(),
            RowTarget::Folder("/references".to_string()),
            None,
        )
        .unwrap();
        assert!(matches_any(&globs, &row));
    }
}
