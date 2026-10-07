# FDS-PDF-R1 — corporate desktop release candidate

Product: **ФДС ПДФ**, desktop version **0.2.0-fds.1**.
Branch: `fds-pdf-r1-branding-ru`.
Windows toolchain: Rust **1.95.0**; retain the validated Windows D3D12/OpenGL configuration.

## Scope and compatibility

This release changes desktop presentation and the dedicated Windows build workflow.
PDF engine, OCR implementation/models, opening/saving code paths, internal command IDs,
crate names and upstream settings/recovery paths stay compatible. Library/workspace versions
remain `0.2.0`; only the desktop package version changes. Cargo.lock changes only that package entry.

The window title is always **ФДС ПДФ**. Document names/titles remain in document tabs.
Home, About, menus and the command palette do not advertise ArtCraft/Discord/upstream community links.
Upstream attribution remains in About and in LICENSE-MIT, LICENSE-APACHE, NOTICE and ATTRIBUTION.

## Localization architecture

- `crates/ui-egui/src/resources.rs` is the source of truth: frozen `ui.*` keys, English source labels,
  English fallback, Russian copy, and a `formatted` flag for messages with arguments.
- `i18n::text(key)` resolves desktop-owned copy. Russian is the default for new preferences;
  existing English/Japanese preferences are retained. Unknown keys remain visible for diagnosis.
- An immutable source index is built once for fast catalogue lookup in large panels.
- `Language::tr` is a presentation adapter for upstream catalogue/enum labels. It must never be
  applied to document text, file paths, PDF dictionary keys, command IDs or user-created names.
- `Language::command_label` translates history labels for display while preserving canonical
  English labels inside the engine. PDF Named Actions such as `Print` remain English.
- Locale scopes restore the prior locale after each app call/frame. `i18n::in_locale` carries it
  into background jobs. No process-wide mutable language singleton is used.
- `messages.rs` is generated from the resource table. `msg!` compiles both locale format strings,
  checking argument names/types/format specifiers at each call site. Do not edit it by hand.
- Add a resource without renaming an existing key, use `text` or `msg!` at the presentation boundary,
  then run `cargo run --locked -p printcraft-ui-egui --example l10n -- --write`.
  Verify with the same command ending in `--check`, plus formatting and localization tests.
- Existing upstream UI tests explicitly use the English fallback; `tests/localization.rs` and
  `tests/links.rs` exercise actual Russian screens and the corporate branding.

## Windows artifact

`.github/workflows/fds-windows-build.yml` runs for main, the R1 branch, pull requests and manual dispatch.
It checks formatting/version consistency, tests the desktop/UI, and builds with `--release --locked`.

Keep the Cargo binary **printcraft** and copy the result to **FDS-PDF.exe** when packaging.
This avoids changing upstream build scripts, internal tooling and package names.
Artifact: **FDS-PDF-0.2.0-fds.1-windows-x64**. The archive retains root licence/notice/attribution
files and all existing licence files referenced by ATTRIBUTION.toml. Windows version resources
identify ФДС ПДФ and 0.2.0-fds.1; their numeric version is the standard Cargo-derived 0.2.0.0.

The dedicated FDS workflow produces the R1 candidate; upstream multi-platform release/packaging
workflows retain their upstream identities and are not the R1 publication path.

## Corporate icon

`assets/fds-pdf/README.md` documents the approved icon slot. No ArtCraft/PrintCraft/eFrame logo
is selected by the corporate desktop build. Until a licensed FDS icon is supplied, use a neutral
system icon. A later asset-only change can add `fds-pdf.ico` and an attributed PNG.
Do not introduce unlicensed artwork or modify the upstream licence/notice files.

## Validation commands

```sh
cargo fmt --all -- --check
cargo check --locked -p printcraft -p printcraft-ui-egui
cargo test --locked -p printcraft-ui-egui -p printcraft
cargo run --locked -p printcraft-ui-egui --example l10n -- --check
cargo build --release --locked -p printcraft
```

In the prepared Linux cloud instance, first source `/workspace/.setup/environment.sh` for Rust 1.95
and the local, signature-verified Debian development libraries. No desktop display is required for
UI tests. The existing `shot` example can render a local review image through Mesa software graphics.
The Windows build must be executed by the Windows Actions runner; a Linux build/lint is not evidence
of a newly built Windows artifact. The user-provided Windows main baseline remains PASS.

## Recorded validation (Rust 1.95.0, Linux cloud)

- Formatting, locked desktop/UI check and affected-crate Clippy with `-D warnings`: PASS.
- Full workspace tests: **708 passed, 0 failed, 4 ignored**. Affected desktop/UI suites: **196 passed, 0 failed, 1 ignored**.
- Resource generator consistency: PASS; **1769** stable resources and **189** compile-time checked format messages.
- WASM check: **27 crates PASS**; dependency layering: **31 crates PASS**.
- Asset audit: PASS, with all licence/notice/attribution files unchanged.
- Headless Russian Home visual review: PASS; long card copy is bounded and search copy is truncated with a full tooltip.
- Dedicated Windows workflow: actionlint **1.7.7 PASS**.
- `cargo build --release --locked -p printcraft`: **PASS**; the built binary reports `ФДС ПДФ 0.2.0-fds.1`.
- Repeatable environment installer: **PASS**; setup/start instructions are saved as an environment configuration draft, not published.
- The full workspace Clippy gate stops on the existing `collapsible_match` warning in unchanged
  `crates/optimize/src/images.rs:57`. The engine is deliberately left unchanged for R1.
  This baseline failure is recorded rather than suppressed or reported as a green workspace gate.

## Known limits

Native OS file-picker buttons follow the OS language. Technical error details originating in the
PDF engine, JavaScript, the OS or third-party libraries may retain their original diagnostic text
inside a localized message; document contents and generated file names are deliberately not translated.
The RGB color picker uses a small localized desktop popup while retaining the PDF linear-RGB values.
The corporate icon and Windows visual acceptance require the approved corporate asset and a Windows runner/user session.
No release is published by these changes. English/Japanese options are retained for upstream compatibility;
Japanese remains the original partial translation with English fallback.

## Next step

FDS-PDF-R2: интеграция локального OCRmyPDF + Tesseract в существующий Scan & OCR UI.
