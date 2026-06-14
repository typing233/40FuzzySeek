pub mod parser;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::HashMap;

pub use parser::parse_key_bind;

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
    TogglePreview,
    RefreshPreview,
    CursorHome,
    CursorEnd,
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

    pub fn display(&self) -> String {
        let mut parts = Vec::new();
        if self.modifiers.contains(KeyModifiers::CONTROL) {
            parts.push("ctrl");
        }
        if self.modifiers.contains(KeyModifiers::ALT) {
            parts.push("alt");
        }
        if self.modifiers.contains(KeyModifiers::SHIFT) {
            parts.push("shift");
        }
        let key = match self.code {
            KeyCode::Enter => "enter".to_string(),
            KeyCode::Esc => "esc".to_string(),
            KeyCode::Tab => "tab".to_string(),
            KeyCode::BackTab => "btab".to_string(),
            KeyCode::Backspace => "bs".to_string(),
            KeyCode::Delete => "del".to_string(),
            KeyCode::Up => "up".to_string(),
            KeyCode::Down => "down".to_string(),
            KeyCode::Left => "left".to_string(),
            KeyCode::Right => "right".to_string(),
            KeyCode::Home => "home".to_string(),
            KeyCode::End => "end".to_string(),
            KeyCode::PageUp => "pgup".to_string(),
            KeyCode::PageDown => "pgdn".to_string(),
            KeyCode::Char(' ') => "space".to_string(),
            KeyCode::Char(c) => c.to_string(),
            _ => "?".to_string(),
        };
        parts.push(&key);
        // Can't use parts.join directly because of borrow, rebuild
        let mut result = String::new();
        if self.modifiers.contains(KeyModifiers::CONTROL) {
            result.push_str("ctrl-");
        }
        if self.modifiers.contains(KeyModifiers::ALT) {
            result.push_str("alt-");
        }
        if self.modifiers.contains(KeyModifiers::SHIFT) && self.code != KeyCode::BackTab {
            result.push_str("shift-");
        }
        result.push_str(&key);
        result
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindConflict {
    pub key: KeyBind,
    pub displaced_action: Action,
    pub new_action: Action,
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
        add(Action::ToggleSelect, KeyCode::Char(' '), KeyModifiers::CONTROL);
        add(Action::SelectAll, KeyCode::Char('a'), KeyModifiers::CONTROL);
        add(Action::DeselectAll, KeyCode::Char('d'), KeyModifiers::CONTROL);

        add(Action::DeleteChar, KeyCode::Backspace, KeyModifiers::NONE);
        add(Action::ClearQuery, KeyCode::Char('u'), KeyModifiers::CONTROL);
        add(Action::DeleteWord, KeyCode::Char('w'), KeyModifiers::CONTROL);

        add(Action::ScrollUp, KeyCode::Char('y'), KeyModifiers::CONTROL);
        add(Action::ScrollDown, KeyCode::Char('e'), KeyModifiers::CONTROL);

        add(Action::TogglePreview, KeyCode::Char('\\'), KeyModifiers::CONTROL);
        add(Action::RefreshPreview, KeyCode::Char('r'), KeyModifiers::CONTROL);
        add(Action::CursorHome, KeyCode::Home, KeyModifiers::NONE);
        add(Action::CursorEnd, KeyCode::End, KeyModifiers::NONE);

        Self { bindings }
    }

    pub fn bind(&mut self, action: Action, kb: KeyBind) {
        for binds in self.bindings.values_mut() {
            binds.retain(|b| b != &kb);
        }
        self.bindings.entry(action).or_default().push(kb);
    }

    pub fn bind_checked(&mut self, action: Action, kb: KeyBind) -> Vec<BindConflict> {
        let mut conflicts = Vec::new();
        for (&existing_action, binds) in self.bindings.iter() {
            if existing_action != action {
                for b in binds {
                    if b == &kb {
                        conflicts.push(BindConflict {
                            key: kb.clone(),
                            displaced_action: existing_action,
                            new_action: action,
                        });
                    }
                }
            }
        }
        self.bind(action, kb);
        conflicts
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

    pub fn get_bindings_for(&self, action: Action) -> &[KeyBind] {
        self.bindings.get(&action).map(|v| v.as_slice()).unwrap_or(&[])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_bindings_resolve() {
        let kb = KeyBindings::default_bindings();
        let event = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(kb.resolve(&event), Some(Action::Confirm));
    }

    #[test]
    fn test_ctrl_space_toggle_select() {
        let kb = KeyBindings::default_bindings();
        let event = KeyEvent::new(KeyCode::Char(' '), KeyModifiers::CONTROL);
        assert_eq!(kb.resolve(&event), Some(Action::ToggleSelect));
    }

    #[test]
    fn test_bind_removes_from_other_action() {
        let mut kb = KeyBindings::default_bindings();
        let enter_bind = KeyBind::new(KeyCode::Enter, KeyModifiers::NONE);
        kb.bind(Action::Cancel, enter_bind.clone());

        let event = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(kb.resolve(&event), Some(Action::Cancel));
    }

    #[test]
    fn test_bind_checked_reports_conflicts() {
        let mut kb = KeyBindings::default_bindings();
        let enter_bind = KeyBind::new(KeyCode::Enter, KeyModifiers::NONE);
        let conflicts = kb.bind_checked(Action::Cancel, enter_bind);

        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].displaced_action, Action::Confirm);
        assert_eq!(conflicts[0].new_action, Action::Cancel);
    }

    #[test]
    fn test_bind_checked_no_conflict() {
        let mut kb = KeyBindings::default_bindings();
        let new_bind = KeyBind::new(KeyCode::Char('z'), KeyModifiers::CONTROL);
        let conflicts = kb.bind_checked(Action::Confirm, new_bind);
        assert!(conflicts.is_empty());
    }

    #[test]
    fn test_key_bind_display() {
        let kb = KeyBind::new(KeyCode::Char('a'), KeyModifiers::CONTROL);
        assert_eq!(kb.display(), "ctrl-a");

        let kb = KeyBind::new(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(kb.display(), "enter");
    }

    #[test]
    fn test_home_end_actions() {
        let kb = KeyBindings::default_bindings();
        let event = KeyEvent::new(KeyCode::Home, KeyModifiers::NONE);
        assert_eq!(kb.resolve(&event), Some(Action::CursorHome));

        let event = KeyEvent::new(KeyCode::End, KeyModifiers::NONE);
        assert_eq!(kb.resolve(&event), Some(Action::CursorEnd));
    }
}
