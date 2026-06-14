use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use serde::Deserialize;

use crate::keybind::{Action, KeyBindings, parse_key_bind};
use crate::Cli;

#[derive(Deserialize, Default)]
struct ConfigFile {
    #[serde(default)]
    keys: HashMap<String, String>,
}

pub struct Config {
    pub multi_select: bool,
    pub preview_cmd: Option<String>,
    pub initial_query: String,
    pub height: u16,
    pub keybindings: KeyBindings,
}

impl Config {
    pub fn from_cli(cli: &Cli) -> Result<Self, String> {
        let mut keybindings = KeyBindings::default_bindings();

        // Load from config file if it exists
        if let Some(config_path) = Self::config_file_path() {
            if config_path.exists() {
                let content = fs::read_to_string(&config_path)
                    .map_err(|e| format!("cannot read config file: {}", e))?;
                let file_cfg: ConfigFile = toml::from_str(&content)
                    .map_err(|e| format!("invalid config file: {}", e))?;

                for (action_str, key_str) in &file_cfg.keys {
                    let action = Self::parse_action(action_str)?;
                    let kb = parse_key_bind(key_str)?;
                    keybindings.bind(action, kb);
                }
            }
        }

        // Override with --bind flags
        for bind_str in &cli.bind {
            for pair in bind_str.split(',') {
                let parts: Vec<&str> = pair.splitn(2, ':').collect();
                if parts.len() != 2 {
                    return Err(format!("invalid --bind format '{}', expected action:key", pair));
                }
                let action = Self::parse_action(parts[0])?;
                let kb = parse_key_bind(parts[1])?;
                keybindings.bind(action, kb);
            }
        }

        Ok(Self {
            multi_select: cli.multi,
            preview_cmd: cli.preview.clone(),
            initial_query: cli.query.clone(),
            height: cli.height,
            keybindings,
        })
    }

    fn config_file_path() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("fuzzyseek").join("config.toml"))
    }

    fn parse_action(s: &str) -> Result<Action, String> {
        match s {
            "confirm" => Ok(Action::Confirm),
            "cancel" | "abort" => Ok(Action::Cancel),
            "up" => Ok(Action::CursorUp),
            "down" => Ok(Action::CursorDown),
            "page-up" | "page_up" => Ok(Action::PageUp),
            "page-down" | "page_down" => Ok(Action::PageDown),
            "toggle" | "toggle-select" => Ok(Action::ToggleSelect),
            "select-all" | "select_all" => Ok(Action::SelectAll),
            "deselect-all" | "deselect_all" => Ok(Action::DeselectAll),
            "delete-char" | "backspace" => Ok(Action::DeleteChar),
            "clear-query" | "clear_query" => Ok(Action::ClearQuery),
            "delete-word" | "delete_word" => Ok(Action::DeleteWord),
            "scroll-up" => Ok(Action::ScrollUp),
            "scroll-down" => Ok(Action::ScrollDown),
            _ => Err(format!("unknown action '{}'", s)),
        }
    }
}
