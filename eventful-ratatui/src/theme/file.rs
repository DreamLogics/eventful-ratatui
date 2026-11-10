//! [`ThemeFile`]: themes in CROL data files, with a `<dark>` and a `<light>`
//! variant.
use super::{Appearance, Palette, Theme};
use crol::{CrolComposer, CrolConfig, CrolLayer, CrolVariants};
use ratatui::style::Color;
use std::{fmt, io, path::Path, str::FromStr};

/// A theme in a [CROL](https://crates.io/crates/crol) data file (`.crod`),
/// with a `<dark>` and a `<light>` variant. See [the module
/// docs](super#theme-files) for the format.
///
/// Opening a file only reads it; [`load`](Self::load) evaluates the variant
/// for an appearance. Hand it to [`Tui::theme_file`](crate::Tui::theme_file)
/// to load the variant matching the terminal.
#[derive(Clone, Debug)]
pub struct ThemeFile {
    /// Where the source came from, for messages.
    name: String,
    source: String,
}

impl ThemeFile {
    /// Read the theme file at `path`.
    ///
    /// # Errors
    /// Fails if the file cannot be read.
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref();
        Ok(Self::from_source(
            path.display().to_string(),
            std::fs::read_to_string(path)?,
        ))
    }

    /// A theme file from its text; `name` names it in error messages.
    pub fn from_source(name: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            source: source.into(),
        }
    }

    /// The name or path it was read from.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Evaluate the variant for `appearance`.
    ///
    /// # Errors
    /// Fails if the file does not parse, a palette field is missing, or a
    /// color is not a color.
    pub fn load(&self, appearance: Appearance) -> Result<Theme, ThemeError> {
        let variants = CrolVariants::new().add_grouped(["dark", "light"]);
        // Data mode: variants need layered data, so the file is one layer.
        let document = format!("{}.crod", self.name.trim_end_matches(".crod"));
        let file: format::Theme = CrolComposer::with_cfg(CrolConfig {
            variants,
            ..Default::default()
        })
        .load_layer(CrolLayer::from_sources(
            "theme",
            [(document, self.source.clone())],
        ))
        .with_variant(appearance.variant())
        .and_then(CrolComposer::compose)
        .map_err(|err| ThemeError::new(&self.name, err.to_string()))?;
        let palette = file
            .palette
            .into_palette()
            .map_err(|message| ThemeError::new(&self.name, message))?;
        Ok(Theme::new(file.name, palette))
    }
}

/// A theme file that could not be loaded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThemeError {
    message: String,
}

impl ThemeError {
    fn new(file: &str, message: impl fmt::Display) -> Self {
        Self {
            message: format!("theme {file}: {message}"),
        }
    }
}

impl fmt::Display for ThemeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ThemeError {}

/// The format of theme files, as serde reads it and as the schema describes
/// it.
mod format {
    use super::{Palette as Colors, parse};
    use serde::Deserialize;

    /// A theme for eventful-ratatui. Write the values shared by both variants
    /// at the root, and what differs in `<dark> { … }` and `<light> { … }`.
    #[derive(Deserialize)]
    #[cfg_attr(test, derive(crol::Schema))]
    pub(super) struct Theme {
        /// A display name, for a theme picker.
        pub(super) name: String,
        /// The base colors; every state color is derived from these.
        pub(super) palette: Palette,
    }

