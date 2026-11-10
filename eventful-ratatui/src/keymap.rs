//! Key bindings: [`KeyChord`], a key plus modifiers parsed from strings like
//! `"j"` or `"ctrl+s"`, and [`Keymap`], a builder mapping chords to closures
//! over a receiver.
//!
//! [`key_events`](crate::key_events) binds chords to eventful-rs events
//! instead, and checks them at compile time with the same syntax. Use a
//! [`Keymap`] for bindings that only the owning component needs, or that
//! are only known at run time.
//!
//! ```
//! use eventful_ratatui::{prelude::*, testing::Harness};
//! use std::cell::Cell;
//!
//! struct Pager {
//!     page: Cell<u32>,
//!     keys: Keymap<Pager>,
//! }
//! impl Pager {
//!     fn turn(&self, by: i32) {
//!         self.page.set(self.page.get().saturating_add_signed(by));
//!         invalidate::invalidate();
//!     }
//! }
//! impl Component for Pager {
//!     fn render(&self, area: Rect, buf: &mut Buffer) {
//!         format!("page {}", self.page.get()).render(area, buf);
//!     }
//!     fn on_input(&self, event: &InputEvent) -> Handled {
//!         self.keys.dispatch(self, event)
//!     }
//! }
//!
//! let pager = Pager {
//!     page: Cell::new(1),
//!     keys: Keymap::new()
//!         .bind("pagedown", |p: &Pager| p.turn(1))
//!         .bind("space", |p: &Pager| p.turn(1))
//!         .bind("pageup", |p: &Pager| p.turn(-1)),
//! };
//! let mut ui = Harness::new(pager, 10, 1);
//! ui.keys(&["space", "pagedown", "pageup"]);
//! assert_eq!(ui.lines(), ["page 2"]);
//! ```
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
/// `f1`–`f24`. Names are case-insensitive. A character on its own is
/// case-sensitive, so `"J"` means shift+j, as in vi-style bindings like `"G"`.
/// With any modifier other than shift, a letter's case is ignored: `"Ctrl+S"`
/// is ctrl+s, as in the menus of desktop applications, and ctrl+shift+s is
/// written `"ctrl+shift+s"`.
///
/// Some names have aliases: `control`, `option` (alt), `cmd` (super),
/// `return`, `escape`, `del`, `ins`, `pgup` and `pgdn`. A trailing `+` is the
/// plus key itself, so `"+"` and `"ctrl++"` work. Shift folds into
/// characters and into Tab, so `"shift+j"` equals `"J"` and `"shift+tab"`
/// equals `"backtab"`, matching what terminals report either way.
///
/// ```
/// use eventful_ratatui::prelude::*;
///
/// let save: KeyChord = "Ctrl+S".parse()?;
/// assert_eq!(save, KeyChord::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
/// assert_eq!(save, "ctrl+s".parse()?);
/// assert_eq!(save.to_string(), "ctrl+s");
///
/// // Shift is explicit next to other modifiers...
/// let save_as: KeyChord = "ctrl+shift+s".parse()?;
/// assert_ne!(save_as, save);
/// assert_eq!(save_as.to_string(), "ctrl+shift+s");
///
/// // ...and implied by a capital letter on its own.
/// assert_eq!("shift+j".parse::<KeyChord>()?, "J".parse()?);
/// assert_eq!("J".parse::<KeyChord>()?.to_string(), "J");
/// assert_eq!("shift+tab".parse::<KeyChord>()?.code(), KeyCode::BackTab);
/// assert!("ctrl+nope".parse::<KeyChord>().is_err());
/// # Ok::<(), eventful_ratatui::keymap::ParseKeyChordError>(())
/// ```
///
/// [`Display`](fmt::Display) writes the canonical form, lowercase with
/// modifiers in a fixed order, which parses back to the same chord: use it
/// for help lines.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct KeyChord {
    /// The key, with shift folded into characters.
    code: KeyCode,
    /// Modifiers other than a folded shift.
    modifiers: KeyModifiers,
}

