//! Core picker state machine.
//!
//! Manages the query string, filtered results, and selection cursor.
//! This module is pure state — no Neovim API calls — so it can be
//! thoroughly unit-tested.

use crate::sources::SourceItem;
use furui::FuzzyMatcher;

/// An action the picker wants the caller to execute.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PickerAction {
    /// User confirmed a selection. Payload is the action string from the
    /// selected [`SourceItem`].
    Accept(String),
    /// User cancelled the picker.
    Cancel,
}

/// The picker's internal state.
#[derive(Debug)]
pub struct PickerState {
    /// Current query string.
    query: String,
    /// Unfiltered items from the source.
    all_items: Vec<SourceItem>,
    /// Indices into `all_items` that match the current query, in ranked order.
    filtered: Vec<usize>,
    /// Currently highlighted index within `filtered`.
    selected: usize,
    /// Scroll offset (index of first visible item).
    scroll_offset: usize,
    /// Number of visible result rows.
    visible_rows: usize,
    /// Fuzzy matcher instance.
    matcher: FuzzyMatcher,
}

impl PickerState {
    /// Create a new picker state with the given items and visible row count.
    #[must_use]
    pub fn new(items: Vec<SourceItem>, visible_rows: usize) -> Self {
        let filtered: Vec<usize> = (0..items.len()).collect();
        Self {
            query: String::new(),
            all_items: items,
            filtered,
            selected: 0,
            scroll_offset: 0,
            visible_rows,
            matcher: FuzzyMatcher::new(),
        }
    }

    /// Replace the items (used when a live source re-collects).
    pub fn set_items(&mut self, items: Vec<SourceItem>) {
        self.all_items = items;
        self.refilter();
    }

    // -- Query manipulation --------------------------------------------------

    /// The current query string.
    #[must_use]
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Append a character to the query.
    pub fn push_char(&mut self, ch: char) {
        self.query.push(ch);
        self.refilter();
    }

    /// Delete the last character from the query.
    pub fn pop_char(&mut self) {
        self.query.pop();
        self.refilter();
    }

    /// Replace the entire query.
    pub fn set_query(&mut self, query: &str) {
        self.query = query.to_owned();
        self.refilter();
    }

    /// Clear the query.
    pub fn clear_query(&mut self) {
        self.query.clear();
        self.refilter();
    }

    // -- Navigation ----------------------------------------------------------

    /// Move selection down by one.
    pub fn select_next(&mut self) {
        if !self.filtered.is_empty() {
            self.selected = (self.selected + 1).min(self.filtered.len() - 1);
            self.ensure_visible();
        }
    }

    /// Move selection up by one.
    pub fn select_prev(&mut self) {
        self.selected = self.selected.saturating_sub(1);
        self.ensure_visible();
    }

    /// Jump to the first item.
    pub fn select_first(&mut self) {
        self.selected = 0;
        self.scroll_offset = 0;
    }

    /// Jump to the last item.
    pub fn select_last(&mut self) {
        if !self.filtered.is_empty() {
            self.selected = self.filtered.len() - 1;
            self.ensure_visible();
        }
    }

    /// The currently selected index within the filtered list.
    #[must_use]
    pub fn selected_index(&self) -> usize {
        self.selected
    }

    /// The scroll offset.
    #[must_use]
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    // -- Actions -------------------------------------------------------------

    /// Accept the current selection. Returns `None` if the list is empty.
    #[must_use]
    pub fn accept(&self) -> Option<PickerAction> {
        let idx = *self.filtered.get(self.selected)?;
        let item = &self.all_items[idx];
        Some(PickerAction::Accept(item.action.clone()))
    }

    /// Cancel the picker.
    #[must_use]
    pub fn cancel(&self) -> PickerAction {
        PickerAction::Cancel
    }

    // -- Rendering helpers ---------------------------------------------------

