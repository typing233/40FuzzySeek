use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph};
use ratatui::Frame;

use crate::app::AppState;
use crate::matcher::MatchResult;

pub fn draw(f: &mut Frame, state: &AppState) {
    let area = f.area();

    let main_chunks = if state.preview_cmd.is_some() {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(100)])
            .split(area)
    };

    let left_area = main_chunks[0];
    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(1), Constraint::Length(1)])
        .split(left_area);

    draw_input(f, state, left_chunks[0]);
    draw_list(f, state, left_chunks[1]);
    draw_status(f, state, left_chunks[2]);

    if state.preview_cmd.is_some() && main_chunks.len() > 1 {
        draw_preview(f, state, main_chunks[1]);
    }
}

fn draw_input(f: &mut Frame, state: &AppState, area: Rect) {
    let input_block = Block::default()
        .borders(Borders::ALL)
        .title(" Query ");

    let cursor_suffix = if state.query.is_empty() { "" } else { "" };
    let input_text = format!("{}{}", state.query, cursor_suffix);
    let input = Paragraph::new(input_text)
        .block(input_block)
        .style(Style::default().fg(Color::White));

    f.render_widget(input, area);

    let cursor_x = area.x + 1 + state.query.len() as u16;
    let cursor_y = area.y + 1;
    f.set_cursor_position((cursor_x, cursor_y));
}

fn draw_list(f: &mut Frame, state: &AppState, area: Rect) {
    let visible_height = area.height as usize;
    let match_state = state.match_state.lock();
    let results = &match_state.results;

    let total = results.len();
    let start = state.scroll_offset;
    let end = (start + visible_height).min(total);

    let items: Vec<ListItem> = results[start..end]
        .iter()
        .enumerate()
        .map(|(vis_idx, m)| {
            let abs_idx = start + vis_idx;
            let is_cursor = abs_idx == state.cursor_pos;
            let is_selected = state.selected.contains(&m.index);

            let line_text = state
                .store
                .lock()
                .get(m.index)
                .unwrap_or("")
                .to_string();

            let spans = build_highlighted_spans_owned(&line_text, m, is_cursor, is_selected, state.multi_select);
            ListItem::new(Line::from(spans))
        })
        .collect();

    let list = List::new(items);
    f.render_widget(Clear, area);
    f.render_widget(list, area);
}

fn draw_status(f: &mut Frame, state: &AppState, area: Rect) {
    let match_state = state.match_state.lock();
    let store = state.store.lock();

    let status = if match_state.is_complete {
        format!(
            " {}/{} ",
            match_state.results.len(),
            store.len()
        )
    } else {
        format!(
            " {}/{}  (scanning...) ",
            match_state.results.len(),
            store.len()
        )
    };

    let multi_hint = if state.multi_select {
        format!(" [{}  selected] ", state.selected.len())
    } else {
        String::new()
    };

    let status_line = Paragraph::new(format!("{}{}", status, multi_hint))
        .style(Style::default().fg(Color::DarkGray));

    f.render_widget(status_line, area);
}

fn draw_preview(f: &mut Frame, state: &AppState, area: Rect) {
    let preview_block = Block::default()
        .borders(Borders::ALL)
        .title(" Preview ");

    let content = &state.preview_content;
    let paragraph = Paragraph::new(content.as_str())
        .block(preview_block)
        .style(Style::default().fg(Color::White));

    f.render_widget(paragraph, area);
}

fn build_highlighted_spans_owned(
    text: &str,
    match_result: &MatchResult,
    is_cursor: bool,
    is_selected: bool,
    multi_select: bool,
) -> Vec<Span<'static>> {
    let mut spans: Vec<Span<'static>> = Vec::new();

    let prefix = if multi_select {
        if is_selected { "● " } else { "  " }
    } else {
        if is_cursor { "> " } else { "  " }
    };

    let prefix_style = if is_cursor {
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else if is_selected {
        Style::default().fg(Color::Green)
    } else {
        Style::default()
    };

    spans.push(Span::styled(prefix.to_string(), prefix_style));

    let highlight_set: std::collections::HashSet<u32> =
        match_result.positions.iter().copied().collect();

    let base_style = if is_cursor {
        Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };

    let highlight_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);

    let mut char_idx: u32 = 0;
    let mut current_run = String::new();
    let mut current_is_highlight = false;

    for ch in text.chars() {
        let is_hl = highlight_set.contains(&char_idx);

        if is_hl != current_is_highlight && !current_run.is_empty() {
            let style = if current_is_highlight {
                highlight_style
            } else {
                base_style
            };
            spans.push(Span::styled(std::mem::take(&mut current_run), style));
        }

        current_run.push(ch);
        current_is_highlight = is_hl;
        char_idx += 1;
    }

    if !current_run.is_empty() {
        let style = if current_is_highlight {
            highlight_style
        } else {
            base_style
        };
        spans.push(Span::styled(current_run, style));
    }

    spans
}
