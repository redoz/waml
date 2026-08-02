//! Access to the embedded web editor.
//!
//! The assets are stored brotli-compressed and are decompressed by whoever
//! writes them out. `waml export site` writes raw files: only a future `waml
//! serve` may keep the compressed form, and only as HTTP content encoding.

// The site assembler behind `waml export site` is the only consumer; until it
// lands, nothing in the binary reads these.
#![allow(dead_code)]

use std::fmt;

/// One file of the built web editor, kept in its compressed form.
#[derive(Debug)]
pub(crate) struct EmbeddedAsset {
    pub path: &'static str,
    pub brotli: &'static [u8],
}

include!(concat!(env!("OUT_DIR"), "/web_artifact.rs"));

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WebArtifactError {
    NotEmbedded,
}

impl fmt::Display for WebArtifactError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotEmbedded => f.write_str(
                "this waml binary has no embedded web editor. Build it with \
                 `--features embed-web` and set WAML_WEB_EMBED_DIR to a directory \
                 produced by `node scripts/package-web-artifact.mjs`.",
            ),
        }
    }
}

impl std::error::Error for WebArtifactError {}

/// The embedded editor, or an explanation of how to build a binary that has one.
pub(crate) fn embedded_artifact() -> Result<&'static [EmbeddedAsset], WebArtifactError> {
    if EMBEDDED.is_empty() {
        return Err(WebArtifactError::NotEmbedded);
    }
    Ok(EMBEDDED)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(not(feature = "embed-web"))]
    fn a_default_build_explains_how_to_embed_the_editor() {
        let error = embedded_artifact().unwrap_err();
        let message = error.to_string();
        assert!(message.contains("--features embed-web"), "{message}");
        assert!(message.contains("WAML_WEB_EMBED_DIR"), "{message}");
    }

    // A `--all-features` build enables embed-web with no artifact packaged, so
    // this asserts the shape of whatever the build script produced rather than
    // requiring an artifact that only CI and release builds have.
    #[test]
    #[cfg(feature = "embed-web")]
    fn an_embedded_build_carries_the_editor_entry_point() {
        let Ok(assets) = embedded_artifact() else {
            return;
        };
        assert!(assets.iter().any(|asset| asset.path == "index.html"));
        for asset in assets {
            assert!(!asset.path.starts_with('/'), "{}", asset.path);
            assert!(!asset.path.contains(".."), "{}", asset.path);
        }
    }
}
