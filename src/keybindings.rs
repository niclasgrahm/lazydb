use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::Deserialize;

// ---------------------------------------------------------------------------
// Config types (deserialized from TOML)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum KeyInput {
    Single(String),
    Multiple(Vec<String>),
}

impl KeyInput {
    fn to_vec(&self) -> Vec<String> {
        match self {
            KeyInput::Single(s) => vec![s.clone()],
            KeyInput::Multiple(v) => v.clone(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct KeybindingsConfig {
    pub global: GlobalKeysConfig,
    pub sidebar: SidebarKeysConfig,
    pub results: ResultsKeysConfig,
}

impl Default for KeybindingsConfig {
    fn default() -> Self {
        Self {
            global: GlobalKeysConfig::default(),
            sidebar: SidebarKeysConfig::default(),
            results: ResultsKeysConfig::default(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct GlobalKeysConfig {
    pub execute_query: KeyInput,
    pub format_query: KeyInput,
    pub next_pane: KeyInput,
    pub prev_pane: KeyInput,
    pub show_help: KeyInput,
}

impl Default for GlobalKeysConfig {
    fn default() -> Self {
        Self {
            execute_query: KeyInput::Single("ctrl+e".into()),
            format_query: KeyInput::Single("ctrl+f".into()),
            next_pane: KeyInput::Single("tab".into()),
            prev_pane: KeyInput::Single("shift+tab".into()),
            show_help: KeyInput::Single("?".into()),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct SidebarKeysConfig {
    pub navigate_up: KeyInput,
    pub navigate_down: KeyInput,
    pub expand: KeyInput,
    pub collapse: KeyInput,
    pub activate: KeyInput,
    pub preview: KeyInput,
    pub quit: KeyInput,
}

impl Default for SidebarKeysConfig {
    fn default() -> Self {
        Self {
            navigate_up: KeyInput::Multiple(vec!["k".into(), "up".into()]),
            navigate_down: KeyInput::Multiple(vec!["j".into(), "down".into()]),
            expand: KeyInput::Multiple(vec!["l".into(), "right".into()]),
            collapse: KeyInput::Multiple(vec!["h".into(), "left".into()]),
            activate: KeyInput::Single("enter".into()),
            preview: KeyInput::Single("s".into()),
            quit: KeyInput::Multiple(vec!["q".into(), "esc".into()]),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct ResultsKeysConfig {
    pub scroll_up: KeyInput,
    pub scroll_down: KeyInput,
    pub scroll_left: KeyInput,
    pub scroll_right: KeyInput,
    pub next_page: KeyInput,
    pub prev_page: KeyInput,
    pub close: KeyInput,
    pub quit: KeyInput,
}

impl Default for ResultsKeysConfig {
    fn default() -> Self {
        Self {
            scroll_up: KeyInput::Multiple(vec!["k".into(), "up".into()]),
            scroll_down: KeyInput::Multiple(vec!["j".into(), "down".into()]),
            scroll_left: KeyInput::Multiple(vec!["h".into(), "left".into()]),
            scroll_right: KeyInput::Multiple(vec!["l".into(), "right".into()]),
            next_page: KeyInput::Multiple(vec!["n".into(), "pagedown".into()]),
            prev_page: KeyInput::Multiple(vec!["p".into(), "pageup".into()]),
            close: KeyInput::Multiple(vec!["c".into(), "esc".into()]),
            quit: KeyInput::Single("q".into()),
        }
    }
}

// ---------------------------------------------------------------------------
// Resolved runtime types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct KeyBind {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

impl KeyBind {
    pub fn parse(s: &str) -> Self {
        let parts: Vec<&str> = s.split('+').collect();
        let mut modifiers = KeyModifiers::empty();

        let key_str = if parts.len() > 1 {
            for part in &parts[..parts.len() - 1] {
                match part.to_lowercase().as_str() {
                    "ctrl" => modifiers |= KeyModifiers::CONTROL,
                    "shift" => modifiers |= KeyModifiers::SHIFT,
                    "alt" => modifiers |= KeyModifiers::ALT,
                    _ => {}
                }
            }
            parts[parts.len() - 1]
        } else {
            parts[0]
        };

        let code = match key_str.to_lowercase().as_str() {
            "tab" if modifiers.contains(KeyModifiers::SHIFT) => {
                modifiers -= KeyModifiers::SHIFT;
                KeyCode::BackTab
            }
            "tab" => KeyCode::Tab,
            "enter" => KeyCode::Enter,
            "esc" | "escape" => KeyCode::Esc,
            "up" => KeyCode::Up,
            "down" => KeyCode::Down,
            "left" => KeyCode::Left,
            "right" => KeyCode::Right,
            "backspace" => KeyCode::Backspace,
            "delete" => KeyCode::Delete,
            "home" => KeyCode::Home,
            "end" => KeyCode::End,
            "pageup" => KeyCode::PageUp,
            "pagedown" => KeyCode::PageDown,
            "space" => KeyCode::Char(' '),
            s if s.len() == 1 => KeyCode::Char(s.chars().next().unwrap()),
            _ => KeyCode::Null,
        };

        Self { code, modifiers }
    }
}

/// A resolved action: one or more alternative key bindings + display string.
#[derive(Debug, Clone)]
pub struct Action {
    pub keys: Vec<KeyBind>,
    pub display: String,
}

impl Action {
    fn from_config(input: &KeyInput) -> Self {
        let strings = input.to_vec();
        let keys: Vec<KeyBind> = strings.iter().map(|s| KeyBind::parse(s)).collect();
        let display = strings.join("/");
        Self { keys, display }
    }

    pub fn matches(&self, key: &KeyEvent) -> bool {
        self.keys
            .iter()
            .any(|k| k.code == key.code && key.modifiers.contains(k.modifiers))
    }
}

// Resolved keybinding groups

pub struct GlobalKeys {
    pub execute_query: Action,
    pub format_query: Action,
    pub next_pane: Action,
    pub prev_pane: Action,
    pub show_help: Action,
}

pub struct SidebarKeys {
    pub navigate_up: Action,
    pub navigate_down: Action,
    pub expand: Action,
    pub collapse: Action,
    pub activate: Action,
    pub preview: Action,
    pub quit: Action,
}

pub struct ResultsKeys {
    pub scroll_up: Action,
    pub scroll_down: Action,
    pub scroll_left: Action,
    pub scroll_right: Action,
    pub next_page: Action,
    pub prev_page: Action,
    pub close: Action,
    pub quit: Action,
}

pub struct Keybindings {
    pub global: GlobalKeys,
    pub sidebar: SidebarKeys,
    pub results: ResultsKeys,
}

impl Keybindings {
    pub fn from_config(config: KeybindingsConfig) -> Self {
        Self {
            global: GlobalKeys {
                execute_query: Action::from_config(&config.global.execute_query),
                format_query: Action::from_config(&config.global.format_query),
                next_pane: Action::from_config(&config.global.next_pane),
                prev_pane: Action::from_config(&config.global.prev_pane),
                show_help: Action::from_config(&config.global.show_help),
            },
            sidebar: SidebarKeys {
                navigate_up: Action::from_config(&config.sidebar.navigate_up),
                navigate_down: Action::from_config(&config.sidebar.navigate_down),
                expand: Action::from_config(&config.sidebar.expand),
                collapse: Action::from_config(&config.sidebar.collapse),
                activate: Action::from_config(&config.sidebar.activate),
                preview: Action::from_config(&config.sidebar.preview),
                quit: Action::from_config(&config.sidebar.quit),
            },
            results: ResultsKeys {
                scroll_up: Action::from_config(&config.results.scroll_up),
                scroll_down: Action::from_config(&config.results.scroll_down),
                scroll_left: Action::from_config(&config.results.scroll_left),
                scroll_right: Action::from_config(&config.results.scroll_right),
                next_page: Action::from_config(&config.results.next_page),
                prev_page: Action::from_config(&config.results.prev_page),
                close: Action::from_config(&config.results.close),
                quit: Action::from_config(&config.results.quit),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_single_char() {
        let kb = KeyBind::parse("q");
        assert_eq!(kb.code, KeyCode::Char('q'));
        assert_eq!(kb.modifiers, KeyModifiers::empty());
    }

    #[test]
    fn parse_ctrl_modifier() {
        let kb = KeyBind::parse("ctrl+e");
        assert_eq!(kb.code, KeyCode::Char('e'));
        assert!(kb.modifiers.contains(KeyModifiers::CONTROL));
    }

    #[test]
    fn parse_shift_tab_becomes_backtab() {
        let kb = KeyBind::parse("shift+tab");
        assert_eq!(kb.code, KeyCode::BackTab);
        assert!(!kb.modifiers.contains(KeyModifiers::SHIFT));
    }

    #[test]
    fn parse_special_keys() {
        assert_eq!(KeyBind::parse("enter").code, KeyCode::Enter);
        assert_eq!(KeyBind::parse("esc").code, KeyCode::Esc);
        assert_eq!(KeyBind::parse("escape").code, KeyCode::Esc);
        assert_eq!(KeyBind::parse("up").code, KeyCode::Up);
        assert_eq!(KeyBind::parse("down").code, KeyCode::Down);
        assert_eq!(KeyBind::parse("left").code, KeyCode::Left);
        assert_eq!(KeyBind::parse("right").code, KeyCode::Right);
        assert_eq!(KeyBind::parse("pageup").code, KeyCode::PageUp);
        assert_eq!(KeyBind::parse("pagedown").code, KeyCode::PageDown);
        assert_eq!(KeyBind::parse("space").code, KeyCode::Char(' '));
        assert_eq!(KeyBind::parse("tab").code, KeyCode::Tab);
        assert_eq!(KeyBind::parse("backspace").code, KeyCode::Backspace);
    }

    #[test]
    fn parse_unknown_key() {
        assert_eq!(KeyBind::parse("nonexistent").code, KeyCode::Null);
    }

    #[test]
    fn action_matches_single() {
        let action = Action::from_config(&KeyInput::Single("q".into()));
        let event = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::empty());
        assert!(action.matches(&event));
    }

    #[test]
    fn action_matches_any_alternative() {
        let action = Action::from_config(&KeyInput::Multiple(vec!["k".into(), "up".into()]));
        let k_event = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::empty());
        let up_event = KeyEvent::new(KeyCode::Up, KeyModifiers::empty());
        assert!(action.matches(&k_event));
        assert!(action.matches(&up_event));
    }

    #[test]
    fn action_no_match() {
        let action = Action::from_config(&KeyInput::Single("q".into()));
        let event = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::empty());
        assert!(!action.matches(&event));
    }

    #[test]
    fn action_from_config_single() {
        let action = Action::from_config(&KeyInput::Single("ctrl+e".into()));
        assert_eq!(action.keys.len(), 1);
        assert_eq!(action.display, "ctrl+e");
    }

    #[test]
    fn action_from_config_multiple() {
        let action = Action::from_config(&KeyInput::Multiple(vec!["k".into(), "up".into()]));
        assert_eq!(action.keys.len(), 2);
        assert_eq!(action.display, "k/up");
    }

    #[test]
    fn default_keybindings_resolve() {
        let kb = Keybindings::from_config(KeybindingsConfig::default());
        // Verify all actions have at least one key
        assert!(!kb.global.execute_query.keys.is_empty());
        assert!(!kb.global.next_pane.keys.is_empty());
        assert!(!kb.sidebar.navigate_up.keys.is_empty());
        assert!(!kb.sidebar.quit.keys.is_empty());
        assert!(!kb.results.scroll_down.keys.is_empty());
        assert!(!kb.results.quit.keys.is_empty());
    }
}
