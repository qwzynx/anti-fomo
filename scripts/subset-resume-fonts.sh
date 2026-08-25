#!/usr/bin/env bash
# Subsets the résumé faces down to the characters a résumé actually uses.
#
# The PDF writer embeds whole font files — printpdf's runtime subsetter is a
# no-op unless the `text_layout` feature is on, and that feature drags in a
# whole layout/fontconfig stack we cannot ship to Android. So the subsetting
# happens here, once, and the results are committed.
#
# It matters: unsubsetted, Liberation Serif is 393 KB and one page of text came
# out a 397 KB PDF. Subsetted it is 59 KB and the same page is 63 KB. A résumé
# is a file you email to strangers.
#
# The same .ttf files are read twice at runtime — `include_bytes!` into the Rust
# binary for the PDF, and @font-face'd by the webview for the preview. That is
# deliberate: one file means the preview cannot measure text differently from
# the PDF. Do not replace one side with a .woff2.
#
# Coverage: ASCII, Latin-1 and Latin Extended-A (names are not all ASCII), plus
# the punctuation the layout emits itself — the bullet, both dashes, curly
# quotes and the ellipsis.
#
# Requires fonttools:  python3 -m venv .venv && .venv/bin/pip install fonttools
# Usage:               scripts/subset-resume-fonts.sh [path-to-pyftsubset]
set -euo pipefail

PYFTSUBSET="${1:-pyftsubset}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT/app/static/fonts/resume"

UNICODES='U+0020-007E,U+00A0-00FF,U+0100-017F,U+00B7,U+2013,U+2014,U+2018,U+2019,U+201C,U+201D,U+2022,U+2026,U+20AC,U+2122'

# Hinting is kept. It costs ~20 KB a face and it is what keeps the on-screen
# preview crisp at the ~15 px an 11 pt line renders at.
subset() {
  local src="$1" dst="$2"
  [ -f "$src" ] || { echo "missing source font: $src" >&2; exit 1; }
  "$PYFTSUBSET" "$src" \
    --output-file="$OUT/$dst" \
    --unicodes="$UNICODES" \
    --layout-features='' \
    --notdef-outline
  printf '  %-28s %6s KB\n' "$dst" "$(( $(stat -c%s "$OUT/$dst") / 1024 ))"
}

mkdir -p "$OUT"
echo "serif (Liberation Serif — metric-compatible with Times New Roman):"
LIB=/usr/share/fonts/liberation
subset "$LIB/LiberationSerif-Regular.ttf"    serif-regular.ttf
subset "$LIB/LiberationSerif-Bold.ttf"       serif-bold.ttf
subset "$LIB/LiberationSerif-Italic.ttf"     serif-italic.ttf
subset "$LIB/LiberationSerif-BoldItalic.ttf" serif-bolditalic.ttf

echo "sans (Inter — the same family the app's UI uses):"
INTER="${INTER_DIR:-$ROOT/.fonts/inter}"
subset "$INTER/Inter-Regular.ttf"    sans-regular.ttf
subset "$INTER/Inter-Bold.ttf"       sans-bold.ttf
subset "$INTER/Inter-Italic.ttf"     sans-italic.ttf
subset "$INTER/Inter-BoldItalic.ttf" sans-bolditalic.ttf

echo "done -> $OUT"
