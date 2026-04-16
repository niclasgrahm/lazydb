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
    pub next_pane: KeyInput,
    pub prev_pane: KeyInput,
    pub show_help: KeyInput,
}

impl Default for GlobalKeysConfig {
    fn default() -> Self {
        Self {
            execute_query: KeyInput::Single("ctrl+e".into()),
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
    pub close: KeyInput,
    pub quit: KeyInput,
}

impl Default for ResultsKeysConfig {
    fn default() -> Self {
        Self {
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
                close: Action::from_config(&config.results.close),
                quit: Action::from_config(&config.results.quit),
            },
        }
    }
}
