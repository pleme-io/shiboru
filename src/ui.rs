//! Floating window UI for the picker.
//!
//! Creates a two-pane layout: a single-line prompt at the top and a results
//! list below. Handles key mappings within the picker buffer.

use crate::picker::{PickerAction, PickerState};
use crate::sources::Source;
use nvim_oxi::api;
use nvim_oxi::api::opts::SetKeymapOpts;
use nvim_oxi::api::types::{Mode, WindowConfig, WindowRelativeTo, WindowStyle};
use std::cell::RefCell;
use std::rc::Rc;

/// Height of the prompt window (always 1 line).
const PROMPT_HEIGHT: u32 = 1;

/// The picker UI manages the floating windows and drives the state machine.
pub struct PickerUi {
    prompt_buf: api::Buffer,
    prompt_win: Option<api::Window>,
    results_buf: api::Buffer,
    results_win: Option<api::Window>,
}

impl PickerUi {
    /// Create a new picker UI (allocates scratch buffers).
    pub fn new() -> nvim_oxi::Result<Self> {
        let prompt_buf = api::create_buf(false, true)?;
        let results_buf = api::create_buf(false, true)?;

        Ok(Self {
            prompt_buf,
            prompt_win: None,
            results_buf,
            results_win: None,
        })
    }

    /// Open the picker with the given source.
    pub fn open<S: Source + 'static>(&mut self, source: S) -> nvim_oxi::Result<()> {
        let ui_info = api::list_uis().into_iter().next();
        #[allow(clippy::cast_possible_truncation)]
        let (editor_width, editor_height) = match ui_info {
            Some(ref u) => (u.width as u32, u.height as u32),
            None => (80, 24),
        };

        // Layout: centered, 80% width, 60% height.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let width = ((f64::from(editor_width) * 0.8) as u32).max(20);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let total_height = ((f64::from(editor_height) * 0.6) as u32).max(5);
        let results_height = total_height.saturating_sub(PROMPT_HEIGHT + 1);

        let start_row = (editor_height.saturating_sub(total_height)) / 2;
        let start_col = (editor_width.saturating_sub(width)) / 2;

        // Open prompt window.
        let prompt_config = WindowConfig::builder()
            .relative(WindowRelativeTo::Editor)
            .row(f64::from(start_row))
            .col(f64::from(start_col))
            .width(width)
            .height(PROMPT_HEIGHT)
            .style(WindowStyle::Minimal)
            .focusable(true)
            .build();

        let prompt_win = api::open_win(&self.prompt_buf, true, &prompt_config)?;
        self.prompt_win = Some(prompt_win);

        // Open results window below the prompt.
        let results_config = WindowConfig::builder()
            .relative(WindowRelativeTo::Editor)
            .row(f64::from(start_row + PROMPT_HEIGHT + 1))
            .col(f64::from(start_col))
            .width(width)
            .height(results_height)
            .style(WindowStyle::Minimal)
            .focusable(false)
            .build();

        let results_win = api::open_win(&self.results_buf, false, &results_config)?;
        self.results_win = Some(results_win);

        // Collect initial items.
        let items = source.collect("");
        let state = PickerState::new(items, results_height as usize);

        // Render initial content.
        self.render_prompt(&state)?;
        self.render_results(&state)?;

        // Set up keymaps using shared state.
        // All buffers and state must be in RefCell since keymap callbacks are Fn, not FnMut.
        let shared_source: Rc<dyn Source> = Rc::new(source);
        let shared_state = Rc::new(RefCell::new(state));
        let shared_rbuf = Rc::new(RefCell::new(self.results_buf.clone()));
        let shared_pbuf = Rc::new(RefCell::new(self.prompt_buf.clone()));

        self.bind_keys(shared_state, shared_source, shared_rbuf, shared_pbuf)?;

        // Start in insert mode so the user can type immediately.
        api::command("startinsert!")?;

