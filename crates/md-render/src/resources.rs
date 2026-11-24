//! Image resource lifecycle and layout invalidation.

use std::collections::HashMap;

use md_core::Size;

/// Image loading state.
#[derive(Clone, Debug, PartialEq)]
pub enum ImageState {
    /// Loading in progress.
    Loading,
    /// Loaded with intrinsic size.
    Loaded {
        /// Intrinsic size.
        size: Size,
    },
    /// Failed with a message.
    Failed {
        /// Failure message.
        message: String,
    },
}

/// Image cache keyed by source.
#[derive(Clone, Debug, Default)]
pub struct ResourceCache {
    entries: HashMap<String, ImageState>,
    version: u64,
}

impl ResourceCache {
    /// Creates an empty cache.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Starts loading a source.
    pub fn begin(&mut self, source: impl Into<String>) {
        self.entries.insert(source.into(), ImageState::Loading);
        self.version += 1;
    }

    /// Marks a source loaded and bumps the cache version.
    pub fn loaded(&mut self, source: impl Into<String>, size: Size) -> bool {
        let changed = self
            .entries
            .insert(source.into(), ImageState::Loaded { size })
            .is_none_or(|previous| previous != ImageState::Loaded { size });
        if changed {
            self.version += 1;
        }
        changed
    }

    /// Marks a source failed.
    pub fn failed(&mut self, source: impl Into<String>, message: impl Into<String>) {
        self.entries.insert(
            source.into(),
            ImageState::Failed {
                message: message.into(),
            },
        );
        self.version += 1;
    }

    /// Returns the state for a source.
    #[must_use]
    pub fn get(&self, source: &str) -> Option<&ImageState> {
        self.entries.get(source)
    }

    /// Returns the cache version, bumped when layout-affecting state changes.
    #[must_use]
    pub const fn version(&self) -> u64 {
        self.version
    }

    /// Returns whether a source needs a placeholder.
    #[must_use]
    pub fn is_placeholder(&self, source: &str) -> bool {
        !matches!(self.get(source), Some(ImageState::Loaded { .. }))
    }
}

#[cfg(test)]
mod tests {
    use super::{ImageState, ResourceCache};
    use md_core::Size;

    #[test]
    fn loading_bumps_version_and_reports_state() {
        let mut cache = ResourceCache::new();
        cache.begin("a.png");
        assert!(cache.is_placeholder("a.png"));
        assert_eq!(cache.version(), 1);
        assert!(cache.loaded("a.png", Size::new(100.0, 50.0)));
        assert_eq!(cache.version(), 2);
        assert!(matches!(
            cache.get("a.png"),
            Some(ImageState::Loaded { .. })
        ));
    }

    #[test]
    fn repeated_identical_load_does_not_bump_version() {
        let mut cache = ResourceCache::new();
        cache.loaded("a.png", Size::new(10.0, 10.0));
        let version = cache.version();
        cache.loaded("a.png", Size::new(10.0, 10.0));
        assert_eq!(cache.version(), version);
    }
}
