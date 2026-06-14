use ratatui::style::{Color, Modifier, Style};
use super::Theme;

pub fn dark_theme() -> Theme {
    Theme {
        name: "dark".to_string(),
        cursor: Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        selected: Style::default().fg(Color::Green),
        highlight: Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        text: Style::default().fg(Color::White),
        text_bold: Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        status: Style::default().fg(Color::DarkGray),
        border: Style::default().fg(Color::DarkGray),
        input: Style::default().fg(Color::White),
        preview_text: Style::default().fg(Color::White),
        preview_border: Style::default().fg(Color::DarkGray),
        loading: Style::default().fg(Color::DarkGray),
        error: Style::default().fg(Color::Red),
        multi_indicator: Style::default().fg(Color::Green),
    }
}

pub fn light_theme() -> Theme {
    Theme {
        name: "light".to_string(),
        cursor: Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD),
        selected: Style::default().fg(Color::Magenta),
        highlight: Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        text: Style::default().fg(Color::Black),
        text_bold: Style::default().fg(Color::Black).add_modifier(Modifier::BOLD),
        status: Style::default().fg(Color::DarkGray),
        border: Style::default().fg(Color::DarkGray),
        input: Style::default().fg(Color::Black),
        preview_text: Style::default().fg(Color::Black),
        preview_border: Style::default().fg(Color::DarkGray),
        loading: Style::default().fg(Color::DarkGray),
        error: Style::default().fg(Color::Red),
        multi_indicator: Style::default().fg(Color::Magenta),
    }
}
