use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    Confirm,
    Cancel,
    CursorUp,
    CursorDown,
    PageUp,
    PageDown,
    ToggleSelect,
    SelectAll,
    DeselectAll,
    DeleteChar,
    ClearQuery,
    DeleteWord,
    ScrollUp,
    ScrollDown,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyBind {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

impl KeyBind {
    pub fn new(code: KeyCode, modifiers: KeyModifiers) -> Self {
        Self { code, modifiers }
    }

    pub fn matches(&self, event: &KeyEvent) -> bool {
        self.code == event.code && event.modifiers.contains(self.modifiers)
    }
}

pub struct KeyBindings {
    bindings: HashMap<Action, Vec<KeyBind>>,
}

impl KeyBindings {
    pub fn default_bindings() -> Self {
        let mut bindings: HashMap<Action, Vec<KeyBind>> = HashMap::new();

        let mut add = |action: Action, code: KeyCode, mods: KeyModifiers| {
            bindings.entry(action).or_default().push(KeyBind::new(code, mods));
        };

        add(Action::Confirm, KeyCode::Enter, KeyModifiers::NONE);
        add(Action::Cancel, KeyCode::Esc, KeyModifiers::NONE);
        add(Action::Cancel, KeyCode::Char('c'), KeyModifiers::CONTROL);

        add(Action::CursorUp, KeyCode::Up, KeyModifiers::NONE);
        add(Action::CursorUp, KeyCode::Char('p'), KeyModifiers::CONTROL);
        add(Action::CursorDown, KeyCode::Down, KeyModifiers::NONE);
        add(Action::CursorDown, KeyCode::Char('n'), KeyModifiers::CONTROL);

        add(Action::PageUp, KeyCode::PageUp, KeyModifiers::NONE);
        add(Action::PageDown, KeyCode::PageDown, KeyModifiers::NONE);

        add(Action::ToggleSelect, KeyCode::Tab, KeyModifiers::NONE);
        add(Action::SelectAll, KeyCode::Char('a'), KeyModifiers::CONTROL);
        add(Action::DeselectAll, KeyCode::Char('d'), KeyModifiers::CONTROL);

        add(Action::DeleteChar, KeyCode::Backspace, KeyModifiers::NONE);
        add(Action::ClearQuery, KeyCode::Char('u'), KeyModifiers::CONTROL);
        add(Action::DeleteWord, KeyCode::Char('w'), KeyModifiers::CONTROL);

        add(Action::ScrollUp, KeyCode::Char('y'), KeyModifiers::CONTROL);
        add(Action::ScrollDown, KeyCode::Char('e'), KeyModifiers::CONTROL);

        Self { bindings }
    }

    pub fn bind(&mut self, action: Action, kb: KeyBind) {
        // Remove this key from any other action first
        for binds in self.bindings.values_mut() {
            binds.retain(|b| b != &kb);
        }
        self.bindings.entry(action).or_default().push(kb);
    }

    pub fn resolve(&self, event: &KeyEvent) -> Option<Action> {
        for (&action, binds) in &self.bindings {
            for kb in binds {
                if kb.matches(event) {
                    return Some(action);
                }
            }
        }
        None
    }
}

pub fn parse_key_bind(s: &str) -> Result<KeyBind, String> {
    let s = s.trim().to_lowercase();
    let parts: Vec<&str> = s.split('-').collect();

    let mut modifiers = KeyModifiers::NONE;
    let key_part;

    if parts.len() == 1 {
        key_part = parts[0];
    } else {
        for &part in &parts[..parts.len() - 1] {
            match part {
                "ctrl" | "c" => modifiers |= KeyModifiers::CONTROL,
                "alt" | "a" => modifiers |= KeyModifiers::ALT,
                "shift" | "s" => modifiers |= KeyModifiers::SHIFT,
                _ => return Err(format!("unknown modifier '{}' in '{}'", part, s)),
            }
        }
        key_part = parts[parts.len() - 1];
    }

    let code = match key_part {
        "enter" | "return" | "cr" => KeyCode::Enter,
        "esc" | "escape" => KeyCode::Esc,
        "tab" => KeyCode::Tab,
        "btab" | "backtab" => {
            modifiers |= KeyModifiers::SHIFT;
            KeyCode::BackTab
        }
        "bs" | "backspace" => KeyCode::Backspace,
        "del" | "delete" => KeyCode::Delete,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pgup" | "pageup" => KeyCode::PageUp,
        "pgdn" | "pagedown" => KeyCode::PageDown,
        "space" => KeyCode::Char(' '),
        c if c.len() == 1 => KeyCode::Char(c.chars().next().unwrap()),
        _ => return Err(format!("unknown key '{}' in '{}'", key_part, s)),
    };

    Ok(KeyBind::new(code, modifiers))
}