    /// Get the display lines for the visible portion of the filtered results.
    #[must_use]
    pub fn visible_lines(&self) -> Vec<&str> {
        let end = (self.scroll_offset + self.visible_rows).min(self.filtered.len());
        self.filtered[self.scroll_offset..end]
            .iter()
            .map(|&idx| self.all_items[idx].display.as_str())
            .collect()
    }

    /// Index within the visible slice that is currently selected.
    #[must_use]
    pub fn visible_selected(&self) -> usize {
        self.selected.saturating_sub(self.scroll_offset)
    }

    /// Total number of filtered results.
    #[must_use]
    pub fn filtered_count(&self) -> usize {
        self.filtered.len()
    }

    /// Total number of items (before filtering).
    #[must_use]
    pub fn total_count(&self) -> usize {
        self.all_items.len()
    }

    /// The prompt line: `> {query}` with a cursor indicator.
    #[must_use]
    pub fn prompt_line(&self) -> String {
        format!("> {}", self.query)
    }

    /// Status line: `{filtered}/{total}`.
    #[must_use]
    pub fn status_line(&self) -> String {
        format!("{}/{}", self.filtered.len(), self.all_items.len())
    }

    // -- Internals -----------------------------------------------------------

    fn refilter(&mut self) {
        if self.query.is_empty() {
            self.filtered = (0..self.all_items.len()).collect();
        } else {
            let candidates: Vec<&str> = self
                .all_items
                .iter()
                .map(|item| item.filter_text.as_str())
                .collect();

            let ranked = self.matcher.rank(&self.query, &candidates);

            self.filtered = ranked.into_iter().map(|r| r.index).collect();
        }

        // Reset selection to top after re-filtering.
        self.selected = 0;
        self.scroll_offset = 0;
    }

