# eventful-ratatui

A Cargo workspace: `eventful-ratatui` (the library), `eventful-ratatui-macros`
(its proc macros, re-exported by the library) and example apps in `examples/`.

## Documentation is part of the API

Good documentation is a hard requirement, not a finishing touch. A change that
adds or changes public API without documenting it is incomplete. Treat it like
a failing test.

Every public item (module, type, trait, function, method, field, variant,
constant, macro) must have docs that tell a reader who has not seen the source:

- **What it is or does**, in a short first paragraph: it becomes the summary
  in rustdoc's item lists. Details go in later paragraphs.
- **How it behaves**: defaults, units, ranges, what happens on edge cases,
  and how it interacts with focus, mouse, overlays, theming or redraws where
  relevant.
- **`# Errors`** for every function returning `Result`, and **`# Panics`** for
  every function that can panic. If a panic is impossible, restructure the
  code or use `#[expect(clippy::missing_panics_doc, reason = "…")]`.
- **An example** for every module, every component, and every type or
  function a user is expected to call directly. Examples are doctests: they
  must compile, and should run and assert something (use
  `testing::Harness` to show rendering and input). Use `no_run` only when the
  example needs a terminal or an async context, and hide boilerplate with `#`.
- **Links** to related items with intra-doc links, e.g. [`Theme`](crate::theme::Theme).

Style: plain, precise sentences, matching the existing docs. Use
`use eventful_ratatui::prelude::*;` in examples. Keep README.md, CHANGELOG.md
and the crate-level docs in `lib.rs` in step with the API.

Private items need a comment wherever the why is not obvious from the code;
internal modules get a `//!` doc saying what they are responsible for.

### Enforcement

`missing_docs` is denied in both crates, and `lib.rs` warns on the clippy and
rustdoc doc lints (`missing_errors_doc`, `missing_panics_doc`, `doc_markdown`,
`too_long_first_doc_paragraph`, …). CI runs with `-D warnings`, so these fail
the build. Example coverage cannot be checked on stable; check it with:

```sh
RUSTDOCFLAGS="-Z unstable-options --show-coverage" \
  cargo +nightly doc --no-deps -p eventful-ratatui --all-features
```

## Checks

Run before committing; CI runs the same with `-D warnings`:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets
cargo test --workspace
cargo doc --workspace --no-deps
```

The toolchain is pinned to 1.99.0 (the MSRV); the macro UI tests compare
compiler output with snapshots.