    /// Declare the palette as the file spells it, colors as text, and its
    /// conversion.
    macro_rules! palette {
        ($($(#[doc = $doc:literal])* $field:ident,)*) => {
            /// The base colors of a theme, each `"#rrggbb"` or a color name.
            #[derive(Deserialize)]
            #[cfg_attr(test, derive(crol::Schema))]
            pub(super) struct Palette {
                $($(#[doc = $doc])* $field: String,)*
            }

            impl Palette {
                pub(super) fn into_palette(self) -> Result<Colors, String> {
                    Ok(Colors {
                        $($field: parse(stringify!($field), &self.$field)?,)*
                    })
                }
            }
        };
    }

    palette! {
        /// The screen, behind everything.
        background,
        /// Raised layers: dialogs, menus, toasts.
        surface,
        /// Wells that controls sit in: inputs, lists, tracks.
        field,
        /// Body text.
        foreground,
        /// Secondary text: hints, placeholders, captions.
        muted,
        /// The main accent: main buttons, the selected tab, progress.
        primary,
        /// Text on `primary`.
        primary_foreground,
        /// Quieter fills: ordinary buttons, chips.
        secondary,
        /// Text on `secondary`.
        secondary_foreground,
        /// A vivid signature color for occasional emphasis.
        accent,
        /// Destructive actions and errors.
        destructive,
        /// Text on `destructive`.
        destructive_foreground,
        /// Success.
        success,
        /// Warnings.
        warning,
        /// Neutral information.
        info,
        /// Borders and separators.
        border,
        /// The border of a container holding the focus.
        ring,
        /// The background of selected text.
        selection,
    }
}

/// A color as a theme file writes it: `#rrggbb` or a color name.
fn parse(field: &str, text: &str) -> Result<Color, String> {
    Color::from_str(text).map_err(|_| format!("palette.{field}: {text:?} is not a color"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PALETTE: &str = r##"
        background: "#000000", surface: "#111111", field: "#222222",
        foreground: "#ffffff", muted: "#aaaaaa", primary: "#3366ff",
        primary_foreground: "#ffffff", secondary: "#333333",
        secondary_foreground: "#ffffff", accent: "magenta",
        destructive: "#ff3333", destructive_foreground: "#000000",
        success: "#33ff33", warning: "#ffff33", info: "#33ffff",
        border: "#666666", ring: &self.primary, selection: "#223366",
    "##;

    /// The schema tools like `crol check` and the language server use for
    /// theme files. Fails when it is out of date; `CROL_UPDATE_SCHEMA=1 cargo
    /// test` updates it.
    #[test]
    fn the_schema_file_is_up_to_date() {
        crol::schema::check_file::<format::Theme>("themes/theme.schema.crol").unwrap();
    }

    /// Every shipped theme file passes the schema without a variant and in
    /// each one, as `crol check` and the language server see it.
    #[test]
    fn shipped_files_pass_the_schema() {
        let schema = crol::schema::build::<format::Theme>();
        for (name, source) in crate::theme::tests::FILES {
            for variant in [None, Some("dark"), Some("light")] {
                let mut composer = CrolComposer::with_cfg(CrolConfig {
                    variants: CrolVariants::new().add_grouped(["dark", "light"]),
                    ..Default::default()
                })
                .schema(schema.clone())
                .load_layer(CrolLayer::from_sources(
                    "theme",
                    [(format!("{name}.crod"), source.to_owned())],
                ));
                if let Some(variant) = variant {
                    composer = composer.with_variant(variant).unwrap();
                }
                let data = composer.compose_data().unwrap();
                let diagnostics = crol::check(&data, &schema, None, &Default::default());
                let messages: Vec<_> = diagnostics.into_iter().map(|d| d.message).collect();
                assert_eq!(messages, Vec::<String>::new(), "{name} {variant:?}");
            }
        }
    }

    #[test]
    fn variants_pick_the_palette() {
        let source = format!(
            "name: \"Test\"\npalette: {{ {PALETTE} }}\n\
             <light> {{ name: \"Test Light\", palette.background: \"#ffffff\" }}"
        );
        let file = ThemeFile::from_source("test", source);
        let dark = file.load(Appearance::Dark).unwrap();
        assert_eq!(dark.name, "Test");
        assert_eq!(dark.palette.background, Color::Rgb(0, 0, 0));
        assert_eq!(
            dark.palette.ring,
            Color::Rgb(0x33, 0x66, 0xff),
            "references work"
        );
        assert_eq!(dark.palette.accent, Color::Magenta, "names work");
        let light = file.load(Appearance::Light).unwrap();
        assert_eq!(light.name, "Test Light");
        assert_eq!(light.palette.background, Color::Rgb(255, 255, 255));
    }

    #[test]
    fn errors_name_the_file_and_the_field() {
        let source = format!("name: \"Bad\"\npalette: {{ {PALETTE} }}\npalette.muted: \"nope\"");
        let err = ThemeFile::from_source("bad.crod", source)
            .load(Appearance::Dark)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("bad.crod") && err.contains("palette.muted"),
            "{err}"
        );
        let err = ThemeFile::from_source("short", "name: \"Short\"")
            .load(Appearance::Dark)
            .unwrap_err()
            .to_string();
        assert!(err.starts_with("theme short:"), "{err}");
    }
}
