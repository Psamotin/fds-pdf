# FDS PDF approved corporate icon

The original `FDS-PDF.png` is preserved byte for byte from the user-approved upload.
Source SHA256: `06a539c67d15487a52d9712f78f137d9bb5016fc5a4205b53b29809083510bed`.

`cargo xtask fds-icon` creates a 256px window PNG and a Windows ICO with
16, 24, 32, 48, 64, 128 and 256px frames. Lanczos resizing preserves aspect ratio
and pads to a square; it does not redraw or replace the supplied artwork.
`cargo xtask fds-icon --check` verifies deterministic regeneration.

The ICO is embedded by the Windows resource compiler and supplied in the portable
bundle for optional Start Menu shortcuts. The window PNG is embedded by the desktop
viewport. Upstream ArtCraft/PrintCraft icons are not selected.

All three assets are attributed in ATTRIBUTION.toml. Upstream notices remain intact.
