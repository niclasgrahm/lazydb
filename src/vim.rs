use std::fmt;

use tui_textarea::{CursorMove, Input, Key, Scrolling, TextArea};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
    Visual,
    Operator(char),
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Normal => write!(f, "NORMAL"),
            Self::Insert => write!(f, "INSERT"),
            Self::Visual => write!(f, "VISUAL"),
            Self::Operator(c) => write!(f, "OPERATOR({c})"),
        }
    }
}

pub enum Transition {
    Nop,
    Mode(Mode),
    Pending(Input),
}

pub struct Vim {
    pub mode: Mode,
    pending: Input,
}

impl Vim {
    pub fn new(mode: Mode) -> Self {
        Self {
            mode,
            pending: Input::default(),
        }
    }

    pub fn with_pending(self, pending: Input) -> Self {
        Self { pending, ..self }
    }

    pub fn transition(&self, input: Input, textarea: &mut TextArea<'_>) -> Transition {
        if input.key == Key::Null {
            return Transition::Nop;
        }

        match self.mode {
            Mode::Normal | Mode::Visual | Mode::Operator(_) => {
                self.handle_normal_visual_operator(input, textarea)
            }
            Mode::Insert => Self::handle_insert(input, textarea),
        }
    }

    fn handle_insert(input: Input, textarea: &mut TextArea<'_>) -> Transition {
        match input {
            Input { key: Key::Esc, .. }
            | Input {
                key: Key::Char('c'),
                ctrl: true,
                ..
            } => Transition::Mode(Mode::Normal),
            input => {
                textarea.input(input);
                Transition::Mode(Mode::Insert)
            }
        }
    }

