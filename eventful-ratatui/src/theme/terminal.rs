//! The built-in theme: [`TerminalColors`], read from the terminal, and the
//! palette [`Theme::from_terminal`] derives from them.
use super::{
    Palette, Theme,
    color::{away_from, blend, contrast},
};
use ratatui::style::Color;
use std::time::Duration;

/// The terminal's own colors: its foreground, background and 16 ANSI colors,
/// as RGB.
///
/// [`query`](Self::query) asks the terminal for them; the loop does so at
/// startup and builds the built-in theme with [`Theme::from_terminal`]. Build
/// one with [`new`](Self::new) to preview the built-in theme for another
/// terminal's color scheme.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct TerminalColors {
    /// The default text color.
    pub foreground: Color,
    /// The default background color.
    pub background: Color,
    /// ANSI colors 0–15: black, red, green, yellow, blue, magenta, cyan,
    /// white, then their bright variants in the same order.
    pub ansi: [Color; 16],
}

impl Default for TerminalColors {
    /// A common dark palette, for when the terminal is not asked or does not
    /// answer.
    fn default() -> Self {
        Self::new(
            hex(0xcccccc),
            hex(0x1e1e1e),
            [
                0x000000, 0xcd3131, 0x0dbc79, 0xe5e510, 0x2472c8, 0xbc3fbc, 0x11a8cd, 0xe5e5e5,
                0x666666, 0xf14c4c, 0x23d18b, 0xf5f543, 0x3b8eea, 0xd670d6, 0x29b8db, 0xffffff,
            ]
            .map(hex),
        )
    }
}

impl TerminalColors {
    /// Colors from their parts. See [`Theme::from_terminal`] for an example.
    pub fn new(foreground: Color, background: Color, ansi: [Color; 16]) -> Self {
        Self {
            foreground,
            background,
            ansi,
        }
    }

    /// Ask the terminal for its colors, waiting at most `timeout` for the
    /// answer. `None` when output is not a terminal, the terminal does not
    /// answer, or on platforms without a query (Windows); colors it leaves
    /// out are taken from [`default`](Self::default).
    ///
    /// Call it before the terminal reads input in raw mode: the answers
    /// arrive as input. [`Tui::run`](crate::Tui::run) does this at startup.
    ///
    /// ```no_run
    /// use eventful_ratatui::theme::{self, TerminalColors, Theme};
    /// use std::time::Duration;
    ///
    /// let colors = TerminalColors::query(Duration::from_millis(500)).unwrap_or_default();
    /// theme::set(Theme::from_terminal(&colors));
    /// ```
    pub fn query(timeout: Duration) -> Option<Self> {
        let answer = query::ask(timeout)?;
        let mut colors = Self::default();
        let mut any = false;
        for (target, value) in answer {
            let slot = match target {
                query::Target::Foreground => &mut colors.foreground,
                query::Target::Background => &mut colors.background,
                query::Target::Ansi(i) => &mut colors.ansi[usize::from(i)],
            };
            *slot = value;
            any = true;
        }
        any.then_some(colors)
    }
}

/// Lowest contrast against the background for body text drawn in a derived
/// color, for state and status colors.
const TONE_FLOOR: f64 = 3.0;

impl Theme {
    /// The built-in theme: a palette derived from the terminal's own colors.
    ///
    /// Surfaces and wells are shades between the background and the
    /// foreground; the primary, destructive and status colors are the ANSI
    /// colors (the normal or the bright variant, whichever stands out more
    /// from the background), adjusted where they would be too faint to read.
    /// The terminal's foreground and background are kept as they are.
    ///
    /// ```
    /// use eventful_ratatui::{prelude::*, theme::{Appearance, TerminalColors, Theme}};
    ///
    /// let mut ansi = TerminalColors::default().ansi;
    /// ansi[4] = Color::Rgb(0x26, 0x8b, 0xd2); // blue
    /// let colors = TerminalColors::new(
    ///     Color::Rgb(0x58, 0x6e, 0x75),
    ///     Color::Rgb(0xfd, 0xf6, 0xe3),
    ///     ansi,
    /// );
    /// let theme = Theme::from_terminal(&colors);
    /// assert_eq!(theme.palette.background, colors.background);
    /// assert_eq!(theme.palette.foreground, colors.foreground);
    /// assert_eq!(theme.appearance(), Appearance::Light);
    /// ```
    pub fn from_terminal(colors: &TerminalColors) -> Self {
        let bg = colors.background;
        let fg = colors.foreground;
        let ansi = &colors.ansi;
        // The ANSI color of `index`, normal or bright, that reads best.
        let pick = |index: usize| {
            let (normal, bright) = (ansi[index], ansi[index + 8]);
            let better = if ratio(bright, bg) > ratio(normal, bg) {
                bright
            } else {
                normal
            };
            readable(better, bg, TONE_FLOOR)
        };
        let shade = |percent| blend(bg, fg, percent);
        // Wells and surfaces may not cost the text too much contrast.
        let floor = well_text_floor(ratio(fg, bg));
        let capped = |percent: u8| {
            let percent = (0..=percent)
                .rev()
                .find(|p| ratio(fg, shade(*p)) >= floor)
                .unwrap_or(0);
            shade(percent)
        };
        let field = capped(10);
        let primary = readable(pick(4), bg, TONE_FLOOR);
        let destructive = pick(1);
        let muted = readable(readable(blend(fg, bg, 30), bg, 4.5), field, 3.0);
        Theme::new(
            "Terminal",
            Palette {
                background: bg,
                surface: capped(5),
                field,
                foreground: fg,
                muted,
                primary,
                primary_foreground: label_on(primary, bg, fg),
                secondary: shade(17),
                secondary_foreground: fg,
                accent: pick(5),
                destructive,
                destructive_foreground: label_on(destructive, bg, fg),
                success: pick(2),
                warning: pick(3),
                info: pick(6),
                border: readable(shade(35), bg, 2.0),
                ring: readable(blend(primary, bg, 15), bg, TONE_FLOOR),
                selection: blend(bg, primary, 30),
            },
        )
    }
}

