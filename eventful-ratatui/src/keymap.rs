//! [`Keymap`]: a builder mapping key chords (`"j"`, `"ctrl+s"`) to closures
//! over a receiver, plus the [`KeyChord`] parser. The `#[key_events]` macro in
//! `eventful-ratatui-macros` will build on the same parser.
use crate::Handled;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::{fmt, str::FromStr};

/// A key plus modifiers, normalized so that it compares equal to the key
/// events terminals report for it.
///
/// Parsed from strings like `"j"`, `"J"`, `"ctrl+s"`, `"alt+enter"`,
/// `"shift+tab"` or `"f5"`. Modifiers are `ctrl`, `alt`, `shift`, `super`,
/// `hyper` and `meta`. Keys are a single character or one of `enter`, `esc`,
/// `tab`, `backtab`, `backspace`, `delete`, `insert`, `home`, `end`,
/// `pageup`, `pagedown`, `up`, `down`, `left`, `right`, `space`, `plus` and
/// `f1`–`f24`. Names are case-insensitive; characters are not, and `"J"`
/// means shift+j.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct KeyChord {
    /// The key, with shift folded into characters.
    code: KeyCode,
    /// Modifiers other than a folded shift.
    modifiers: KeyModifiers,
}

impl KeyChord {
    /// A chord for `code` with `modifiers`, normalized.
    pub fn new(code: KeyCode, modifiers: KeyModifiers) -> Self {
        let (code, modifiers) = match code {
            // Terminals report the shifted character, sometimes with SHIFT.
            KeyCode::Char(c) if modifiers.contains(KeyModifiers::SHIFT) || c.is_uppercase() => (
                KeyCode::Char(c.to_ascii_uppercase()),
                modifiers - KeyModifiers::SHIFT,
            ),
            KeyCode::Tab if modifiers.contains(KeyModifiers::SHIFT) => {
                (KeyCode::BackTab, modifiers - KeyModifiers::SHIFT)
            }
            KeyCode::BackTab => (KeyCode::BackTab, modifiers - KeyModifiers::SHIFT),
            code => (code, modifiers),
        };
        Self { code, modifiers }
    }
    /// The chord a key event represents.
    pub fn from_event(key: &KeyEvent) -> Self {
        Self::new(key.code, key.modifiers)
    }
    /// The normalized key.
    pub fn code(&self) -> KeyCode {
        self.code
    }
    /// The normalized modifiers.
    pub fn modifiers(&self) -> KeyModifiers {
        self.modifiers
    }
}

impl FromStr for KeyChord {
    type Err = ParseKeyChordError;

    fn from_str(chord: &str) -> Result<Self, Self::Err> {
        let error = |reason| ParseKeyChordError {
            chord: chord.to_owned(),
            reason,
        };
        // A trailing `+` is the plus key itself: "+", "ctrl++".
        let (prefix, key) = match chord.strip_suffix('+') {
            Some(prefix) if prefix.is_empty() || prefix.ends_with('+') => {
                (prefix.strip_suffix('+').unwrap_or(prefix), "+")
            }
            _ => chord.rsplit_once('+').unwrap_or(("", chord)),
        };
        let mut modifiers = KeyModifiers::NONE;
        for name in prefix.split('+').filter(|_| !prefix.is_empty()) {
            modifiers |= match name.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => KeyModifiers::CONTROL,
                "alt" | "option" => KeyModifiers::ALT,
                "shift" => KeyModifiers::SHIFT,
                "super" | "cmd" => KeyModifiers::SUPER,
                "hyper" => KeyModifiers::HYPER,
                "meta" => KeyModifiers::META,
                _ => return Err(error("unknown modifier")),
            };
        }
        let mut chars = key.chars();
        let code = match (chars.next(), chars.next()) {
            (None, _) => return Err(error("missing key")),
            (Some(c), None) => KeyCode::Char(c),
            _ => match key.to_ascii_lowercase().as_str() {
                "enter" | "return" => KeyCode::Enter,
                "esc" | "escape" => KeyCode::Esc,
                "tab" => KeyCode::Tab,
                "backtab" => KeyCode::BackTab,
                "backspace" => KeyCode::Backspace,
                "delete" | "del" => KeyCode::Delete,
                "insert" | "ins" => KeyCode::Insert,
                "home" => KeyCode::Home,
                "end" => KeyCode::End,
                "pageup" | "pgup" => KeyCode::PageUp,
                "pagedown" | "pgdn" => KeyCode::PageDown,
                "up" => KeyCode::Up,
                "down" => KeyCode::Down,
                "left" => KeyCode::Left,
                "right" => KeyCode::Right,
                "space" => KeyCode::Char(' '),
                "plus" => KeyCode::Char('+'),
                name => match name.strip_prefix('f').and_then(|n| n.parse().ok()) {
                    Some(n @ 1..=24) => KeyCode::F(n),
                    _ => return Err(error("unknown key")),
                },
            },
        };
        Ok(Self::new(code, modifiers))
    }
}

