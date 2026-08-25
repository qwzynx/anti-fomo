//! Text measurement and line breaking.
//!
//! Pure, and the reason the preview can claim to be the PDF. Every width here
//! is the sum of the glyph advances printpdf will write into the font's `/W`
//! array, read out of the same [`ParsedFont`](printpdf::ParsedFont) it will
//! embed — so a line that fits here fits there, and a line the layout broke in
//! two breaks in the same place in both backends.
//!
//! No kerning and no shaping. That is not an approximation of what the PDF
//! does, it is exactly what the PDF does: with printpdf's `text_layout`
//! feature off, a `ShowText` run maps codepoints straight through the cmap with
//! no GPOS pass, so applying kerning while measuring would make the measurement
//! wrong rather than better. (`scripts/subset-resume-fonts.sh` drops the layout
//! tables for the same reason.)

use super::theme::{FamilyId, FontStyle};

/// Width of `text` at `size` points.
pub fn width(text: &str, family: FamilyId, style: FontStyle, size: f32) -> f32 {
    let font = super::fonts::face(family, style);
    let upem = f32::from(font.units_per_em.max(1));
    let units: u32 = text
        .chars()
        .map(|c| {
            let gid = font.lookup_glyph_index(c as u32).unwrap_or(0);
            u32::from(font.get_glyph_width(gid).unwrap_or(0))
        })
        .sum();
    units as f32 / upem * size
}

/// Greedy line breaking at spaces, to `max_width` points.
///
/// Greedy rather than Knuth-Plass: a résumé bullet is two or three lines and
/// nobody has ever looked at one and wished the raggedness were better
/// balanced. A word longer than the column is not broken mid-word — it
/// overhangs, which is visible and therefore fixable, where a silent hyphenless
/// split reads as corruption.
pub fn wrap(
    text: &str,
    family: FamilyId,
    style: FontStyle,
    size: f32,
    max_width: f32,
) -> Vec<String> {
    let text = text.trim();
    if text.is_empty() {
        return Vec::new();
    }
    if width(text, family, style, size) <= max_width {
        return vec![text.to_string()];
    }

    let space = width(" ", family, style, size);
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    let mut line_width = 0.0_f32;

    for word in text.split_whitespace() {
        let w = width(word, family, style, size);
        let extra = if line.is_empty() { w } else { space + w };
        if !line.is_empty() && line_width + extra > max_width {
            lines.push(std::mem::take(&mut line));
            line_width = 0.0;
        }
        if !line.is_empty() {
            line.push(' ');
            line_width += space;
        }
        line.push_str(word);
        line_width += w;
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

/// Shortens `text` until it fits, ending in an ellipsis.
///
/// Only used where a line physically cannot wrap — the right-hand date column,
/// a contact line. Everywhere else, wrapping is the right answer.
pub fn ellipsize(
    text: &str,
    family: FamilyId,
    style: FontStyle,
    size: f32,
    max_width: f32,
) -> String {
    if width(text, family, style, size) <= max_width {
        return text.to_string();
    }
    let ellipsis = '…';
    let dots = width("…", family, style, size);
    let mut out = String::new();
    let mut used = 0.0_f32;
    for ch in text.chars() {
        let w = width(&ch.to_string(), family, style, size);
        if used + w + dots > max_width {
            break;
        }
        out.push(ch);
        used += w;
    }
    out.push(ellipsis);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SERIF: FamilyId = FamilyId::Serif;
    const R: FontStyle = FontStyle::Regular;

    /// Anchored against the renderer: poppler reports "MAHAN QWZYNX" set in
    /// Liberation Serif at 14 pt spanning 118.552 pt, and the advance sum is
    /// 118.60 — the gap is the last glyph's ink stopping short of its advance,
    /// not a measurement error. If this drifts, the fonts were re-subsetted
    /// with different metrics and every layout in the app moved with them.
    #[test]
    fn width_matches_the_renderer() {
        let w = width("MAHAN QWZYNX", SERIF, R, 14.0);
        assert!((w - 118.60).abs() < 0.1, "expected ~118.60pt, got {w}");
    }

    #[test]
    fn width_scales_with_size() {
        let ten = width("Software Engineer", SERIF, R, 10.0);
        let twenty = width("Software Engineer", SERIF, R, 20.0);
        assert!((twenty - ten * 2.0).abs() < 0.01);
    }

    #[test]
    fn bold_is_wider_than_regular() {
        let regular = width("Responsibilities", SERIF, FontStyle::Regular, 11.0);
        let bold = width("Responsibilities", SERIF, FontStyle::Bold, 11.0);
        assert!(
            bold > regular,
            "bold {bold} should exceed regular {regular}"
        );
    }

    #[test]
    fn empty_text_has_no_lines() {
        assert!(wrap("   ", SERIF, R, 11.0, 300.0).is_empty());
    }

    #[test]
    fn short_text_is_one_line() {
        assert_eq!(
            wrap("Built a parser", SERIF, R, 11.0, 400.0),
            vec!["Built a parser"]
        );
    }

    #[test]
    fn every_wrapped_line_fits() {
        let text = "Cut p95 latency 40% by replacing the N+1 query path with a single \
                    windowed aggregate, then held the budget with a regression test that \
                    fails the build when the query count moves.";
        let max = 260.0;
        let lines = wrap(text, SERIF, R, 10.5, max);
        assert!(
            lines.len() > 2,
            "expected several lines, got {}",
            lines.len()
        );
        for line in &lines {
            let w = width(line, SERIF, R, 10.5);
            assert!(w <= max, "line {line:?} is {w}pt, over the {max}pt column");
        }
    }

    /// Wrapping must not eat or duplicate words — a résumé bullet losing a word
    /// on its way to the page is the worst kind of silent bug.
    #[test]
    fn wrapping_preserves_every_word() {
        let text = "Shipped a Rust service handling 2,000 requests per second on a single core";
        let lines = wrap(text, SERIF, R, 11.0, 120.0);
        assert_eq!(lines.join(" "), text);
    }

    #[test]
    fn overlong_word_overhangs_rather_than_splitting() {
        let lines = wrap("Supercalifragilistic", SERIF, R, 11.0, 10.0);
        assert_eq!(lines, vec!["Supercalifragilistic"]);
    }

    #[test]
    fn ellipsize_fits_and_marks_the_cut() {
        let out = ellipsize("Senior Software Engineering Intern", SERIF, R, 10.0, 60.0);
        assert!(out.ends_with('…'));
        assert!(width(&out, SERIF, R, 10.0) <= 60.0);
    }

    #[test]
    fn ellipsize_leaves_short_text_alone() {
        assert_eq!(ellipsize("Intern", SERIF, R, 10.0, 200.0), "Intern");
    }
}
