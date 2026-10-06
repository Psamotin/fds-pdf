# FDS PDF corporate icon slot

No ArtCraft or PrintCraft logo is used by the FDS desktop build.
Until an approved corporate asset is supplied, the executable uses the operating system's neutral icon.

To add the corporate icon in a separate asset task:
- Supply an original, licensed FDS icon and its source/license evidence.
- Add `assets/fds-pdf/fds-pdf.ico` for Windows resources.
- Add an approved PNG and wire it into the desktop ViewportBuilder for Linux/macOS.
- Add each file, license and SHA-256 to ATTRIBUTION.toml and run `cargo xtask assets --write` then `cargo xtask assets`.
- Keep LICENSE, NOTICE and upstream attribution intact.

No placeholder binary or unlicensed artwork is shipped.