    fn handle_normal_visual_operator(
        &self,
        input: Input,
        textarea: &mut TextArea<'_>,
    ) -> Transition {
        match input {
            // Movement
            Input { key: Key::Char('h'), .. } => textarea.move_cursor(CursorMove::Back),
            Input { key: Key::Char('j'), .. } => textarea.move_cursor(CursorMove::Down),
            Input { key: Key::Char('k'), .. } => textarea.move_cursor(CursorMove::Up),
            Input { key: Key::Char('l'), .. } => textarea.move_cursor(CursorMove::Forward),
            Input { key: Key::Char('w'), .. } => textarea.move_cursor(CursorMove::WordForward),
            Input { key: Key::Char('e'), ctrl: false, .. } => {
                textarea.move_cursor(CursorMove::WordEnd);
                if matches!(self.mode, Mode::Operator(_)) {
                    textarea.move_cursor(CursorMove::Forward);
                }
            }
            Input { key: Key::Char('b'), ctrl: false, .. } => textarea.move_cursor(CursorMove::WordBack),
            Input { key: Key::Char('^' | '0'), .. } => textarea.move_cursor(CursorMove::Head),
            Input { key: Key::Char('$'), .. } => textarea.move_cursor(CursorMove::End),

            // Editing
            Input { key: Key::Char('D'), .. } => {
                textarea.delete_line_by_end();
                return Transition::Mode(Mode::Normal);
            }
            Input { key: Key::Char('C'), .. } => {
                textarea.delete_line_by_end();
                textarea.cancel_selection();
                return Transition::Mode(Mode::Insert);
            }
            Input { key: Key::Char('p'), .. } => {
                textarea.paste();
                return Transition::Mode(Mode::Normal);
            }
            Input { key: Key::Char('u'), ctrl: false, .. } => {
                textarea.undo();
                return Transition::Mode(Mode::Normal);
            }
            Input { key: Key::Char('r'), ctrl: true, .. } => {
                textarea.redo();
                return Transition::Mode(Mode::Normal);
            }
            Input { key: Key::Char('x'), .. } => {
                textarea.delete_next_char();
                return Transition::Mode(Mode::Normal);
            }

            // Insert mode entries
            Input { key: Key::Char('i'), .. } => {
                textarea.cancel_selection();
                return Transition::Mode(Mode::Insert);
            }
            Input { key: Key::Char('a'), .. } => {
                textarea.cancel_selection();
                textarea.move_cursor(CursorMove::Forward);
                return Transition::Mode(Mode::Insert);
            }
            Input { key: Key::Char('A'), .. } => {
                textarea.cancel_selection();
                textarea.move_cursor(CursorMove::End);
                return Transition::Mode(Mode::Insert);
            }
            Input { key: Key::Char('o'), .. } => {
                textarea.move_cursor(CursorMove::End);
                textarea.insert_newline();
                return Transition::Mode(Mode::Insert);
            }
            Input { key: Key::Char('O'), .. } => {
                textarea.move_cursor(CursorMove::Head);
                textarea.insert_newline();
                textarea.move_cursor(CursorMove::Up);
                return Transition::Mode(Mode::Insert);
            }
            Input { key: Key::Char('I'), .. } => {
                textarea.cancel_selection();
                textarea.move_cursor(CursorMove::Head);
                return Transition::Mode(Mode::Insert);
            }

            // Scrolling
            Input { key: Key::Char('e'), ctrl: true, .. } => textarea.scroll((1, 0)),
            Input { key: Key::Char('y'), ctrl: true, .. } => textarea.scroll((-1, 0)),
            Input { key: Key::Char('d'), ctrl: true, .. } => textarea.scroll(Scrolling::HalfPageDown),
            Input { key: Key::Char('u'), ctrl: true, .. } => textarea.scroll(Scrolling::HalfPageUp),
            Input { key: Key::Char('f'), ctrl: true, .. } => textarea.scroll(Scrolling::PageDown),
            Input { key: Key::Char('b'), ctrl: true, .. } => textarea.scroll(Scrolling::PageUp),

            // Visual mode
            Input { key: Key::Char('v'), ctrl: false, .. } if self.mode == Mode::Normal => {
                textarea.start_selection();
                return Transition::Mode(Mode::Visual);
            }
            Input { key: Key::Char('V'), ctrl: false, .. } if self.mode == Mode::Normal => {
                textarea.move_cursor(CursorMove::Head);
                textarea.start_selection();
                textarea.move_cursor(CursorMove::End);
                return Transition::Mode(Mode::Visual);
            }
            Input { key: Key::Esc, .. } | Input { key: Key::Char('v'), ctrl: false, .. }
                if self.mode == Mode::Visual =>
            {
                textarea.cancel_selection();
                return Transition::Mode(Mode::Normal);
            }

            // gg / G
            Input { key: Key::Char('g'), ctrl: false, .. }
                if matches!(self.pending, Input { key: Key::Char('g'), ctrl: false, .. }) =>
            {
                textarea.move_cursor(CursorMove::Top);
            }
            Input { key: Key::Char('G'), ctrl: false, .. } => {
                textarea.move_cursor(CursorMove::Bottom);
            }

            // Operator-pending: dd, yy, cc
            Input { key: Key::Char(c), ctrl: false, .. } if self.mode == Mode::Operator(c) => {
                textarea.move_cursor(CursorMove::Head);
                textarea.start_selection();
                let cursor = textarea.cursor();
                textarea.move_cursor(CursorMove::Down);
                if cursor == textarea.cursor() {
                    textarea.move_cursor(CursorMove::End);
                }
            }

            // Start operator
            Input { key: Key::Char(op @ ('y' | 'd' | 'c')), ctrl: false, .. }
                if self.mode == Mode::Normal =>
            {
                textarea.start_selection();
                return Transition::Mode(Mode::Operator(op));
            }

            // Visual mode actions
            Input { key: Key::Char('y'), ctrl: false, .. } if self.mode == Mode::Visual => {
                textarea.move_cursor(CursorMove::Forward);
                textarea.copy();
                return Transition::Mode(Mode::Normal);
            }
            Input { key: Key::Char('d'), ctrl: false, .. } if self.mode == Mode::Visual => {
                textarea.move_cursor(CursorMove::Forward);
                textarea.cut();
                return Transition::Mode(Mode::Normal);
            }
            Input { key: Key::Char('c'), ctrl: false, .. } if self.mode == Mode::Visual => {
                textarea.move_cursor(CursorMove::Forward);
                textarea.cut();
                return Transition::Mode(Mode::Insert);
            }

            input => return Transition::Pending(input),
        }

        // Resolve pending operator
        match self.mode {
            Mode::Operator('y') => {
                textarea.copy();
                Transition::Mode(Mode::Normal)
            }
            Mode::Operator('d') => {
                textarea.cut();
                Transition::Mode(Mode::Normal)
            }
            Mode::Operator('c') => {
                textarea.cut();
                Transition::Mode(Mode::Insert)
            }
            _ => Transition::Nop,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key_input(c: char) -> Input {
        Input {
            key: Key::Char(c),
            ctrl: false,
            alt: false,
            shift: false,
        }
    }

    fn ctrl_input(c: char) -> Input {
        Input {
            key: Key::Char(c),
            ctrl: true,
            alt: false,
            shift: false,
        }
    }

    fn esc_input() -> Input {
        Input {
            key: Key::Esc,
            ctrl: false,
            alt: false,
            shift: false,
        }
    }

    #[test]
    fn normal_i_enters_insert() {
        let vim = Vim::new(Mode::Normal);
        let mut ta = TextArea::default();
        match vim.transition(key_input('i'), &mut ta) {
            Transition::Mode(Mode::Insert) => {}
            _ => panic!("expected insert mode"),
        }
    }

    #[test]
    fn insert_esc_enters_normal() {
        let vim = Vim::new(Mode::Insert);
        let mut ta = TextArea::default();
        match vim.transition(esc_input(), &mut ta) {
            Transition::Mode(Mode::Normal) => {}
            _ => panic!("expected normal mode"),
        }
    }

    #[test]
    fn insert_ctrl_c_enters_normal() {
        let vim = Vim::new(Mode::Insert);
        let mut ta = TextArea::default();
        match vim.transition(ctrl_input('c'), &mut ta) {
            Transition::Mode(Mode::Normal) => {}
            _ => panic!("expected normal mode"),
        }
    }

    #[test]
    fn normal_v_enters_visual() {
        let vim = Vim::new(Mode::Normal);
        let mut ta = TextArea::default();
        match vim.transition(key_input('v'), &mut ta) {
            Transition::Mode(Mode::Visual) => {}
            _ => panic!("expected visual mode"),
        }
    }

    #[test]
    fn visual_esc_returns_to_normal() {
        let vim = Vim::new(Mode::Visual);
        let mut ta = TextArea::default();
        match vim.transition(esc_input(), &mut ta) {
            Transition::Mode(Mode::Normal) => {}
            _ => panic!("expected normal mode"),
        }
    }

    #[test]
    fn normal_d_enters_operator() {
        let vim = Vim::new(Mode::Normal);
        let mut ta = TextArea::default();
        match vim.transition(key_input('d'), &mut ta) {
            Transition::Mode(Mode::Operator('d')) => {}
            _ => panic!("expected operator(d) mode"),
        }
    }

    #[test]
    fn normal_o_enters_insert() {
        let vim = Vim::new(Mode::Normal);
        let mut ta = TextArea::default();
        match vim.transition(key_input('o'), &mut ta) {
            Transition::Mode(Mode::Insert) => {}
            _ => panic!("expected insert mode"),
        }
    }

    #[test]
    fn insert_types_text() {
        let vim = Vim::new(Mode::Insert);
        let mut ta = TextArea::default();
        vim.transition(key_input('h'), &mut ta);
        vim.transition(key_input('i'), &mut ta);
        assert_eq!(ta.lines(), &["hi"]);
    }

    #[test]
    fn null_key_is_nop() {
        let vim = Vim::new(Mode::Normal);
        let mut ta = TextArea::default();
        let input = Input {
            key: Key::Null,
            ctrl: false,
            alt: false,
            shift: false,
        };
        match vim.transition(input, &mut ta) {
            Transition::Nop => {}
            _ => panic!("expected nop"),
        }
    }

    #[test]
    fn unknown_key_returns_pending() {
        let vim = Vim::new(Mode::Normal);
        let mut ta = TextArea::default();
        match vim.transition(key_input('g'), &mut ta) {
            Transition::Pending(_) => {}
            _ => panic!("expected pending for first g"),
        }
    }

    #[test]
    fn mode_display() {
        assert_eq!(Mode::Normal.to_string(), "NORMAL");
        assert_eq!(Mode::Insert.to_string(), "INSERT");
        assert_eq!(Mode::Visual.to_string(), "VISUAL");
        assert_eq!(Mode::Operator('d').to_string(), "OPERATOR(d)");
    }

    #[test]
    fn operator_dd_deletes_line() {
        let vim = Vim::new(Mode::Normal);
        let mut ta = TextArea::default();
        ta.insert_str("hello\nworld");
        ta.move_cursor(tui_textarea::CursorMove::Top);

        // d enters Operator mode
        let vim = match vim.transition(key_input('d'), &mut ta) {
            Transition::Mode(Mode::Operator('d')) => Vim::new(Mode::Operator('d')),
            _ => panic!("expected operator(d) mode"),
        };
        // dd deletes the line
        match vim.transition(key_input('d'), &mut ta) {
            Transition::Mode(Mode::Normal) => {}
            _ => panic!("expected normal mode after dd"),
        }
        // First line should be deleted
        assert_eq!(ta.lines().len(), 1);
    }

    #[test]
    fn operator_yy_copies_line() {
        let vim = Vim::new(Mode::Normal);
        let mut ta = TextArea::default();
        ta.insert_str("hello\nworld");
        ta.move_cursor(tui_textarea::CursorMove::Top);

        // y enters Operator mode
        let vim = match vim.transition(key_input('y'), &mut ta) {
            Transition::Mode(Mode::Operator('y')) => Vim::new(Mode::Operator('y')),
            _ => panic!("expected operator(y) mode"),
        };
        // yy copies the line
        match vim.transition(key_input('y'), &mut ta) {
            Transition::Mode(Mode::Normal) => {}
            _ => panic!("expected normal mode after yy"),
        }
        // Text unchanged, can paste
        assert_eq!(ta.lines().len(), 2);
        ta.move_cursor(tui_textarea::CursorMove::Bottom);
        ta.move_cursor(tui_textarea::CursorMove::End);
        ta.paste();
        assert!(ta.lines().len() > 2);
    }

    #[test]
    fn visual_mode_delete() {
        let vim = Vim::new(Mode::Insert);
        let mut ta = TextArea::default();
        // Type some text
        vim.transition(key_input('A'), &mut ta);
        vim.transition(key_input('B'), &mut ta);
        vim.transition(key_input('C'), &mut ta);
        assert_eq!(ta.lines(), &["ABC"]);

        // Go to normal mode
        let vim = Vim::new(Mode::Normal);
        // Move to beginning
        ta.move_cursor(tui_textarea::CursorMove::Head);

        // Enter visual mode
        let vim = match vim.transition(key_input('v'), &mut ta) {
            Transition::Mode(Mode::Visual) => Vim::new(Mode::Visual),
            _ => panic!("expected visual mode"),
        };
        // Move right to select 'A'
        ta.move_cursor(tui_textarea::CursorMove::Forward);
        // Delete selection
        match vim.transition(key_input('d'), &mut ta) {
            Transition::Mode(Mode::Normal) => {}
            _ => panic!("expected normal mode after delete"),
        }
        // Some text should be deleted
        assert!(ta.lines()[0].len() < 3);
    }

    #[test]
    fn undo_restores_text() {
        let vim = Vim::new(Mode::Insert);
        let mut ta = TextArea::default();
        vim.transition(key_input('h'), &mut ta);
        vim.transition(key_input('i'), &mut ta);
        assert_eq!(ta.lines(), &["hi"]);

        // Normal mode, undo
        let vim = Vim::new(Mode::Normal);
        vim.transition(
            Input {
                key: Key::Char('u'),
                ctrl: false,
                alt: false,
                shift: false,
            },
            &mut ta,
        );
        // After undo, text should be shorter or empty
        assert_ne!(ta.lines(), &["hi"]);
    }

    #[test]
    fn normal_x_deletes_char() {
        let vim = Vim::new(Mode::Insert);
        let mut ta = TextArea::default();
        vim.transition(key_input('A'), &mut ta);
        vim.transition(key_input('B'), &mut ta);
        assert_eq!(ta.lines(), &["AB"]);

        let vim = Vim::new(Mode::Normal);
        ta.move_cursor(tui_textarea::CursorMove::Head);
        vim.transition(key_input('x'), &mut ta);
        assert_eq!(ta.lines(), &["B"]);
    }

    #[test]
    fn gg_moves_to_top() {
        let vim = Vim::new(Mode::Normal);
        let mut ta = TextArea::default();
        ta.insert_str("line1\nline2\nline3");
        // Cursor should be at end
        assert_eq!(ta.cursor().0, 2);

        // First g returns Pending
        let vim = match vim.transition(key_input('g'), &mut ta) {
            Transition::Pending(input) => Vim::new(Mode::Normal).with_pending(input),
            _ => panic!("expected pending"),
        };
        // Second g moves to top
        vim.transition(key_input('g'), &mut ta);
        assert_eq!(ta.cursor().0, 0);
    }
}
