//! Color math for deriving state colors from a [`Palette`](super::Palette).
//!
//! Every helper works on RGB channels: [`Color::Rgb`] as is, and the 16 named
//! colors through a fixed VGA approximation. `Indexed` and `Reset` carry no
//! channels and pass through unchanged.
//!
//! The channel table and the blend rounding follow ratcn's `color` module
//! (MIT, Copyright (c) 2026 Kristofer Lund); see `THIRD-PARTY-NOTICES.md`.
use ratatui::style::Color;

/// The RGB channels of `color`, if it has any.
pub const fn channels(color: Color) -> Option<(u8, u8, u8)> {
    Some(match color {
        Color::Rgb(r, g, b) => (r, g, b),
        Color::Black => (0, 0, 0),
        Color::Red => (128, 0, 0),
        Color::Green => (0, 128, 0),
        Color::Yellow => (128, 128, 0),
        Color::Blue => (0, 0, 128),
        Color::Magenta => (128, 0, 128),
        Color::Cyan => (0, 128, 128),
        Color::Gray => (192, 192, 192),
        Color::DarkGray => (128, 128, 128),
        Color::LightRed => (255, 0, 0),
        Color::LightGreen => (0, 255, 0),
        Color::LightYellow => (255, 255, 0),
        Color::LightBlue => (0, 0, 255),
        Color::LightMagenta => (255, 0, 255),
        Color::LightCyan => (0, 255, 255),
        Color::White => (255, 255, 255),
        Color::Indexed(_) | Color::Reset => return None,
    })
}

/// Blend `color` `percent` of the way toward `toward`. Colors without
/// channels come back unchanged.
pub const fn blend(color: Color, toward: Color, percent: u8) -> Color {
    let (Some((r, g, b)), Some((tr, tg, tb))) = (channels(color), channels(toward)) else {
        return color;
    };
    let amount = if percent > 100 { 100 } else { percent } as u16;
    Color::Rgb(mix(r, tr, amount), mix(g, tg, amount), mix(b, tb, amount))
}

/// `amount` percent of the way from `a` to `b`, rounded.
const fn mix(a: u8, b: u8, amount: u16) -> u8 {
    ((a as u16 * (100 - amount) + b as u16 * amount + 50) / 100) as u8
}

/// Relative luminance per WCAG 2.1, from 0 (black) to 1 (white).
pub fn luminance(color: Color) -> Option<f64> {
    let (r, g, b) = channels(color)?;
    let linear = |c: u8| {
        let c = f64::from(c) / 255.0;
        if c <= 0.039_28 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    Some(0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b))
}

/// The WCAG 2.1 contrast ratio between two colors, from 1 to 21.
pub fn contrast(a: Color, b: Color) -> Option<f64> {
    let (a, b) = (luminance(a)?, luminance(b)?);
    Some((a.max(b) + 0.05) / (a.min(b) + 0.05))
}

/// Whether `color` is light: things drawn on it get darker to stand out.
/// Colors without channels count as dark.
pub fn is_light(color: Color) -> bool {
    luminance(color).is_some_and(|l| l > 0.4)
}

/// White on a dark `color`, black on a light one: the direction that stands
/// out from it.
pub fn away_from(color: Color) -> Color {
    if is_light(color) {
        Color::Rgb(0, 0, 0)
    } else {
        Color::Rgb(255, 255, 255)
    }
}

/// Shift a fill `percent` away from its own lightness: a dark fill gets
/// lighter, a light one darker.
pub fn lift(color: Color, percent: u8) -> Color {
    blend(color, away_from(color), percent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blend_rounds_and_clamps() {
        let a = Color::Rgb(0, 100, 200);
        let b = Color::Rgb(255, 0, 0);
        assert_eq!(blend(a, b, 0), a);
        assert_eq!(blend(a, b, 100), b);
        assert_eq!(blend(a, b, 200), b);
        assert_eq!(blend(a, b, 50), Color::Rgb(128, 50, 100));
        assert_eq!(blend(Color::Reset, b, 50), Color::Reset);
    }

    #[test]
    fn contrast_spans_black_to_white() {
        let ratio = contrast(Color::Black, Color::White).unwrap();
        assert!((ratio - 21.0).abs() < 0.01);
        assert_eq!(contrast(Color::Reset, Color::White), None);
    }

    #[test]
    fn lift_moves_away_from_the_fill() {
        assert_eq!(lift(Color::Rgb(0, 0, 0), 10), Color::Rgb(26, 26, 26));
        assert_eq!(
            lift(Color::Rgb(255, 255, 255), 10),
            Color::Rgb(230, 230, 230)
        );
    }
}