/// The contrast text keeps on wells: 4.5:1 where the terminal's own text
/// reaches 5:1, and nine tenths of what it has otherwise.
pub(crate) fn well_text_floor(text: f64) -> f64 {
    if text >= 5.0 { 4.5 } else { text * 0.9 }
}

fn hex(h: u32) -> Color {
    Color::Rgb((h >> 16) as u8, (h >> 8) as u8, h as u8)
}

fn ratio(a: Color, b: Color) -> f64 {
    contrast(a, b).unwrap_or(1.0)
}

/// `color`, moved away from `against` until their contrast reaches `floor`.
fn readable(color: Color, against: Color, floor: f64) -> Color {
    let away = away_from(against);
    (0..=100)
        .step_by(4)
        .map(|percent| blend(color, away, percent))
        .find(|c| ratio(*c, against) >= floor)
        .unwrap_or(away)
}

/// The label for a fill: the terminal's background or foreground, whichever
/// reads better on it, or black or white when neither reads well enough.
fn label_on(fill: Color, bg: Color, fg: Color) -> Color {
    let best = |a: Color, b: Color| {
        if ratio(a, fill) >= ratio(b, fill) {
            a
        } else {
            b
        }
    };
    let candidate = best(bg, fg);
    if ratio(candidate, fill) >= 4.5 {
        candidate
    } else {
        best(Color::Rgb(0, 0, 0), Color::Rgb(255, 255, 255))
    }
}

/// Asking the terminal: the escape sequences and their answers.
mod query {
    use ratatui::style::Color;
    use std::time::Duration;

