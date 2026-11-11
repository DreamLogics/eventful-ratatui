# Third-party notices

eventful-ratatui is licensed under MIT OR Apache-2.0 (see `LICENSE-MIT` and
`LICENSE-APACHE`). It builds on the work below.

## ratcn

<https://github.com/kristoferlund/ratcn>

The visual design of the components follows ratcn: fills instead of borders,
input wells that lift on focus and hover, half-block button caps, and a
palette of purpose-named colors from which state colors are derived. The
Neutral theme's dark colors, and the palette mapping and tuning of the
Catppuccin, Gruvbox, Nord and Tokyo Night themes in `eventful-ratatui/themes/`,
are taken from ratcn's themes, as are the 16-color RGB table and the blend
rounding in `eventful-ratatui/src/theme/color.rs`.

```text
MIT License

Copyright (c) 2026 Kristofer Lund

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## Color schemes

The theme files use the colors of these schemes, credited in each file:

- [shadcn/ui](https://github.com/shadcn-ui/ui) (MIT, Copyright (c) 2023
  shadcn): the neutral palette behind the Neutral theme, by way of ratcn.
- [Catppuccin](https://github.com/catppuccin/palette) (MIT, Copyright (c) 2021
  Catppuccin): Mocha and Latte.
- [Gruvbox](https://github.com/morhetz/gruvbox) (MIT/X11, by Pavel Pertsev):
  dark and light.
- [Nord](https://github.com/nordtheme/nord) (MIT, Copyright (c) 2016-present
  Sven Greb).
- [One Dark and One Light](https://github.com/atom/atom/tree/master/packages)
  (MIT, Copyright (c) 2016 GitHub Inc.): Atom's syntax and UI themes, behind
  the One theme.
- [Tokyo Night](https://github.com/folke/tokyonight.nvim) (Apache-2.0, by
  Folke Lemaitre): Night and Day.