        Ok(())
    }

    /// Close both windows.
    pub fn close(&mut self) -> nvim_oxi::Result<()> {
        if let Some(win) = self.prompt_win.take() {
            if win.is_valid() {
                win.close(true)?;
            }
        }
        if let Some(win) = self.results_win.take() {
            if win.is_valid() {
                win.close(true)?;
            }
        }
        api::command("stopinsert")?;
        Ok(())
    }

    /// Render the prompt line into the prompt buffer.
    fn render_prompt(&mut self, state: &PickerState) -> nvim_oxi::Result<()> {
        let line = state.prompt_line();
        self.prompt_buf.set_lines(0..1, true, [line.as_str()])?;
        Ok(())
    }

    /// Render the results into the results buffer.
    fn render_results(&mut self, state: &PickerState) -> nvim_oxi::Result<()> {
        let lines = state.visible_lines();
        let status = state.status_line();

        let display_lines = Self::format_lines(&lines, state.visible_selected(), &status);

        let line_refs: Vec<&str> = display_lines.iter().map(String::as_str).collect();
        let line_count = line_refs.len();
        self.results_buf
            .set_lines(0..line_count, true, line_refs)?;

        Ok(())
    }

    /// Re-render results via the shared `RefCell` buffer (for use in callbacks).
    fn render_results_shared(
        state: &PickerState,
        rbuf: &Rc<RefCell<api::Buffer>>,
    ) {
        let lines = state.visible_lines();
        let vis_sel = state.visible_selected();
        let status = state.status_line();
        let display = Self::format_lines(&lines, vis_sel, &status);
        let refs: Vec<&str> = display.iter().map(String::as_str).collect();
        let _ = rbuf.borrow_mut().set_lines(0..refs.len(), true, refs);
    }

    /// Bind insert-mode keys on the prompt buffer to drive the picker.
    #[allow(clippy::too_many_arguments)]
    fn bind_keys(
        &mut self,
        state: Rc<RefCell<PickerState>>,
        source: Rc<dyn Source>,
        rbuf: Rc<RefCell<api::Buffer>>,
        pbuf: Rc<RefCell<api::Buffer>>,
    ) -> nvim_oxi::Result<()> {
        // -- Navigation: <C-j> = next --
        {
            let st = Rc::clone(&state);
            let rb = Rc::clone(&rbuf);
            let opts = SetKeymapOpts::builder()
                .silent(true)
                .nowait(true)
                .callback(move |_: ()| {
                    let mut s = st.borrow_mut();
                    s.select_next();
                    Self::render_results_shared(&s, &rb);
                })
                .build();
            self.prompt_buf
                .set_keymap(Mode::Insert, "<C-j>", "", &opts)?;
        }

        // -- Navigation: <C-k> = prev --
        {
            let st = Rc::clone(&state);
            let rb = Rc::clone(&rbuf);
            let opts = SetKeymapOpts::builder()
                .silent(true)
                .nowait(true)
                .callback(move |_: ()| {
                    let mut s = st.borrow_mut();
                    s.select_prev();
                    Self::render_results_shared(&s, &rb);
                })
                .build();
            self.prompt_buf
                .set_keymap(Mode::Insert, "<C-k>", "", &opts)?;
        }

        // -- Accept: <CR> --
        {
            let st = Rc::clone(&state);
            let pw = self.prompt_win.clone();
            let rw = self.results_win.clone();
            let opts = SetKeymapOpts::builder()
                .silent(true)
                .nowait(true)
                .callback(move |_: ()| {
                    let s = st.borrow();
                    let action = s.accept();
                    // Close windows.
                    if let Some(ref w) = pw {
                        if w.is_valid() {
                            let _ = w.clone().close(true);
                        }
                    }
                    if let Some(ref w) = rw {
                        if w.is_valid() {
                            let _ = w.clone().close(true);
                        }
                    }
                    let _ = api::command("stopinsert");

                    if let Some(PickerAction::Accept(target)) = action {
                        Self::execute_action(&target);
                    }
                })
                .build();
            self.prompt_buf
                .set_keymap(Mode::Insert, "<CR>", "", &opts)?;
        }

        // -- Cancel: <Esc> --
        {
            let pw = self.prompt_win.clone();
            let rw = self.results_win.clone();
            let opts = SetKeymapOpts::builder()
                .silent(true)
                .nowait(true)
                .callback(move |_: ()| {
                    if let Some(ref w) = pw {
                        if w.is_valid() {
                            let _ = w.clone().close(true);
                        }
                    }
                    if let Some(ref w) = rw {
                        if w.is_valid() {
                            let _ = w.clone().close(true);
                        }
                    }
                    let _ = api::command("stopinsert");
                })
                .build();
            self.prompt_buf
                .set_keymap(Mode::Insert, "<Esc>", "", &opts)?;
        }

        // -- Text input: TextChangedI autocmd on the prompt buffer --
        {
            let st = Rc::clone(&state);
            let src = Rc::clone(&source);
            let pb = Rc::clone(&pbuf);
            let rb = Rc::clone(&rbuf);

            let autocmd_opts = nvim_oxi::api::opts::CreateAutocmdOpts::builder()
                .buffer(self.prompt_buf.clone())
                .callback(move |_args| {
                    // Read current line from prompt buffer.
                    let line: String = pb
                        .borrow()
                        .get_lines(0..1, true)
                        .ok()
                        .and_then(|mut iter| iter.next())
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_default();

                    // Strip the "> " prefix to extract the raw query.
                    let query = line.strip_prefix("> ").unwrap_or(&line);

                    let mut s = st.borrow_mut();
                    let old_query = s.query().to_owned();
                    if query != old_query {
                        s.set_query(query);

                        // For live sources, re-collect items.
                        if src.is_live() {
                            let items = src.collect(query);
                            s.set_items(items);
                        }

                        // Re-render prompt (keeps "> " prefix consistent).
                        let prompt = s.prompt_line();
                        let _ = pb
                            .borrow_mut()
                            .set_lines(0..1, true, [prompt.as_str()]);

                        // Re-render results.
                        Self::render_results_shared(&s, &rb);
                    }

                    false // keep the autocmd
                })
                .build();

            api::create_autocmd(["TextChangedI"], &autocmd_opts)?;
        }

        Ok(())
    }

    /// Format result lines with selection indicator and status.
    fn format_lines(lines: &[&str], selected: usize, status: &str) -> Vec<String> {
        let mut display: Vec<String> = lines
            .iter()
            .enumerate()
            .map(|(i, line)| {
                if i == selected {
                    format!("> {line}")
                } else {
                    format!("  {line}")
                }
            })
            .collect();
        display.push(format!("  [{status}]"));
        display
    }

    /// Execute the selected item's action (open a file, jump to a line, etc.).
    fn execute_action(target: &str) {
        // If target contains `:line`, split and jump.
        if let Some((file, line_str)) = target.rsplit_once(':') {
            if let Ok(line) = line_str.parse::<u32>() {
                let cmd = format!("edit +{line} {file}");
                let _ = api::command(&cmd);
                return;
            }
        }

        // Simple file open.
        let cmd = format!("edit {target}");
        let _ = api::command(&cmd);
    }
}
