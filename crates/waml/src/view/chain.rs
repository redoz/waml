//! The middleware chain runner. Stub type only for now — see Task B5 for the
//! runner itself.

/// A resolved sequence of [`super::projection::Projection`] stages for one
/// folder. Fleshed out in Task B5.
#[derive(Default)]
pub struct Chain {
    _private: (),
}

/// Runner bounds. Constructed by the HOST (editor from `.waml/settings.json`,
/// tests directly, LSP from its own config) and passed in. There is no
/// constructor that reads a bundle: bundle-supplied `max_view_depth` is
/// unreachable by construction, not by filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChainLimits {
    /// Maximum descent depth the runner will walk before giving up.
    pub max_depth: usize,
}

impl Default for ChainLimits {
    fn default() -> Self {
        ChainLimits { max_depth: 20 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chain_limits_default_is_twenty() {
        assert_eq!(ChainLimits::default().max_depth, 20);
    }

    #[test]
    fn bundle_frontmatter_max_view_depth_never_reaches_the_runner() {
        // ChainLimits has no constructor that reads a bundle or an Index's
        // frontmatter -- the only way to get one is `default()` or building
        // the struct literal directly. A bundle whose root and folder
        // indexes both declare `max_view_depth: 3` therefore cannot affect
        // this value: nothing ever reads that key on this path. This test
        // documents the invariant at the type level; the descent-depth
        // assertion against a live bundle is added once B6's runner exists.
        let limits = ChainLimits::default();
        assert_eq!(limits.max_depth, 20);
    }
}
