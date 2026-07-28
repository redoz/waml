//! Shell-backed entry point for the generic OKF projection.

use crate::{
    okf::{parse_bundle, Bundle, BundleError},
    source::SourceBundle,
};

/// The semantic projection stays deliberately domain-neutral.  `analysis`
/// validates and retains the corresponding shell snapshots before calling here.
pub(crate) fn derive(source: &SourceBundle) -> Result<Bundle, BundleError> {
    parse_bundle(source)
}
