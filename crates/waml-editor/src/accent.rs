//! The accent-bucket swatch table: `AccentBucket` -> the colour that bucket
//! actually paints with, plus the `TreeKind` entry point chrome uses.
//!
//! Split out of `node_style.rs` rather than living beside `accent_bucket`
//! because `node_style` is also compiled into the standalone node-editor
//! harness bin, where these would be dead code -- and the gate promotes
//! `dead_code` to a hard error. This module is shell-only.

use makepad_widgets::Vec4;

use crate::node_style::AccentBucket;
use crate::tree::TreeKind;

fn rgb(hex: u32) -> Vec4 {
    Vec4 {
        x: ((hex >> 16) & 0xff) as f32 / 255.0,
        y: ((hex >> 8) & 0xff) as f32 / 255.0,
        z: (hex & 0xff) as f32 / 255.0,
        w: 1.0,
    }
}

/// The Atlas `bucket_*` swatch for an accent bucket (`None` and `Unknown` share
/// the neutral slate). Literal hex rather than an atlas token because the bucket
/// swatches are theme-invariant by design -- the same saturated HUD colours in
/// light and dark (see the bucket notes in `theme_atlas.rs`).
pub fn bucket_color(b: AccentBucket) -> Vec4 {
    match b {
        AccentBucket::Interface => rgb(0x1496dc),
        AccentBucket::Enum => rgb(0x00b4d2),
        AccentBucket::Note => rgb(0x14bea0),
        AccentBucket::Actor => rgb(0x5a6ef0),
        AccentBucket::UseCase => rgb(0xe69614),
        AccentBucket::Package => rgb(0x3cbe5a),
        AccentBucket::Behavior => rgb(0xeb4678),
        AccentBucket::None | AccentBucket::Unknown => rgb(0x64748b),
    }
}

/// Which accent bucket a *tree* kind renders with -- the same decision as
/// `node_style::accent_bucket`, taken one step downstream. The project tree and
/// the doc-tab strip carry a `TreeKind` rather than the `ElementType` it was
/// derived from, and re-resolving the element just to colour a tab would mean
/// threading the model into chrome that has no other use for it.
///
/// `Class` and `Diagram` map to `None` for the same reason they do upstream: a
/// plain class carries no accent bar on the canvas either.
pub fn tree_kind_bucket(kind: TreeKind) -> AccentBucket {
    match kind {
        TreeKind::Interface => AccentBucket::Interface,
        TreeKind::Enum | TreeKind::DataType => AccentBucket::Enum,
        TreeKind::Note => AccentBucket::Note,
        TreeKind::Directory => AccentBucket::Package,
        TreeKind::OkfDocument => AccentBucket::Unknown,
        TreeKind::Behavior | TreeKind::Sequence => AccentBucket::Behavior,
        TreeKind::Class | TreeKind::Diagram => AccentBucket::None,
    }
}

/// The bucket swatch for a tree kind, or `None` where the bucket carries no
/// colour of its own (plain class, diagram, unresolved type) and the caller
/// should fall back to its own default -- for the doc tabs, the theme accent.
pub fn tree_kind_color(kind: TreeKind) -> Option<Vec4> {
    match tree_kind_bucket(kind) {
        AccentBucket::None | AccentBucket::Unknown => None,
        b => Some(bucket_color(b)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_kinds_agree_with_the_element_type_buckets() {
        // The two mappings are written out separately, so pin the pairs that
        // would silently drift if one side gained a metaclass.
        assert_eq!(
            tree_kind_bucket(TreeKind::Interface),
            AccentBucket::Interface
        );
        assert_eq!(tree_kind_bucket(TreeKind::Enum), AccentBucket::Enum);
        assert_eq!(tree_kind_bucket(TreeKind::DataType), AccentBucket::Enum);
        assert_eq!(tree_kind_bucket(TreeKind::Directory), AccentBucket::Package);
        assert_eq!(tree_kind_bucket(TreeKind::Behavior), AccentBucket::Behavior);
        assert_eq!(tree_kind_bucket(TreeKind::Sequence), AccentBucket::Behavior);
        assert_eq!(tree_kind_bucket(TreeKind::Note), AccentBucket::Note);
    }

    #[test]
    fn colourless_buckets_defer_to_the_callers_default() {
        // A plain class tab and the diagram base tab both keep the theme
        // accent; only a kind with a bucket of its own overrides it.
        assert!(tree_kind_color(TreeKind::Class).is_none());
        assert!(tree_kind_color(TreeKind::Diagram).is_none());
        assert!(tree_kind_color(TreeKind::OkfDocument).is_none());
        assert!(tree_kind_color(TreeKind::Enum).is_some());
    }

    #[test]
    fn every_bucket_swatch_is_opaque_and_distinct_from_the_neutral() {
        for b in [
            AccentBucket::Interface,
            AccentBucket::Enum,
            AccentBucket::Note,
            AccentBucket::Actor,
            AccentBucket::UseCase,
            AccentBucket::Package,
            AccentBucket::Behavior,
        ] {
            let c = bucket_color(b);
            assert_eq!(c.w, 1.0, "{b:?} swatch must be opaque");
            assert_ne!(
                (c.x, c.y, c.z),
                {
                    let n = bucket_color(AccentBucket::None);
                    (n.x, n.y, n.z)
                },
                "{b:?} must not collapse onto the neutral slate"
            );
        }
    }
}
