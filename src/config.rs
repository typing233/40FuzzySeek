use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use serde::Deserialize;

use crate::input::{InputSource, ProviderConfig};
use crate::keybind::{Action, KeyBindings, parse_key_bind, BindConflict};
use crate::preview::resolver::PreviewRule;
use crate::preview::PreviewConfig;
use crate::theme::{Theme, ThemeConfig};
use crate::Cli;

#[derive(Deserialize, Default)]
struct ConfigFile {
    #[serde(default)]
    keys: HashMap<String, String>,
    #[serde(default)]
    theme: Option<ThemeConfig>,
    #[serde(default)]
    preview: Option<PreviewFileConfig>,
    #[serde(default)]
    input: Option<InputFileConfig>,
}

#[derive(Deserialize, Default)]
struct PreviewFileConfig {
    timeout_ms: Option<u64>,
    cache_size: Option<usize>,
    max_output_bytes: Option<usize>,
    rules: Option<Vec<PreviewRule>>,
}

#[derive(Deserialize, Default)]
struct InputFileConfig {
    max_items: Option<usize>,
    max_line_length: Option<usize>,
}

pub struct Config {
    pub multi_select: bool,
    pub preview_cmd: Option<String>,
    pub preview_config: PreviewConfig,
    pub preview_rules: Vec<PreviewRule>,
    pub initial_query: String,
    pub height: u16,
    pub keybindings: KeyBindings,
    pub theme: Theme,
    pub provider_config: ProviderConfig,
    pub delimiter: String,
}

impl Config {
    pub fn from_cli(cli: &Cli) -> Result<(Self, Vec<BindConflict>), String> {
        let mut keybindings = KeyBindings::default_bindings();
        let mut all_conflicts = Vec::new();

        let mut file_theme: Option<ThemeConfig> = None;
        let mut preview_config = PreviewConfig::default();
        let mut preview_rules = Vec::new();
        let mut provider_config = ProviderConfig::default();

        // Load from config file
        if let Some(config_path) = Self::config_file_path() {
            if config_path.exists() {
                let content = fs::read_to_string(&config_path)
                    .map_err(|e| format!("cannot read config file: {}", e))?;
                let file_cfg: ConfigFile = toml::from_str(&content)
                    .map_err(|e| format!("invalid config file: {}", e))?;

                for (action_str, key_str) in &file_cfg.keys {
                    let action = Self::parse_action(action_str)?;
                    let kb = parse_key_bind(key_str)?;
                    let conflicts = keybindings.bind_checked(action, kb);
                    all_conflicts.extend(conflicts);
                }

                file_theme = file_cfg.theme;

                if let Some(preview_file) = file_cfg.preview {
                    if let Some(ms) = preview_file.timeout_ms {
                        preview_config.timeout = Duration::from_millis(ms);
                    }
                    if let Some(size) = preview_file.cache_size {
                        preview_config.cache_capacity = size;
                    }
                    if let Some(max) = preview_file.max_output_bytes {
                        preview_config.max_output_bytes = max;
                    }
                    if let Some(rules) = preview_file.rules {
                        preview_rules = rules;
                    }
                }

                if let Some(input_file) = file_cfg.input {
                    if let Some(max) = input_file.max_items {
                        provider_config.max_items = Some(max);
                    }
                    if let Some(len) = input_file.max_line_length {
                        provider_config.max_line_length = len;
                    }
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
                let conflicts = keybindings.bind_checked(action, kb);
                all_conflicts.extend(conflicts);
            }
        }

        // CLI overrides for preview timeout
        if cli.preview_timeout != 5000 {
            preview_config.timeout = Duration::from_millis(cli.preview_timeout);
        }

        // CLI override for max_items
        if let Some(max) = cli.max_items {
            provider_config.max_items = Some(max);
        }

        // Resolve theme: CLI > config > env > dark
        let theme = Theme::resolve(cli.theme.as_deref(), file_theme.as_ref());

        Ok((Self {
            multi_select: cli.multi,
            preview_cmd: cli.preview.clone(),
            preview_config,
            preview_rules,
            initial_query: cli.query.clone(),
            height: cli.height,
            keybindings,
            theme,
            provider_config,
            delimiter: cli.delimiter.clone(),
        }, all_conflicts))
    }

    pub fn determine_input_source(cli: &Cli) -> Result<InputSource, String> {
        if let Some(ref cmd) = cli.cmd {
            return Ok(InputSource::Command { cmd: cmd.clone(), shell: None });
        }
        if let Some(ref path) = cli.file {
            if !std::path::Path::new(path).exists() {
                return Err(format!("cannot open '{}': No such file or directory", path));
            }
            return Ok(InputSource::File(path.clone()));
        }
        if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
            return Ok(InputSource::Stdin);
        }
        Err("no input (provide --file, --cmd, or pipe data via stdin)".to_string())
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
            "toggle-preview" | "toggle_preview" => Ok(Action::TogglePreview),
            "refresh-preview" | "refresh_preview" => Ok(Action::RefreshPreview),
            "home" | "cursor-home" => Ok(Action::CursorHome),
            "end" | "cursor-end" => Ok(Action::CursorEnd),
            _ => Err(format!("unknown action '{}'", s)),
        }
    }
}
