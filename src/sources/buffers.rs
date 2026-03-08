//! Buffer-list source: lists currently open Neovim buffers.

use super::{Source, SourceItem};
use nvim_oxi::api;

/// Source that lists open, listed Neovim buffers.
pub struct BufferSource;

impl BufferSource {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for BufferSource {
    fn default() -> Self {
        Self::new()
    }
}

impl Source for BufferSource {
    fn collect(&self, _query: &str) -> Vec<SourceItem> {
        let bufs = api::list_bufs();
        let mut items = Vec::new();

        for buf in bufs {
            // Only include listed buffers.
            #[allow(deprecated)]
            let listed = buf.get_option::<bool>("buflisted").unwrap_or(false);
            if !listed {
                continue;
            }

            let name = buf
                .get_name()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            if name.is_empty() {
                continue;
            }

            let buf_nr = buf.handle();

            let display = format!("{buf_nr}: {name}");
            items.push(SourceItem {
                display,
                filter_text: name.clone(),
                action: name,
            });
        }

        items
    }

    fn name(&self) -> &str {
        "Buffers"
    }
}
