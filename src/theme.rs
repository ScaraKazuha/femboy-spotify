//! Pink femboy palette ♡

use ratatui::style::{Color, Modifier, Style};

pub const HOT_PINK: Color = Color::Rgb(255, 92, 170);
pub const PINK: Color = Color::Rgb(255, 133, 200);
pub const PASTEL: Color = Color::Rgb(255, 196, 228);
pub const LAVENDER: Color = Color::Rgb(200, 162, 255);
pub const BABY_BLUE: Color = Color::Rgb(150, 205, 255);
pub const WHITE: Color = Color::Rgb(255, 240, 250);
pub const DIM: Color = Color::Rgb(160, 120, 150);
pub const BG: Color = Color::Rgb(34, 20, 36);
pub const BG_SEL: Color = Color::Rgb(92, 38, 82);

/// Colors of the femboy pride flag, used for the animated logo.
pub const FLAG: [Color; 6] = [
    HOT_PINK,
    PINK,
    PASTEL,
    WHITE,
    BABY_BLUE,
    LAVENDER,
];

pub fn fg(c: Color) -> Style {
    Style::default().fg(c)
}

pub fn bold(c: Color) -> Style {
    Style::default().fg(c).add_modifier(Modifier::BOLD)
}

pub fn selected() -> Style {
    Style::default()
        .bg(BG_SEL)
        .fg(WHITE)
        .add_modifier(Modifier::BOLD)
}
