# Résumé faces

Eight subset faces, two families, used by **both** the PDF writer and the preview.

| File | Source | Licence |
| --- | --- | --- |
| `serif-*.ttf` | Liberation Serif — metric-compatible with Times New Roman | SIL OFL 1.1 |
| `sans-*.ttf` | Inter 4.1 (`extras/ttf`) — the family the app's UI already uses | SIL OFL 1.1 |

Regenerate with `scripts/subset-resume-fonts.sh`; that script documents the
coverage and why the subsetting is an authoring step rather than a runtime one.
Liberation comes from the system (`/usr/share/fonts/liberation`); Inter is not
vendored in source form — download the release zip and point `INTER_DIR` at its
`extras/ttf`.

These files are read twice at runtime: `include_bytes!` into the Rust binary so
the PDF can embed them, and `@font-face` in `app.css` so the preview can draw
with them. One file, so the preview cannot measure text differently from the
PDF. Replacing either side with a `.woff2` would break that guarantee.
