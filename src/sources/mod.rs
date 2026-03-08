//! Source trait and implementations for populating the picker.
//!
//! Each source provides a list of items and knows how to act on a selection.

pub mod buffers;
pub mod files;
pub mod grep;

/// A single entry produced by a source.
#[derive(Debug, Clone)]
pub struct SourceItem {
    /// Display text shown in the picker results list.
    pub display: String,
    /// The value used for fuzzy matching (may differ from display).
    pub filter_text: String,
    /// Action payload — typically a file path or `path:line` location.
    pub action: String,
}

impl SourceItem {
    /// Create a source item where display, filter, and action are all the same.
    #[must_use]
    pub fn simple(text: &str) -> Self {
        Self {
            display: text.to_owned(),
            filter_text: text.to_owned(),
            action: text.to_owned(),
        }
    }
}

/// Trait implemented by each picker source (files, grep, buffers, etc.).
pub trait Source {
    /// Gather all items. For static sources this collects once; for live
    /// sources (grep) this may re-collect on each query change.
    fn collect(&self, query: &str) -> Vec<SourceItem>;

    /// Human-readable name shown in the picker title.
    fn name(&self) -> &str;

    /// Whether [`collect`] should be re-invoked when the query changes.
    /// Grep needs this (`true`); files and buffers do not (`false`).
    fn is_live(&self) -> bool {
        false
    }
}
