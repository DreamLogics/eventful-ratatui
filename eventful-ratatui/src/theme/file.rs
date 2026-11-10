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
/// The themes this crate ships are bundled with the library:
/// [`catppuccin`](Self::catppuccin), [`gruvbox`](Self::gruvbox),
/// [`neutral`](Self::neutral), [`nord`](Self::nord), [`one`](Self::one) and
/// [`tokyo_night`](Self::tokyo_night), or all of them from
/// [`bundled`](Self::bundled). Your own themes come from a file with
/// [`open`](Self::open), or from text with [`from_source`](Self::from_source).
///
/// Getting a theme file only reads its text; [`load`](Self::load) evaluates
/// the variant for an appearance. Hand it to
/// [`Tui::theme_file`](crate::Tui::theme_file) to load the variant matching
/// the terminal.
///
/// ```
/// use eventful_ratatui::theme::{Appearance, ThemeFile};
///
/// let light = ThemeFile::nord().load(Appearance::Light)?;
/// assert_eq!(light.name, "Nord Light");
/// assert_eq!(light.appearance(), Appearance::Light);
/// # Ok::<(), eventful_ratatui::theme::ThemeError>(())
/// ```
///
/// The shipped themes are also files in the crate's `themes/` directory.
/// They start with `#![schema("crate:eventful-ratatui@0.1")]`, which lets
/// `crol check` and the CROL language server check them; copy one to start a
/// theme.
#[derive(Clone, Debug)]
pub struct ThemeFile {
    /// Where the source came from, for messages.
    name: String,
    source: String,
}

