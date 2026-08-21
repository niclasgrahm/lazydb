use ratatui::style::{Color, Style};

pub const WORKSPACE: Color = Color::Rgb(26, 29, 35);
pub const NAVIGATION: Color = Color::Rgb(31, 35, 43);
pub const RESULTS: Color = Color::Rgb(36, 41, 50);
pub const STATUS: Color = Color::Rgb(37, 42, 51);
pub const FOREGROUND: Color = Color::Rgb(200, 204, 212);
pub const MUTED: Color = Color::Rgb(107, 114, 128);
pub const ACCENT: Color = Color::Rgb(138, 169, 201);
pub const SELECTION: Color = Color::Rgb(58, 65, 80);

pub fn surface(background: Color) -> Style {
    Style::default().fg(FOREGROUND).bg(background)
}

pub fn title(focused: bool) -> Style {
    Style::default()
        .fg(if focused { ACCENT } else { MUTED })
        .bg(if focused { WORKSPACE } else { NAVIGATION })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pane_surfaces_are_visually_distinct() {
        assert_ne!(WORKSPACE, NAVIGATION);
        assert_ne!(WORKSPACE, RESULTS);
        assert_ne!(NAVIGATION, RESULTS);
        assert_ne!(RESULTS, STATUS);
    }
}
