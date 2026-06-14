use std::collections::HashSet;
use std::io;
use std::process::Command;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::config::Config;
use crate::input::{self, InputSource, SharedStore};
use crate::matcher::{FuzzyMatcher, SharedMatchState};
use crate::ui;

pub struct AppState {
    pub query: String,
    pub cursor_pos: usize,
    pub scroll_offset: usize,
    pub selected: HashSet<usize>,
    pub multi_select: bool,
    pub preview_cmd: Option<String>,
    pub preview_content: String,
    pub store: SharedStore,
    pub match_state: SharedMatchState,
}

pub struct App {
    state: AppState,
    matcher: FuzzyMatcher,
    last_item_count: usize,
}

impl App {
    pub fn new(config: Config, source: InputSource) -> Self {
        let store = input::start_reader(source);
        let mut matcher = FuzzyMatcher::new(store.clone());
        let match_state = matcher.match_state();

        matcher.update_query(&config.initial_query);

        let state = AppState {
            query: config.initial_query,
            cursor_pos: 0,
            scroll_offset: 0,
            selected: HashSet::new(),
            multi_select: config.multi_select,
            preview_cmd: config.preview_cmd,
            preview_content: String::new(),
            store,
            match_state,
        };

        Self {
            state,
            matcher,
            last_item_count: 0,
        }
    }

    pub fn run(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::StderrLock<'_>>>,
    ) -> io::Result<Option<Vec<String>>> {
        loop {
            // Re-trigger match if new items arrived and query is empty
            let current_count = self.state.store.lock().len();
            if current_count != self.last_item_count {
                self.last_item_count = current_count;
                self.matcher.update_query(&self.state.query);
            }

            terminal.draw(|f| ui::draw(f, &self.state))?;

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
        match (key.modifiers, key.code) {
            (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
                return Some(None);
            }
            (_, KeyCode::Esc) => {
                return Some(None);
            }
            (_, KeyCode::Enter) => {
                let selections = self.get_selections();
                return Some(Some(selections));
            }
            (KeyModifiers::CONTROL, KeyCode::Char('n')) | (_, KeyCode::Down) => {
                self.move_cursor_down();
            }
            (KeyModifiers::CONTROL, KeyCode::Char('p')) | (_, KeyCode::Up) => {
                self.move_cursor_up();
            }
            (_, KeyCode::Tab) if self.state.multi_select => {
                self.toggle_selection();
                self.move_cursor_down();
            }
            (KeyModifiers::SHIFT, KeyCode::BackTab) if self.state.multi_select => {
                self.move_cursor_up();
                self.toggle_selection();
            }
            (KeyModifiers::CONTROL, KeyCode::Char('a')) if self.state.multi_select => {
                self.select_all();
            }
            (KeyModifiers::CONTROL, KeyCode::Char('d')) if self.state.multi_select => {
                self.deselect_all();
            }
            (_, KeyCode::Backspace) => {
                if !self.state.query.is_empty() {
                    self.state.query.pop();
                    self.on_query_change();
                }
            }
            (KeyModifiers::CONTROL, KeyCode::Char('u')) => {
                self.state.query.clear();
                self.on_query_change();
            }
            (KeyModifiers::CONTROL, KeyCode::Char('w')) => {
                let trimmed = self.state.query.trim_end();
                if let Some(pos) = trimmed.rfind(|c: char| c.is_whitespace()) {
                    self.state.query.truncate(pos);
                } else {
                    self.state.query.clear();
                }
                self.on_query_change();
            }
            (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char(c)) => {
                self.state.query.push(c);
                self.on_query_change();
            }
            (_, KeyCode::PageDown) => {
                for _ in 0..20 {
                    self.move_cursor_down();
                }
            }
            (_, KeyCode::PageUp) => {
                for _ in 0..20 {
                    self.move_cursor_up();
                }
            }
            (KeyModifiers::CONTROL, KeyCode::Char('e')) => {
                self.scroll_down(1);
            }
            (KeyModifiers::CONTROL, KeyCode::Char('y')) => {
                self.scroll_up(1);
            }
            _ => {}
        }
        None
    }

    fn handle_mouse(&mut self, mouse: MouseEvent) {
        match mouse.kind {
            MouseEventKind::ScrollDown => {
                self.move_cursor_down();
            }
            MouseEventKind::ScrollUp => {
                self.move_cursor_up();
            }
            _ => {}
        }
    }

    fn on_query_change(&mut self) {
        self.state.cursor_pos = 0;
        self.state.scroll_offset = 0;
        self.matcher.update_query(&self.state.query);
    }

    fn move_cursor_down(&mut self) {
        let total = self.state.match_state.lock().results.len();
        if total == 0 {
            return;
        }
        if self.state.cursor_pos + 1 < total {
            self.state.cursor_pos += 1;
            self.ensure_visible();
            self.update_preview();
        }
    }

    fn move_cursor_up(&mut self) {
        if self.state.cursor_pos > 0 {
            self.state.cursor_pos -= 1;
            self.ensure_visible();
            self.update_preview();
        }
    }

    fn scroll_down(&mut self, n: usize) {
        let total = self.state.match_state.lock().results.len();
        if self.state.scroll_offset + n < total {
            self.state.scroll_offset += n;
        }
    }

    fn scroll_up(&mut self, n: usize) {
        self.state.scroll_offset = self.state.scroll_offset.saturating_sub(n);
    }

    fn ensure_visible(&mut self) {
        let visible_height = 20usize; // Will be updated based on terminal size
        if self.state.cursor_pos < self.state.scroll_offset {
            self.state.scroll_offset = self.state.cursor_pos;
        } else if self.state.cursor_pos >= self.state.scroll_offset + visible_height {
            self.state.scroll_offset = self.state.cursor_pos - visible_height + 1;
        }
    }

    fn toggle_selection(&mut self) {
        let ms = self.state.match_state.lock();
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
        let ms = self.state.match_state.lock();
        for r in &ms.results {
            self.state.selected.insert(r.index);
        }
    }

    fn deselect_all(&mut self) {
        self.state.selected.clear();
    }

    fn get_selections(&self) -> Vec<String> {
        let ms = self.state.match_state.lock();
        let store = self.state.store.lock();

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

    fn update_preview(&mut self) {
        let cmd = match &self.state.preview_cmd {
            Some(c) => c.clone(),
            None => return,
        };

        let current_line = {
            let ms = self.state.match_state.lock();
            if let Some(result) = ms.results.get(self.state.cursor_pos) {
                let store = self.state.store.lock();
                store.get(result.index).map(|s| s.to_string())
            } else {
                None
            }
        };

        if let Some(line) = current_line {
            let cmd_expanded = cmd.replace("{}", &line);
            let output = Command::new("sh")
                .arg("-c")
                .arg(&cmd_expanded)
                .output();

            self.state.preview_content = match output {
                Ok(o) => String::from_utf8_lossy(&o.stdout).into_owned(),
                Err(e) => format!("Preview error: {}", e),
            };
        }
    }
}