impl ThemeFile {
    /// Read the theme file at `path`. The path names it in error messages.
    /// The contents are not checked until [`load`](Self::load).
    ///
    /// # Errors
    /// Returns the I/O error if the file cannot be read or is not UTF-8.
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref();
        Ok(Self::from_source(
            path.display().to_string(),
            std::fs::read_to_string(path)?,
        ))
    }

    /// A theme file from its text; `name` names it in error messages. For
    /// themes embedded with `include_str!` or fetched from elsewhere.
    ///
    /// ```
    /// use eventful_ratatui::{prelude::*, theme::{Appearance, ThemeFile}};
    ///
    /// let source = r##"
    ///     name: "Mono"
    ///     palette: {
    ///         background: "#101010", surface: "#181818", field: "#202020",
    ///         foreground: "#e0e0e0", muted: "#a0a0a0", primary: "#e0e0e0",
    ///         primary_foreground: "#101010", secondary: "#303030",
    ///         secondary_foreground: "#e0e0e0", accent: "white",
    ///         destructive: "#ff6060", destructive_foreground: "#101010",
    ///         success: "#60d060", warning: "#e0c040", info: "#60b0e0",
    ///         border: "#505050", ring: &self.primary, selection: "#404040",
    ///     }
    ///     <light> {
    ///         name: "Mono Light",
    ///         palette.background: "#fafafa",
    ///     }
    /// "##;
    /// let file = ThemeFile::from_source("mono.crod", source);
    /// let dark = file.load(Appearance::Dark)?;
    /// assert_eq!(dark.name, "Mono");
    /// assert_eq!(dark.palette.ring, Color::Rgb(0xe0, 0xe0, 0xe0));
    /// assert_eq!(file.load(Appearance::Light)?.name, "Mono Light");
    /// # Ok::<(), eventful_ratatui::theme::ThemeError>(())
    /// ```
    pub fn from_source(name: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            source: source.into(),
        }
    }

    /// Neutral: grays with a white or black primary, after
    /// [shadcn/ui](https://ui.shadcn.com), named "Neutral Dark" and "Neutral
    /// Light". Bundled; see [`bundled`](Self::bundled).
    ///
    /// ```
    /// use eventful_ratatui::theme::{Appearance, ThemeFile};
    ///
    /// let theme = ThemeFile::neutral().load(Appearance::Dark)?;
    /// assert_eq!(theme.name, "Neutral Dark");
    /// # Ok::<(), eventful_ratatui::theme::ThemeError>(())
    /// ```
    pub fn neutral() -> Self {
        Self::embedded(NEUTRAL)
    }

    /// [Catppuccin](https://catppuccin.com): "Catppuccin Mocha" when dark,
    /// "Catppuccin Latte" when light. Bundled; see
    /// [`bundled`](Self::bundled).
    ///
    /// ```
    /// use eventful_ratatui::{prelude::*, theme::ThemeFile};
    ///
    /// let tui = Tui::new().theme_file(ThemeFile::catppuccin());
    /// ```
    pub fn catppuccin() -> Self {
        Self::embedded(CATPPUCCIN)
    }

    /// [Gruvbox](https://github.com/morhetz/gruvbox): "Gruvbox Dark" and
    /// "Gruvbox Light". Bundled; see [`bundled`](Self::bundled).
    ///
    /// ```
    /// use eventful_ratatui::theme::{Appearance, ThemeFile};
    ///
    /// let theme = ThemeFile::gruvbox().load(Appearance::Light)?;
    /// assert_eq!(theme.name, "Gruvbox Light");
    /// # Ok::<(), eventful_ratatui::theme::ThemeError>(())
    /// ```
    pub fn gruvbox() -> Self {
        Self::embedded(GRUVBOX)
    }

    /// [Nord](https://www.nordtheme.com): "Nord" when dark, "Nord Light"
    /// when light. Bundled; see [`bundled`](Self::bundled).
    ///
    /// ```
    /// use eventful_ratatui::theme::{Appearance, ThemeFile};
    ///
    /// let theme = ThemeFile::nord().load(Appearance::Dark)?;
    /// assert_eq!(theme.name, "Nord");
    /// # Ok::<(), eventful_ratatui::theme::ThemeError>(())
    /// ```
    pub fn nord() -> Self {
        Self::embedded(NORD)
    }

    /// One, after Atom's themes of that name, which Zed and VS Code carry
    /// on: "One Dark" when dark, "One Light" when light. Bundled; see
    /// [`bundled`](Self::bundled).
    ///
    /// ```
    /// use eventful_ratatui::{prelude::*, theme::{Appearance, ThemeFile}};
    ///
    /// let dark = ThemeFile::one().load(Appearance::Dark)?;
    /// assert_eq!(dark.name, "One Dark");
    /// assert_eq!(dark.palette.field, Color::Rgb(0x28, 0x2c, 0x34));
    /// assert_eq!(ThemeFile::one().load(Appearance::Light)?.name, "One Light");
    /// # Ok::<(), eventful_ratatui::theme::ThemeError>(())
    /// ```
    pub fn one() -> Self {
        Self::embedded(ONE)
    }

    /// [Tokyo Night](https://github.com/folke/tokyonight.nvim): "Tokyo
    /// Night" when dark, "Tokyo Night Day" when light. Bundled; see
    /// [`bundled`](Self::bundled).
    ///
    /// ```
    /// use eventful_ratatui::theme::{Appearance, ThemeFile};
    ///
    /// let theme = ThemeFile::tokyo_night().load(Appearance::Light)?;
    /// assert_eq!(theme.name, "Tokyo Night Day");
    /// # Ok::<(), eventful_ratatui::theme::ThemeError>(())
    /// ```
    pub fn tokyo_night() -> Self {
        Self::embedded(TOKYO_NIGHT)
    }

    /// Every bundled theme, ordered by name: Catppuccin, Gruvbox, Neutral,
    /// Nord, One and Tokyo Night. For a theme picker; each is the same as
    /// its own constructor, like [`one`](Self::one).
    ///
    /// The themes are part of the library, so no files need to be installed
    /// with an app. Their [`name`](Self::name)s are their file names in the
    /// crate's `themes/` directory, like `"one.crod"`.
    ///
    /// ```
    /// use eventful_ratatui::theme::{Appearance, ThemeFile};
    ///
    /// let names: Vec<String> = ThemeFile::bundled()
    ///     .iter()
    ///     .map(|file| file.load(Appearance::Dark).map(|theme| theme.name))
    ///     .collect::<Result<_, _>>()?;
    /// assert_eq!(names[4], "One Dark");
    /// assert_eq!(names.len(), 6);
    /// # Ok::<(), eventful_ratatui::theme::ThemeError>(())
    /// ```
    pub fn bundled() -> [Self; 6] {
        BUNDLED.map(Self::embedded)
    }

    /// A bundled theme from its entry in [`BUNDLED`].
    fn embedded((name, source): (&str, &str)) -> Self {
        Self::from_source(name, source)
    }

    /// The name or path it was read from: a path for [`open`](Self::open),
    /// the given name for [`from_source`](Self::from_source), and the file
    /// name, like `"nord.crod"`, for a bundled theme.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Evaluate the variant for `appearance`: the values at the root, with
    /// the `<dark>` or `<light>` block applied on top. The theme gets the
    /// default symbols and rounded borders.
    ///
    /// # Errors
    /// Returns a [`ThemeError`] naming the file if it does not parse, the
    /// `name` or a palette field is missing, or a palette value is not a
    /// color (`"#rrggbb"` or a color name).
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