    /// What an answer is about.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(super) enum Target {
        Foreground,
        Background,
        Ansi(u8),
    }

    /// The question: foreground, background and the 16 ANSI colors, then a
    /// device attributes request (DA1). Terminals answer in order, so the DA1
    /// answer marks the end, also when the colors get no answer.
    pub(super) fn request() -> Vec<u8> {
        let mut out = b"\x1b]10;?\x1b\\\x1b]11;?\x1b\\".to_vec();
        for i in 0..16 {
            out.extend(format!("\x1b]4;{i};?\x1b\\").bytes());
        }
        out.extend(b"\x1b[c");
        out
    }

    /// Whether `input` holds the end of a DA1 answer: `ESC [ ? … c`.
    pub(super) fn complete(input: &[u8]) -> bool {
        input.windows(3).enumerate().any(|(start, w)| {
            w == b"\x1b[?"
                && input[start + 3..]
                    .iter()
                    .find(|b| !(b.is_ascii_digit() || **b == b';'))
                    == Some(&b'c')
        })
    }

    /// The color answers in `input`: `OSC 10 ; spec`, `OSC 11 ; spec` and
    /// `OSC 4 ; index ; spec`, ended by BEL or ST.
    pub(super) fn parse(input: &[u8]) -> Vec<(Target, Color)> {
        let text = String::from_utf8_lossy(input);
        let mut found = Vec::new();
        for answer in text.split("\x1b]").skip(1) {
            let end = answer.find(['\x07', '\x1b']).unwrap_or(answer.len());
            let answer = &answer[..end];
            let mut parts = answer.split(';');
            let target = match parts.next() {
                Some("10") => Target::Foreground,
                Some("11") => Target::Background,
                Some("4") => match parts.next().and_then(|i| i.parse().ok()) {
                    Some(i) if i < 16 => Target::Ansi(i),
                    _ => continue,
                },
                _ => continue,
            };
            if let Some(color) = parts.next().and_then(spec) {
                found.push((target, color));
            }
        }
        found
    }

    /// An X11 color spec: `rgb:R/G/B` or `rgba:R/G/B/A` with 1–4 hex digits
    /// per channel, or `#RRGGBB`.
    pub(super) fn spec(spec: &str) -> Option<Color> {
        let channel = |digits: &str| {
            if digits.is_empty() || digits.len() > 4 {
                return None;
            }
            let value = u32::from_str_radix(digits, 16).ok()?;
            let max = (1u32 << (4 * digits.len())) - 1;
            Some(((value * 255 + max / 2) / max) as u8)
        };
        let channels = if let Some(rest) = spec.strip_prefix("rgb:") {
            rest.split('/').collect::<Vec<_>>()
        } else if let Some(rest) = spec.strip_prefix("rgba:") {
            rest.split('/').take(3).collect()
        } else {
            // `get`, not indexing: the answer may not be ASCII.
            let hex = spec.strip_prefix('#').filter(|h| h.len() == 6)?;
            vec![hex.get(0..2)?, hex.get(2..4)?, hex.get(4..6)?]
        };
        match channels[..] {
            [r, g, b] => Some(Color::Rgb(channel(r)?, channel(g)?, channel(b)?)),
            _ => None,
        }
    }

    /// Send the request to the controlling terminal and read the answer.
    #[cfg(unix)]
    pub(super) fn ask(timeout: Duration) -> Option<Vec<(Target, Color)>> {
        use rustix::termios::{self, OptionalActions, SpecialCodeIndex};
        use std::{
            fs::OpenOptions,
            io::{IsTerminal, Read, Write},
            time::Instant,
        };
        // Nothing to ask when the app does not draw to a terminal.
        if !std::io::stdout().is_terminal() {
            return None;
        }
        let mut tty = OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/tty")
            .ok()?;
        let saved = termios::tcgetattr(&tty).ok()?;
        let mut raw = saved.clone();
        raw.make_raw();
        // Reads return after a tenth of a second without input, so the
        // deadline is checked without polling the terminal.
        raw.special_codes[SpecialCodeIndex::VMIN] = 0;
        raw.special_codes[SpecialCodeIndex::VTIME] = 1;
        termios::tcsetattr(&tty, OptionalActions::Now, &raw).ok()?;
        let answer = (|| {
            tty.write_all(&request()).ok()?;
            tty.flush().ok()?;
            let deadline = Instant::now() + timeout;
            let mut input = Vec::new();
            let mut chunk = [0; 512];
            while !complete(&input) && Instant::now() < deadline {
                match tty.read(&mut chunk) {
                    Ok(read) => input.extend_from_slice(&chunk[..read]),
                    Err(err) if err.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(_) => return None,
                }
            }
            Some(parse(&input))
        })();
        let _ = termios::tcsetattr(&tty, OptionalActions::Now, &saved);
        answer
    }

    /// No query on this platform.
    #[cfg(not(unix))]
    pub(super) fn ask(_timeout: Duration) -> Option<Vec<(Target, Color)>> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::query::*;
    use super::*;

    #[test]
    fn color_specs_scale_to_eight_bits() {
        assert_eq!(spec("rgb:ffff/0000/8080"), Some(Color::Rgb(255, 0, 128)));
        assert_eq!(spec("rgb:f/0/8"), Some(Color::Rgb(255, 0, 136)));
        assert_eq!(spec("rgba:ff/00/00/ff"), Some(Color::Rgb(255, 0, 0)));
        assert_eq!(spec("#1e1e2e"), Some(Color::Rgb(0x1e, 0x1e, 0x2e)));
        assert_eq!(spec("rgb:ff/00"), None);
        assert_eq!(spec("blue"), None);
        assert_eq!(spec("#ééé"), None, "no panic on non-ASCII");
    }

    #[test]
    fn answers_parse_with_either_terminator() {
        let input = b"\x1b]10;rgb:cccc/cccc/cccc\x1b\\\x1b]11;rgb:1e1e/1e1e/1e1e\x07\
            \x1b]4;12;rgb:3b3b/8e8e/eaea\x1b\\\x1b]4;99;rgb:0/0/0\x07\x1b[?64;1;22c";
        assert!(complete(input));
        assert_eq!(
            parse(input),
            [
                (Target::Foreground, Color::Rgb(0xcc, 0xcc, 0xcc)),
                (Target::Background, Color::Rgb(0x1e, 0x1e, 0x1e)),
                (Target::Ansi(12), Color::Rgb(0x3b, 0x8e, 0xea)),
            ]
        );
    }

    #[test]
    fn the_answer_is_complete_at_device_attributes() {
        assert!(!complete(b"\x1b]11;rgb:0/0/0\x07"));
        assert!(!complete(b"\x1b[?64;1"));
        assert!(complete(b"\x1b[?1;2c"));
        assert_eq!(request().iter().filter(|b| **b == 0x1b).count(), 2 * 18 + 1);
    }

    #[test]
    fn tones_come_from_the_ansi_colors() {
        let colors = TerminalColors::default();
        let theme = Theme::from_terminal(&colors);
        // Bright blue stands out more on the dark default background.
        assert_eq!(theme.palette.primary, colors.ansi[12]);
        assert_eq!(theme.palette.background, colors.background);
        assert_eq!(theme.palette.foreground, colors.foreground);
    }
}
