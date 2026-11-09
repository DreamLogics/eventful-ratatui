//! Compile-time key chord parsing. Mirrors `KeyChord::from_str` and
//! `KeyChord::new` in eventful-ratatui; `tests/key_events.rs` there checks
//! that both agree.
use proc_macro2::TokenStream;
use quote::quote;

/// A normalized key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Code {
    Char(char),
    F(u8),
    /// A `KeyCode` variant without data.
    Named(&'static str),
}

/// crossterm `KeyModifiers` flag names, in bit order.
const MODIFIERS: [&str; 6] = ["SHIFT", "CONTROL", "ALT", "SUPER", "HYPER", "META"];
const SHIFT: u8 = 1;

/// A normalized key plus modifiers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Chord {
    code: Code,
    /// Bits in [`MODIFIERS`] order.
    modifiers: u8,
}

impl Chord {
    /// Normalize like `KeyChord::new`.
    fn new(code: Code, modifiers: u8) -> Self {
        let (code, modifiers) = match code {
            Code::Char(c) if modifiers & SHIFT != 0 || c.is_uppercase() => {
                (Code::Char(c.to_ascii_uppercase()), modifiers & !SHIFT)
            }
            Code::Named("Tab") if modifiers & SHIFT != 0 => {
                (Code::Named("BackTab"), modifiers & !SHIFT)
            }
            Code::Named("BackTab") => (code, modifiers & !SHIFT),
            code => (code, modifiers),
        };
        Self { code, modifiers }
    }

    /// Parse like `KeyChord::from_str`. Errors are the reason only.
    pub(crate) fn parse(chord: &str) -> Result<Self, &'static str> {
        let (prefix, key) = match chord.strip_suffix('+') {
            Some(prefix) if prefix.is_empty() || prefix.ends_with('+') => {
                (prefix.strip_suffix('+').unwrap_or(prefix), "+")
            }
            _ => chord.rsplit_once('+').unwrap_or(("", chord)),
        };
        let mut modifiers = 0;
        for name in prefix.split('+').filter(|_| !prefix.is_empty()) {
            let flag = match name.to_ascii_lowercase().as_str() {
                "shift" => "SHIFT",
                "ctrl" | "control" => "CONTROL",
                "alt" | "option" => "ALT",
                "super" | "cmd" => "SUPER",
                "hyper" => "HYPER",
                "meta" => "META",
                _ => return Err("unknown modifier"),
            };
            let bit = MODIFIERS.iter().position(|m| *m == flag).unwrap_or(0);
            modifiers |= 1 << bit;
        }
        let mut chars = key.chars();
        let code = match (chars.next(), chars.next()) {
            (None, _) => return Err("missing key"),
            (Some(c), None) => Code::Char(c),
            _ => match key.to_ascii_lowercase().as_str() {
                "enter" | "return" => Code::Named("Enter"),
                "esc" | "escape" => Code::Named("Esc"),
                "tab" => Code::Named("Tab"),
                "backtab" => Code::Named("BackTab"),
                "backspace" => Code::Named("Backspace"),
                "delete" | "del" => Code::Named("Delete"),
                "insert" | "ins" => Code::Named("Insert"),
                "home" => Code::Named("Home"),
                "end" => Code::Named("End"),
                "pageup" | "pgup" => Code::Named("PageUp"),
                "pagedown" | "pgdn" => Code::Named("PageDown"),
                "up" => Code::Named("Up"),
                "down" => Code::Named("Down"),
                "left" => Code::Named("Left"),
                "right" => Code::Named("Right"),
                "space" => Code::Char(' '),
                "plus" => Code::Char('+'),
                name => match name.strip_prefix('f').and_then(|n| n.parse().ok()) {
                    Some(n @ 1..=24) => Code::F(n),
                    _ => return Err("unknown key"),
                },
            },
        };
        Ok(Self::new(code, modifiers))
    }

    /// A const expression building the `KeyChord`, given the path of the
    /// hidden support module.
    pub(crate) fn to_tokens(self, private: &TokenStream) -> TokenStream {
        let code = match self.code {
            Code::Char(c) => quote!(#private::KeyCode::Char(#c)),
            Code::F(n) => quote!(#private::KeyCode::F(#n)),
            Code::Named(name) => {
                let variant = proc_macro2::Ident::new(name, proc_macro2::Span::call_site());
                quote!(#private::KeyCode::#variant)
            }
        };
        let flags = MODIFIERS
            .iter()
            .enumerate()
            .filter(|(bit, _)| self.modifiers & (1 << bit) != 0)
            .map(|(_, name)| proc_macro2::Ident::new(name, proc_macro2::Span::call_site()));
        quote! {
            #private::chord(#code, #private::KeyModifiers::NONE #(.union(#private::KeyModifiers::#flags))*)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_like_the_runtime() {
        assert_eq!(Chord::parse("J"), Chord::parse("shift+j"));
        assert_eq!(Chord::parse("shift+tab"), Chord::parse("backtab"));
        assert_eq!(Chord::parse("ctrl++"), Ok(Chord::new(Code::Char('+'), 2)));
        assert_eq!(Chord::parse("ctrl+x+"), Err("unknown modifier"));
        assert_eq!(Chord::parse("f25"), Err("unknown key"));
        assert_eq!(Chord::parse(""), Err("missing key"));
    }
}