    fn ensure_visible(&mut self) {
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        } else if self.selected >= self.scroll_offset + self.visible_rows {
            self.scroll_offset = self.selected - self.visible_rows + 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_items() -> Vec<SourceItem> {
        vec![
            SourceItem::simple("src/main.rs"),
            SourceItem::simple("src/lib.rs"),
            SourceItem::simple("Cargo.toml"),
            SourceItem::simple("README.md"),
            SourceItem::simple("src/picker.rs"),
            SourceItem::simple("src/ui.rs"),
        ]
    }

    #[test]
    fn initial_state_shows_all_items() {
        let state = PickerState::new(sample_items(), 10);
        assert_eq!(state.filtered_count(), 6);
        assert_eq!(state.total_count(), 6);
        assert_eq!(state.selected_index(), 0);
        assert_eq!(state.query(), "");
    }

    #[test]
    fn push_char_filters_results() {
        let mut state = PickerState::new(sample_items(), 10);
        state.push_char('m');
        state.push_char('a');
        state.push_char('i');
        state.push_char('n');

        // "main" should match at least src/main.rs
        assert!(
            state.filtered_count() >= 1,
            "should match at least one item for 'main'"
        );

        let lines = state.visible_lines();
        assert!(
            lines.iter().any(|l| l.contains("main")),
            "visible lines should include main: {lines:?}"
        );
    }

    #[test]
    fn pop_char_widens_results() {
        let mut state = PickerState::new(sample_items(), 10);
        state.set_query("main");
        let narrow = state.filtered_count();

        state.pop_char(); // "mai"
        let wider = state.filtered_count();

        assert!(
            wider >= narrow,
            "removing a char should not reduce results: narrow={narrow}, wider={wider}"
        );
    }

    #[test]
    fn clear_query_shows_all() {
        let mut state = PickerState::new(sample_items(), 10);
        state.set_query("xyz");
        assert_eq!(state.filtered_count(), 0);

        state.clear_query();
        assert_eq!(state.filtered_count(), 6);
    }

    #[test]
    fn select_next_and_prev() {
        let mut state = PickerState::new(sample_items(), 10);
        assert_eq!(state.selected_index(), 0);

        state.select_next();
        assert_eq!(state.selected_index(), 1);

        state.select_next();
        assert_eq!(state.selected_index(), 2);

        state.select_prev();
        assert_eq!(state.selected_index(), 1);
    }

    #[test]
    fn select_next_clamps_at_end() {
        let mut state = PickerState::new(sample_items(), 10);
        for _ in 0..100 {
            state.select_next();
        }
        assert_eq!(state.selected_index(), 5); // last item
    }

    #[test]
    fn select_prev_clamps_at_zero() {
        let mut state = PickerState::new(sample_items(), 10);
        state.select_prev();
        assert_eq!(state.selected_index(), 0);
    }

    #[test]
    fn select_first_and_last() {
        let mut state = PickerState::new(sample_items(), 10);
        state.select_last();
        assert_eq!(state.selected_index(), 5);

        state.select_first();
        assert_eq!(state.selected_index(), 0);
    }

    #[test]
    fn accept_returns_action() {
        let state = PickerState::new(sample_items(), 10);
        let action = state.accept().unwrap();
        assert_eq!(action, PickerAction::Accept("src/main.rs".to_owned()));
    }

    #[test]
    fn accept_on_empty_returns_none() {
        let mut state = PickerState::new(sample_items(), 10);
        state.set_query("zzzzzzz_no_match");
        assert!(state.accept().is_none());
    }

    #[test]
    fn cancel_action() {
        let state = PickerState::new(sample_items(), 10);
        assert_eq!(state.cancel(), PickerAction::Cancel);
    }

    #[test]
    fn scrolling_works() {
        let mut state = PickerState::new(sample_items(), 3);
        assert_eq!(state.scroll_offset(), 0);
        assert_eq!(state.visible_lines().len(), 3);

        // Move to item index 3 (beyond visible_rows=3)
        state.select_next();
        state.select_next();
        state.select_next();

        assert_eq!(state.selected_index(), 3);
        assert_eq!(state.scroll_offset(), 1);
    }

    #[test]
    fn visible_selected_is_relative() {
        let mut state = PickerState::new(sample_items(), 3);
        state.select_next();
        state.select_next();
        state.select_next(); // index 3, scroll_offset 1
        // visible_selected = 3 - 1 = 2
        assert_eq!(state.visible_selected(), 2);
    }

    #[test]
    fn prompt_line_format() {
        let mut state = PickerState::new(sample_items(), 10);
        assert_eq!(state.prompt_line(), "> ");

        state.set_query("hello");
        assert_eq!(state.prompt_line(), "> hello");
    }

    #[test]
    fn status_line_format() {
        let mut state = PickerState::new(sample_items(), 10);
        assert_eq!(state.status_line(), "6/6");

        state.set_query("main");
        let status = state.status_line();
        assert!(
            status.ends_with("/6"),
            "status should end with /6: {status}"
        );
    }

    #[test]
    fn set_items_refilters() {
        let mut state = PickerState::new(sample_items(), 10);
        state.set_query("src");
        let before = state.filtered_count();

        // Add more items that match "src"
        let mut items = sample_items();
        items.push(SourceItem::simple("src/extra.rs"));
        state.set_items(items);

        assert!(
            state.filtered_count() >= before,
            "adding matching items should not reduce count"
        );
    }

    #[test]
    fn empty_items() {
        let state = PickerState::new(Vec::new(), 10);
        assert_eq!(state.filtered_count(), 0);
        assert_eq!(state.total_count(), 0);
        assert!(state.visible_lines().is_empty());
        assert!(state.accept().is_none());
    }

    #[test]
    fn fuzzy_matching_ranks_best_first() {
        let mut state = PickerState::new(sample_items(), 10);
        state.set_query("pick");

        let lines = state.visible_lines();
        if !lines.is_empty() {
            assert!(
                lines[0].contains("picker"),
                "first result should contain 'picker': {lines:?}"
            );
        }
    }

    #[test]
    fn navigation_resets_on_filter_change() {
        let mut state = PickerState::new(sample_items(), 10);
        state.select_next();
        state.select_next();
        assert_eq!(state.selected_index(), 2);

        state.push_char('a');
        assert_eq!(
            state.selected_index(),
            0,
            "selection should reset after filtering"
        );
    }
}