impl TryFrom<&str> for KeyChord {
    type Error = ParseKeyChordError;

    fn try_from(chord: &str) -> Result<Self, Self::Error> {
        chord.parse()
    }
}

impl fmt::Display for KeyChord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (modifier, name) in [
            (KeyModifiers::CONTROL, "ctrl"),
            (KeyModifiers::ALT, "alt"),
            (KeyModifiers::SHIFT, "shift"),
            (KeyModifiers::SUPER, "super"),
            (KeyModifiers::HYPER, "hyper"),
            (KeyModifiers::META, "meta"),
        ] {
            if self.modifiers.contains(modifier) {
                write!(f, "{name}+")?;
            }
        }
        match self.code {
            KeyCode::Char(' ') => f.write_str("space"),
            KeyCode::Char(c) => write!(f, "{c}"),
            KeyCode::F(n) => write!(f, "f{n}"),
            KeyCode::PageUp => f.write_str("pageup"),
            KeyCode::PageDown => f.write_str("pagedown"),
            KeyCode::BackTab => f.write_str("backtab"),
            code => write!(f, "{}", code.to_string().to_lowercase()),
        }
    }
}

/// A key chord string could not be parsed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseKeyChordError {
    /// The rejected input.
    chord: String,
    /// What was wrong with it.
    reason: &'static str,
}

impl fmt::Display for ParseKeyChordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid key chord `{}`: {}", self.chord, self.reason)
    }
}

impl std::error::Error for ParseKeyChordError {}

/// An action bound to a chord.
type Action<T> = Box<dyn Fn(&T)>;

/// Key bindings that call closures with a receiver, usually the component
/// that owns the map:
///
/// ```
/// use eventful_ratatui::{Handled, keymap::Keymap};
/// # use crossterm::event::{Event, KeyCode, KeyEvent};
/// use std::cell::Cell;
///
/// struct Counter {
///     count: Cell<u32>,
///     keys: Keymap<Counter>,
/// }
/// let counter = Counter {
///     count: Cell::new(0),
///     keys: Keymap::new()
///         .bind("j", |c: &Counter| c.count.set(c.count.get() + 1))
///         .bind("ctrl+r", |c: &Counter| c.count.set(0)),
/// };
/// let j = Event::Key(KeyEvent::from(KeyCode::Char('j')));
/// assert_eq!(counter.keys.dispatch(&counter, &j), Handled::Yes);
/// assert_eq!(counter.count.get(), 1);
/// ```
///
/// Dispatch is synchronous, so the result can stop input from bubbling up the
/// focus chain. Actions that should reach several listeners can emit an event.
pub struct Keymap<T: ?Sized> {
    /// Bindings in insertion order; the first match wins.
    bindings: Vec<(KeyChord, Action<T>)>,
}

impl<T: ?Sized> Keymap<T> {
    /// An empty key map.
    pub fn new() -> Self {
        Self {
            bindings: Vec::new(),
        }
    }

    /// Bind `chord` to `action`. Bind a chord again to add an alternative key
    /// for the same action.
    ///
    /// # Panics
    /// Panics if `chord` is a string that does not parse as a [`KeyChord`].
    #[track_caller]
    pub fn bind<C>(mut self, chord: C, action: impl Fn(&T) + 'static) -> Self
    where
        C: TryInto<KeyChord>,
        C::Error: fmt::Display,
    {
        let chord = chord.try_into().unwrap_or_else(|err| panic!("{err}"));
        self.bindings.push((chord, Box::new(action)));
        self
    }

    /// Run the action bound to a key press or repeat in `event`, if any.
    /// Key releases and other events are not handled.
    pub fn dispatch(&self, receiver: &T, event: &Event) -> Handled {
        let Event::Key(key) = event else {
            return Handled::No;
        };
        if key.kind == KeyEventKind::Release {
            return Handled::No;
        }
        let chord = KeyChord::from_event(key);
        let Some((_, action)) = self.bindings.iter().find(|(bound, _)| *bound == chord) else {
            return Handled::No;
        };
        action(receiver);
        Handled::Yes
    }

    /// The bound chords, in insertion order, for help screens.
    pub fn chords(&self) -> impl Iterator<Item = KeyChord> + '_ {
        self.bindings.iter().map(|(chord, _)| *chord)
    }
}