impl KeyChord {
    /// A chord for `code` with `modifiers`, normalized: shift folds into
    /// characters (uppercased) and turns Tab into `BackTab`.
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
    /// A chord whose parts are already normalized, for generated code.
    pub(crate) const fn from_normalized(code: KeyCode, modifiers: KeyModifiers) -> Self {
        Self { code, modifiers }
    }
    /// The chord a key event represents. Its kind (press, repeat, release)
    /// is ignored.
    ///
    /// ```
    /// use eventful_ratatui::prelude::*;
    ///
    /// let event = KeyEvent::new(KeyCode::Char('J'), KeyModifiers::SHIFT);
    /// assert_eq!(KeyChord::from_event(&event).to_string(), "J");
    /// ```
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
        // Next to ctrl, alt and the like, a letter names the key, not the
        // character: "Ctrl+S" is ctrl+s, and shift has to be spelled out.
        let code = match code {
            KeyCode::Char(c) if !(modifiers - KeyModifiers::SHIFT).is_empty() => {
                KeyCode::Char(c.to_ascii_lowercase())
            }
            code => code,
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
        // A folded shift is written out next to other modifiers, because
        // the parser ignores a letter's case there.
        let mut modifiers = self.modifiers;
        let mut code = self.code;
        if let KeyCode::Char(c) = code
            && c.is_ascii_uppercase()
            && !modifiers.is_empty()
        {
            modifiers |= KeyModifiers::SHIFT;
            code = KeyCode::Char(c.to_ascii_lowercase());
        }
        for (modifier, name) in [
            (KeyModifiers::CONTROL, "ctrl"),
            (KeyModifiers::ALT, "alt"),
            (KeyModifiers::SHIFT, "shift"),
            (KeyModifiers::SUPER, "super"),
            (KeyModifiers::HYPER, "hyper"),
            (KeyModifiers::META, "meta"),
        ] {
            if modifiers.contains(modifier) {
                write!(f, "{name}+")?;
            }
        }
        match code {
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

/// A key chord string could not be parsed: an unknown modifier or key name,
/// or a missing key, as in `"ctrl+"`. Displays the input and the reason.
///
/// ```
/// use eventful_ratatui::KeyChord;
///
/// let err = "ctrl+".parse::<KeyChord>().unwrap_err();
/// assert_eq!(err.to_string(), "invalid key chord `ctrl+`: missing key");
/// ```
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

/// The chord of a key press or repeat; `None` for releases and other input.
pub(crate) fn pressed(event: &Event) -> Option<KeyChord> {
    match event {
        Event::Key(key) if key.kind != KeyEventKind::Release => Some(KeyChord::from_event(key)),
        _ => None,
    }
}

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
/// See the [module docs](self) for a keymap inside a component.
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

    /// Bind `chord` to `action`. `chord` is a [`KeyChord`] or a string that
    /// parses as one. Bind several chords to the same action for alternative
    /// keys; if a chord is bound twice, the first binding wins.
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

    /// Run the action bound to a key press or repeat in `event`, if any, and
    /// return [`Handled::Yes`]. Key releases, unbound keys and other events
    /// return [`Handled::No`], so they keep bubbling.
    pub fn dispatch(&self, receiver: &T, event: &Event) -> Handled {
        let Some(chord) = pressed(event) else {
            return Handled::No;
        };
        let Some((_, action)) = self.bindings.iter().find(|(bound, _)| *bound == chord) else {
            return Handled::No;
        };
        action(receiver);
        Handled::Yes
    }

    /// The bound chords, in insertion order, for help screens.
    ///
    /// ```
    /// use eventful_ratatui::Keymap;
    ///
    /// let keys = Keymap::<()>::new().bind("q", |_| {}).bind("ctrl+c", |_| {});
    /// let help: Vec<String> = keys.chords().map(|chord| chord.to_string()).collect();
    /// assert_eq!(help, ["q", "ctrl+c"]);
    /// ```
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
