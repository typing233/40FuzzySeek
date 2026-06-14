pub mod builtin;

use ratatui::style::{Color, Style};
use serde::Deserialize;
use std::env;

#[derive(Clone, Debug)]
pub struct Theme {
    pub name: String,
    pub cursor: Style,
    pub selected: Style,
    pub highlight: Style,
    pub text: Style,
    pub text_bold: Style,
    pub status: Style,
    pub border: Style,
    pub input: Style,
    pub preview_text: Style,
    pub preview_border: Style,
    pub loading: Style,
    pub error: Style,
    pub multi_indicator: Style,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct ThemeConfig {
    pub base: Option<String>,
    pub cursor_fg: Option<String>,
    pub selected_fg: Option<String>,
    pub highlight_fg: Option<String>,
    pub text_fg: Option<String>,
    pub status_fg: Option<String>,
    pub border_fg: Option<String>,
    pub error_fg: Option<String>,
    pub bg: Option<String>,
}

impl Theme {
    pub fn resolve(cli_theme: Option<&str>, config_theme: Option<&ThemeConfig>) -> Self {
        let base_name = Self::determine_base(cli_theme, config_theme);
        let mut theme = match base_name.as_str() {
            "light" => builtin::light_theme(),
            _ => builtin::dark_theme(),
        };

        if let Some(cfg) = config_theme {
            theme.apply_overrides(cfg);
        }

        theme
    }

    fn determine_base(cli_theme: Option<&str>, config_theme: Option<&ThemeConfig>) -> String {
        if let Some(name) = cli_theme {
            return name.to_string();
        }
        if let Some(cfg) = config_theme {
            if let Some(ref base) = cfg.base {
                return base.clone();
            }
        }
        if let Ok(val) = env::var("FUZZYSEEK_THEME") {
            return val;
        }
        "dark".to_string()
    }

    fn apply_overrides(&mut self, cfg: &ThemeConfig) {
        if let Some(ref c) = cfg.cursor_fg {
            if let Some(color) = parse_color(c) {
                self.cursor = self.cursor.fg(color);
            }
        }
        if let Some(ref c) = cfg.selected_fg {
            if let Some(color) = parse_color(c) {
                self.selected = self.selected.fg(color);
                self.multi_indicator = self.multi_indicator.fg(color);
            }
        }
        if let Some(ref c) = cfg.highlight_fg {
            if let Some(color) = parse_color(c) {
                self.highlight = self.highlight.fg(color);
            }
        }
        if let Some(ref c) = cfg.text_fg {
            if let Some(color) = parse_color(c) {
                self.text = self.text.fg(color);
                self.text_bold = self.text_bold.fg(color);
                self.input = self.input.fg(color);
                self.preview_text = self.preview_text.fg(color);
            }
        }
        if let Some(ref c) = cfg.status_fg {
            if let Some(color) = parse_color(c) {
                self.status = self.status.fg(color);
            }
        }
        if let Some(ref c) = cfg.border_fg {
            if let Some(color) = parse_color(c) {
                self.border = self.border.fg(color);
                self.preview_border = self.preview_border.fg(color);
            }
        }
        if let Some(ref c) = cfg.error_fg {
            if let Some(color) = parse_color(c) {
                self.error = self.error.fg(color);
            }
        }
    }
}

pub fn parse_color(s: &str) -> Option<Color> {
    match s.to_lowercase().as_str() {
        "black" => Some(Color::Black),
        "red" => Some(Color::Red),
        "green" => Some(Color::Green),
        "yellow" => Some(Color::Yellow),
        "blue" => Some(Color::Blue),
        "magenta" | "purple" => Some(Color::Magenta),
        "cyan" => Some(Color::Cyan),
        "white" => Some(Color::White),
        "gray" | "grey" | "darkgray" | "dark_gray" => Some(Color::DarkGray),
        "lightred" | "light_red" => Some(Color::LightRed),
        "lightgreen" | "light_green" => Some(Color::LightGreen),
        "lightyellow" | "light_yellow" => Some(Color::LightYellow),
        "lightblue" | "light_blue" => Some(Color::LightBlue),
        "lightmagenta" | "light_magenta" => Some(Color::LightMagenta),
        "lightcyan" | "light_cyan" => Some(Color::LightCyan),
        s if s.starts_with('#') && s.len() == 7 => {
            let r = u8::from_str_radix(&s[1..3], 16).ok()?;
            let g = u8::from_str_radix(&s[3..5], 16).ok()?;
            let b = u8::from_str_radix(&s[5..7], 16).ok()?;
            Some(Color::Rgb(r, g, b))
        }
        s if s.parse::<u8>().is_ok() => {
            Some(Color::Indexed(s.parse().unwrap()))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_named_colors() {
        assert_eq!(parse_color("red"), Some(Color::Red));
        assert_eq!(parse_color("Blue"), Some(Color::Blue));
        assert_eq!(parse_color("CYAN"), Some(Color::Cyan));
        assert_eq!(parse_color("darkgray"), Some(Color::DarkGray));
    }

    #[test]
    fn test_parse_hex_color() {
        assert_eq!(parse_color("#ff0000"), Some(Color::Rgb(255, 0, 0)));
        assert_eq!(parse_color("#00FF00"), Some(Color::Rgb(0, 255, 0)));
        assert_eq!(parse_color("#1a2b3c"), Some(Color::Rgb(26, 43, 60)));
    }

    #[test]
    fn test_parse_indexed_color() {
        assert_eq!(parse_color("196"), Some(Color::Indexed(196)));
        assert_eq!(parse_color("0"), Some(Color::Indexed(0)));
    }

    #[test]
    fn test_parse_invalid_color() {
        assert_eq!(parse_color("notacolor"), None);
        assert_eq!(parse_color("#gg0000"), None);
        assert_eq!(parse_color("#fff"), None);
    }

    #[test]
    fn test_resolve_default_is_dark() {
        let theme = Theme::resolve(None, None);
        assert_eq!(theme.name, "dark");
    }

    #[test]
    fn test_resolve_cli_overrides_env() {
        let theme = Theme::resolve(Some("light"), None);
        assert_eq!(theme.name, "light");
    }

    #[test]
    fn test_resolve_config_override() {
        let cfg = ThemeConfig {
            base: Some("light".to_string()),
            ..Default::default()
        };
        let theme = Theme::resolve(None, Some(&cfg));
        assert_eq!(theme.name, "light");
    }

    #[test]
    fn test_apply_color_overrides() {
        let cfg = ThemeConfig {
            cursor_fg: Some("#ff0000".to_string()),
            highlight_fg: Some("green".to_string()),
            ..Default::default()
        };
        let theme = Theme::resolve(None, Some(&cfg));
        assert_eq!(theme.cursor.fg, Some(Color::Rgb(255, 0, 0)));
        assert_eq!(theme.highlight.fg, Some(Color::Green));
    }
}
