use std::collections::HashSet;
use std::io;
use std::time::Duration;

use crossterm::event::{self, Event, KeyEvent, MouseEvent, MouseEventKind};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::config::Config;
use crate::input::{self, InputSource, SharedStore};
use crate::keybind::{Action, KeyBindings};
use crate::matcher::{FuzzyMatcher, SharedMatchState};
use crate::preview::{PreviewRunner, SharedPreview};
use crate::ui;

pub struct AppState {
    pub query: String,
    pub cursor_pos: usize,
    pub scroll_offset: usize,
    pub selected: HashSet<usize>,
    pub multi_select: bool,
    pub store: SharedStore,
    pub match_state: SharedMatchState,
    preview_state: Option<SharedPreview>,
}

impl AppState {
    pub fn has_preview(&self) -> bool {
        self.preview_state.is_some()
    }

    pub fn get_preview_content(&self) -> (String, bool) {
        match &self.preview_state {
            Some(ps) => {
                let s = ps.lock();
                (s.content.clone(), s.loading)
            }
            None => (String::new(), false),
        }
    }
}

pub struct App {
    state: AppState,
    matcher: FuzzyMatcher,
    keybindings: KeyBindings,
    preview_runner: Option<PreviewRunner>,
    last_item_count: usize,
    last_terminal_height: u16,
}

impl App {
    pub fn new(config: Config, source: InputSource) -> Self {
        let store = input::start_reader(source)
            .expect("failed to start reader");
        let mut matcher = FuzzyMatcher::new(store.clone());
        let match_state = matcher.match_state();

        matcher.update_query(&config.initial_query);

        let preview_runner = config.preview_cmd.as_ref().map(|cmd| {
            PreviewRunner::new(cmd.clone())
        });
        let preview_state = preview_runner.as_ref().map(|r| r.state());

        let state = AppState {
            query: config.initial_query,
            cursor_pos: 0,
            scroll_offset: 0,
            selected: HashSet::new(),
            multi_select: config.multi_select,
            store,
            match_state,
            preview_state,
        };

        Self {
            state,
            matcher,
            keybindings: config.keybindings,
            preview_runner,
            last_item_count: 0,
            last_terminal_height: 0,
        }
    }