impl<T: ?Sized> Default for Keymap<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: ?Sized> fmt::Debug for Keymap<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entries(self.chords().map(|chord| chord.to_string()))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEventState;
    use std::cell::Cell;

    fn chord(s: &str) -> KeyChord {
        s.parse().unwrap()
    }
    fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    #[test]
    fn parses_keys_and_modifiers() {
        assert_eq!(
            chord("j"),
            KeyChord::new(KeyCode::Char('j'), KeyModifiers::NONE)
        );
        assert_eq!(
            chord("Ctrl+Alt+Enter"),
            KeyChord::new(KeyCode::Enter, KeyModifiers::CONTROL | KeyModifiers::ALT)
        );
        assert_eq!(chord("f12").code(), KeyCode::F(12));
        assert_eq!(chord("space").code(), KeyCode::Char(' '));
        assert_eq!(chord("+").code(), KeyCode::Char('+'));
        assert_eq!(chord("ctrl++"), chord("ctrl+plus"));
        assert_eq!(chord("ctrl++").modifiers(), KeyModifiers::CONTROL);
    }

    #[test]
    fn rejects_invalid_chords() {
        for bad in ["", "ctrl+", "hyperr+x", "f25", "f0", "enterr"] {
            assert!(bad.parse::<KeyChord>().is_err(), "{bad:?} parsed");
        }
        let err = "foo+x".parse::<KeyChord>().unwrap_err();
        assert_eq!(
            err.to_string(),
            "invalid key chord `foo+x`: unknown modifier"
        );
    }

    #[test]
    fn shift_folds_into_characters_and_tab() {
        let shifted = KeyChord::from_event(&key(KeyCode::Char('J'), KeyModifiers::SHIFT));
        assert_eq!(shifted, chord("J"));
        assert_eq!(shifted, chord("shift+j"));
        assert_ne!(shifted, chord("j"));
        // Some terminals omit SHIFT on shifted characters.
        assert_eq!(
            KeyChord::from_event(&key(KeyCode::Char('J'), KeyModifiers::NONE)),
            chord("J")
        );
        assert_eq!(
            KeyChord::from_event(&key(KeyCode::Char('?'), KeyModifiers::SHIFT)),
            chord("?")
        );
        let backtab = KeyChord::from_event(&key(KeyCode::BackTab, KeyModifiers::SHIFT));
        assert_eq!(backtab, chord("shift+tab"));
        assert_eq!(backtab, chord("backtab"));
    }

    #[test]
    fn displays_canonical_form() {
        for s in [
            "j",
            "J",
            "ctrl+s",
            "alt+enter",
            "f5",
            "space",
            "pagedown",
            "backtab",
            "ctrl++",
        ] {
            assert_eq!(chord(&chord(s).to_string()), chord(s), "{s}");
        }
        assert_eq!(chord("Ctrl+Enter").to_string(), "ctrl+enter");
    }

    #[test]
    fn dispatches_presses_and_repeats_only() {
        let keys = Keymap::new()
            .bind("j", |n: &Cell<u32>| n.set(n.get() + 1))
            .bind("down", |n: &Cell<u32>| n.set(n.get() + 1));
        let n = Cell::new(0);
        let event = |kind| {
            Event::Key(KeyEvent {
                code: KeyCode::Char('j'),
                modifiers: KeyModifiers::NONE,
                kind,
                state: KeyEventState::NONE,
            })
        };
        assert_eq!(keys.dispatch(&n, &event(KeyEventKind::Press)), Handled::Yes);
        assert_eq!(
            keys.dispatch(&n, &event(KeyEventKind::Repeat)),
            Handled::Yes
        );
        assert_eq!(
            keys.dispatch(&n, &event(KeyEventKind::Release)),
            Handled::No
        );
        assert_eq!(
            keys.dispatch(&n, &Event::Key(KeyCode::Down.into())),
            Handled::Yes
        );
        assert_eq!(
            keys.dispatch(&n, &Event::Key(KeyCode::Up.into())),
            Handled::No
        );
        assert_eq!(keys.dispatch(&n, &Event::FocusGained), Handled::No);
        assert_eq!(n.get(), 3);
        assert_eq!(format!("{keys:?}"), r#"["j", "down"]"#);
    }

    #[test]
    #[should_panic(expected = "invalid key chord `ctrl+`: missing key")]
    fn bind_panics_on_invalid_chord() {
        let _ = Keymap::<()>::new().bind("ctrl+", |_| {});
    }
}
