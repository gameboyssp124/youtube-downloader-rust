use iced::widget::{container, progress_bar};
use iced::{Theme, Color, Background, Border};

pub fn hex_color(hex: &str) -> Color {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 { return Color::BLACK; }
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
    Color::from_rgb8(r, g, b)
}

pub struct DarkBackgroundStyle;
impl container::StyleSheet for DarkBackgroundStyle {
    type Style = Theme;
    fn appearance(&self, _: &Self::Style) -> container::Appearance {
        container::Appearance {
            background: Some(Background::Color(hex_color("#1e1e2e"))),
            text_color: Some(Color::WHITE),
            ..Default::default()
        }
    }
}

pub struct DarkCardStyle;
impl container::StyleSheet for DarkCardStyle {
    type Style = Theme;
    fn appearance(&self, _: &Self::Style) -> container::Appearance {
        container::Appearance {
            background: Some(Background::Color(hex_color("#313244"))),
            border: Border { color: hex_color("#45475a"), width: 1.0, radius: 8.0.into() },
            ..Default::default()
        }
    }
}

pub struct BarStyle { pub color: Color }
impl progress_bar::StyleSheet for BarStyle {
    type Style = Theme;
    fn appearance(&self, _: &Self::Style) -> progress_bar::Appearance {
        progress_bar::Appearance {
            background: Background::Color(hex_color("#45475a")),
            bar: Background::Color(self.color),
            border_radius: 4.0.into(),
        }
    }
}