    pub fn run(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stderr>>,
    ) -> io::Result<Option<Vec<String>>> {
        loop {
            // Re-trigger match if new items arrived
            let current_count = self.state.store.read().len();
            if current_count != self.last_item_count {
                self.last_item_count = current_count;
                self.matcher.update_query(&self.state.query);
            }

            terminal.draw(|f| {
                let area = f.area();
                // Track terminal height for scroll calculations
                self.last_terminal_height = area.height;
                ui::draw(f, &self.state);
            })?;

            if event::poll(Duration::from_millis(50))? {
                match event::read()? {
                    Event::Key(key) => {
                        if let Some(action) = self.handle_key(key) {
                            return Ok(action);
                        }
                    }
                    Event::Mouse(mouse) => {
                        self.handle_mouse(mouse);
                    }
                    Event::Resize(_, _) => {}
                    _ => {}
                }
            }
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> Option<Option<Vec<String>>> {
        // First check configured keybindings
        if let Some(action) = self.keybindings.resolve(&key) {
            match action {
                Action::Confirm => {
                    return Some(Some(self.get_selections()));
                }
                Action::Cancel => {
                    return Some(None);
                }
                Action::CursorUp => { self.move_cursor_up(); }
                Action::CursorDown => { self.move_cursor_down(); }
                Action::PageUp => {
                    let h = self.visible_height();
                    for _ in 0..h {
                        self.move_cursor_up();
                    }
                }
                Action::PageDown => {
                    let h = self.visible_height();
                    for _ in 0..h {
                        self.move_cursor_down();
                    }
                }
                Action::ToggleSelect if self.state.multi_select => {
                    self.toggle_selection();
                    self.move_cursor_down();
                }
                Action::SelectAll if self.state.multi_select => {
                    self.select_all();
                }
                Action::DeselectAll if self.state.multi_select => {
                    self.deselect_all();
                }
                Action::DeleteChar => {
                    if !self.state.query.is_empty() {
                        self.state.query.pop();
                        self.on_query_change();
                    }
                }
                Action::ClearQuery => {
                    self.state.query.clear();
                    self.on_query_change();
                }
                Action::DeleteWord => {
                    let trimmed = self.state.query.trim_end().to_string();
                    if let Some(pos) = trimmed.rfind(|c: char| c.is_whitespace()) {
                        self.state.query.truncate(pos);
                    } else {
                        self.state.query.clear();
                    }
                    self.on_query_change();
                }
                Action::ScrollUp => { self.scroll_up(1); }
                Action::ScrollDown => { self.scroll_down(1); }
                _ => {}
            }
            return None;
        }

        // Unbound keys: treat printable chars as query input
        if let crossterm::event::KeyCode::Char(c) = key.code {
            if key.modifiers.is_empty() || key.modifiers == crossterm::event::KeyModifiers::SHIFT {
                self.state.query.push(c);
                self.on_query_change();
            }
        }

        None
    }

    fn handle_mouse(&mut self, mouse: MouseEvent) {
        match mouse.kind {
            MouseEventKind::ScrollDown => { self.move_cursor_down(); }
            MouseEventKind::ScrollUp => { self.move_cursor_up(); }
            _ => {}
        }
    }

    fn on_query_change(&mut self) {
        self.state.cursor_pos = 0;
        self.state.scroll_offset = 0;
        self.matcher.update_query(&self.state.query);
    }

    fn visible_height(&self) -> usize {
        // Approximate: total height minus input(3) and status(1)
        (self.last_terminal_height as usize).saturating_sub(4).max(1)
    }

    fn move_cursor_down(&mut self) {
        let total = self.state.match_state.read().results.len();
        if total == 0 { return; }
        if self.state.cursor_pos + 1 < total {
            self.state.cursor_pos += 1;
            self.ensure_visible();
            self.request_preview();
        }
    }

    fn move_cursor_up(&mut self) {
        if self.state.cursor_pos > 0 {
            self.state.cursor_pos -= 1;
            self.ensure_visible();
            self.request_preview();
        }
    }

    fn scroll_down(&mut self, n: usize) {
        let total = self.state.match_state.read().results.len();
        if self.state.scroll_offset + n < total {
            self.state.scroll_offset += n;
        }
    }

    fn scroll_up(&mut self, n: usize) {
        self.state.scroll_offset = self.state.scroll_offset.saturating_sub(n);
    }

    fn ensure_visible(&mut self) {
        let vh = self.visible_height();
        if self.state.cursor_pos < self.state.scroll_offset {
            self.state.scroll_offset = self.state.cursor_pos;
        } else if self.state.cursor_pos >= self.state.scroll_offset + vh {
            self.state.scroll_offset = self.state.cursor_pos - vh + 1;
        }
    }

    fn toggle_selection(&mut self) {
        let ms = self.state.match_state.read();
        if let Some(result) = ms.results.get(self.state.cursor_pos) {
            let idx = result.index;
            drop(ms);
            if self.state.selected.contains(&idx) {
                self.state.selected.remove(&idx);
            } else {
                self.state.selected.insert(idx);
            }
        }
    }

    fn select_all(&mut self) {
        let ms = self.state.match_state.read();
        for r in &ms.results {
            self.state.selected.insert(r.index);
        }
    }

    fn deselect_all(&mut self) {
        self.state.selected.clear();
    }

    fn get_selections(&self) -> Vec<String> {
        let ms = self.state.match_state.read();
        let store = self.state.store.read();

        if self.state.multi_select && !self.state.selected.is_empty() {
            let mut indices: Vec<usize> = self.state.selected.iter().copied().collect();
            indices.sort_unstable();
            indices
                .iter()
                .filter_map(|&i| store.get(i).map(|s| s.to_string()))
                .collect()
        } else if let Some(result) = ms.results.get(self.state.cursor_pos) {
            if let Some(line) = store.get(result.index) {
                vec![line.to_string()]
            } else {
                vec![]
            }
        } else {
            vec![]
        }
    }

    fn request_preview(&mut self) {
        let runner = match &self.preview_runner {
            Some(r) => r,
            None => return,
        };

        let current_line = {
            let ms = self.state.match_state.read();
            if let Some(result) = ms.results.get(self.state.cursor_pos) {
                let store = self.state.store.read();
                store.get(result.index).map(|s| s.to_string())
            } else {
                None
            }
        };

        match current_line {
            Some(line) => runner.request(&line),
            None => runner.clear(),
        }
    }
}
