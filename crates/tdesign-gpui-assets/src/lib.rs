//! TDesign SVG icons and a composable GPUI [`AssetSource`].

use gpui::{AssetSource, Result, SharedString};
use std::{borrow::Cow, fmt, sync::Arc};

#[allow(missing_docs)]
mod generated_icon_names {
    use super::UnknownIcon;
    include!(concat!(env!("OUT_DIR"), "/icon_names.rs"));
}
pub use generated_icon_names::IconName;
include!(concat!(env!("OUT_DIR"), "/embedded_assets.rs"));

/// Error returned when parsing an unknown TDesign icon name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnknownIcon(pub String);

impl fmt::Display for UnknownIcon {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "unknown TDesign icon: {}", self.0)
    }
}

impl std::error::Error for UnknownIcon {}

/// An asset source that serves embedded TDesign icons before consulting an
/// optional application-provided fallback source.
#[derive(Default)]
pub struct TDesignAssetSource {
    fallback: Option<Arc<dyn AssetSource>>,
}

impl TDesignAssetSource {
    /// Creates an asset source containing TDesign assets only.
    pub fn new() -> Self {
        Self::default()
    }

    /// Wraps another source, preserving application assets outside the
    /// `tdesign/` namespace.
    pub fn with_fallback(fallback: impl AssetSource) -> Self {
        Self {
            fallback: Some(Arc::new(fallback)),
        }
    }

    /// Wraps a dynamically-dispatched fallback source.
    pub fn with_shared_fallback(fallback: Arc<dyn AssetSource>) -> Self {
        Self {
            fallback: Some(fallback),
        }
    }

    /// Returns whether an icon is compiled into this feature set.
    pub fn contains(icon: IconName) -> bool {
        embedded(icon.asset_path().as_ref()).is_some()
    }

    /// Returns the embedded asset paths.
    pub fn paths() -> &'static [&'static str] {
        EMBEDDED_PATHS
    }
}

impl AssetSource for TDesignAssetSource {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some(bytes) = embedded(path) {
            return Ok(Some(Cow::Borrowed(bytes)));
        }
        match &self.fallback {
            Some(fallback) => fallback.load(path),
            None => Ok(None),
        }
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut result = EMBEDDED_PATHS
            .iter()
            .filter(|candidate| candidate.starts_with(path))
            .map(|candidate| SharedString::from(*candidate))
            .collect::<Vec<_>>();
        if let Some(fallback) = &self.fallback {
            result.extend(fallback.list(path)?);
        }
        result.sort();
        result.dedup();
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_names_round_trip() {
        for icon in IconName::ALL {
            assert_eq!(icon.as_str().parse::<IconName>(), Ok(*icon));
        }
    }

    #[test]
    fn assets_are_namespaced() {
        assert!(
            TDesignAssetSource::paths()
                .iter()
                .all(|path| path.starts_with("tdesign/icons/"))
        );
    }
}
