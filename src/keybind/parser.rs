use crossterm::event::KeyCode;
use super::KeyBind;
use crossterm::event::KeyModifiers;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_char() {
        let kb = parse_key_bind("a").unwrap();
        assert_eq!(kb.code, KeyCode::Char('a'));
        assert_eq!(kb.modifiers, KeyModifiers::NONE);
    }

    #[test]
    fn test_parse_ctrl_modifier() {
        let kb = parse_key_bind("ctrl-c").unwrap();
        assert_eq!(kb.code, KeyCode::Char('c'));
        assert_eq!(kb.modifiers, KeyModifiers::CONTROL);
    }

    #[test]
    fn test_parse_alt_modifier() {
        let kb = parse_key_bind("alt-x").unwrap();
        assert_eq!(kb.code, KeyCode::Char('x'));
        assert_eq!(kb.modifiers, KeyModifiers::ALT);
    }

    #[test]
    fn test_parse_special_keys() {
        assert_eq!(parse_key_bind("enter").unwrap().code, KeyCode::Enter);
        assert_eq!(parse_key_bind("esc").unwrap().code, KeyCode::Esc);
        assert_eq!(parse_key_bind("tab").unwrap().code, KeyCode::Tab);
        assert_eq!(parse_key_bind("space").unwrap().code, KeyCode::Char(' '));
        assert_eq!(parse_key_bind("pgup").unwrap().code, KeyCode::PageUp);
    }

    #[test]
    fn test_parse_backtab() {
        let kb = parse_key_bind("btab").unwrap();
        assert_eq!(kb.code, KeyCode::BackTab);
        assert!(kb.modifiers.contains(KeyModifiers::SHIFT));
    }

    #[test]
    fn test_parse_ctrl_space() {
        let kb = parse_key_bind("ctrl-space").unwrap();
        assert_eq!(kb.code, KeyCode::Char(' '));
        assert_eq!(kb.modifiers, KeyModifiers::CONTROL);
    }

    #[test]
    fn test_parse_error_unknown_modifier() {
        assert!(parse_key_bind("foo-a").is_err());
    }

    #[test]
    fn test_parse_error_unknown_key() {
        assert!(parse_key_bind("ctrl-unknown").is_err());
    }

    #[test]
    fn test_parse_short_modifier_forms() {
        let kb = parse_key_bind("c-a").unwrap();
        assert_eq!(kb.code, KeyCode::Char('a'));
        assert_eq!(kb.modifiers, KeyModifiers::CONTROL);

        let kb = parse_key_bind("a-b").unwrap();
        assert_eq!(kb.code, KeyCode::Char('b'));
        assert_eq!(kb.modifiers, KeyModifiers::ALT);
    }
}