// The bundled themes: the files in `themes/`, as (file name, text). One
// list, so `bundled` and the tests cover exactly what ships; a test checks
// that no file in `themes/` is missing from it.
const CATPPUCCIN: (&str, &str) = (
    "catppuccin.crod",
    include_str!("../../themes/catppuccin.crod"),
);
const GRUVBOX: (&str, &str) = ("gruvbox.crod", include_str!("../../themes/gruvbox.crod"));
const NEUTRAL: (&str, &str) = ("neutral.crod", include_str!("../../themes/neutral.crod"));
const NORD: (&str, &str) = ("nord.crod", include_str!("../../themes/nord.crod"));
const ONE: (&str, &str) = ("one.crod", include_str!("../../themes/one.crod"));
const TOKYO_NIGHT: (&str, &str) = (
    "tokyo-night.crod",
    include_str!("../../themes/tokyo-night.crod"),
);

/// Every bundled theme, ordered by name.
pub(super) const BUNDLED: [(&str, &str); 6] =
    [CATPPUCCIN, GRUVBOX, NEUTRAL, NORD, ONE, TOKYO_NIGHT];

/// A theme file that could not be loaded. Displays as
/// `theme <file>: <what went wrong>`, naming the field where it can.
///
/// ```
/// use eventful_ratatui::theme::{Appearance, ThemeFile};
///
/// let err = ThemeFile::from_source("short.crod", r#"name: "Short""#)
///     .load(Appearance::Dark)
///     .unwrap_err();
/// assert!(err.to_string().starts_with("theme short.crod:"));
/// ```
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
        for (name, source) in BUNDLED {
            for variant in [None, Some("dark"), Some("light")] {
                let mut composer = CrolComposer::with_cfg(CrolConfig {
                    variants: CrolVariants::new().add_grouped(["dark", "light"]),
                    ..Default::default()
                })
                .schema(schema.clone())
                .load_layer(CrolLayer::from_sources(
                    "theme",
                    [(name.to_owned(), source.to_owned())],
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

    /// Every file in `themes/` is bundled, so a new theme file cannot be
    /// left out of `bundled` and the tests.
    #[test]
    fn every_shipped_file_is_bundled() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/themes");
        let mut files: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.ends_with(".crod"))
            .collect();
        files.sort();
        let bundled: Vec<&str> = BUNDLED.iter().map(|(name, _)| *name).collect();
        assert_eq!(files, bundled);
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
