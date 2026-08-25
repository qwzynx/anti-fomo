//! The résumé faces, compiled into the binary.
//!
//! `include_bytes!` rather than a file read: the same bytes have to be there on
//! a phone, where there is no working directory to resolve a path against and
//! no guarantee an asset survived the packaging step. They are small enough to
//! carry — `scripts/subset-resume-fonts.sh` cuts each face to the characters a
//! résumé uses, which took Liberation Serif from 393 KB to 59 KB.
//!
//! The files live under `app/static/` rather than next to this module because
//! the webview serves the *same* files to `@font-face` for the preview. One
//! copy on disk is what makes the preview and the PDF agree on how wide a line
//! is; two copies would be two things to keep in sync, and the failure would be
//! silent — a preview that wraps one word earlier than the page it claims to be.

use std::collections::HashMap;
use std::sync::OnceLock;

use printpdf::ParsedFont;

use super::theme::{FamilyId, FontStyle};

macro_rules! face {
    ($path:literal) => {
        include_bytes!(concat!("../../../static/fonts/resume/", $path))
    };
}

/// Every face, keyed the way a [`Draw`](super::layout::Draw) names one.
///
/// The web font stack in `app.css` mirrors this table; a face added here needs
/// a matching `@font-face` there or the preview silently falls back to a system
/// font and stops matching the PDF.
pub const FACES: &[(FamilyId, FontStyle, &[u8])] = &[
    (
        FamilyId::Serif,
        FontStyle::Regular,
        face!("serif-regular.ttf"),
    ),
    (FamilyId::Serif, FontStyle::Bold, face!("serif-bold.ttf")),
    (
        FamilyId::Serif,
        FontStyle::Italic,
        face!("serif-italic.ttf"),
    ),
    (
        FamilyId::Serif,
        FontStyle::BoldItalic,
        face!("serif-bolditalic.ttf"),
    ),
    (
        FamilyId::Sans,
        FontStyle::Regular,
        face!("sans-regular.ttf"),
    ),
    (FamilyId::Sans, FontStyle::Bold, face!("sans-bold.ttf")),
    (FamilyId::Sans, FontStyle::Italic, face!("sans-italic.ttf")),
    (
        FamilyId::Sans,
        FontStyle::BoldItalic,
        face!("sans-bolditalic.ttf"),
    ),
];

/// Raw bytes for one face, for the PDF writer to embed.
pub fn bytes(family: FamilyId, style: FontStyle) -> &'static [u8] {
    FACES
        .iter()
        .find(|(f, s, _)| *f == family && *s == style)
        .map(|(_, _, b)| *b)
        .unwrap_or(FACES[0].2)
}

/// Every face parsed once, on first use.
///
/// Parsing walks the cmap into a `BTreeMap` of a few hundred entries per face;
/// doing that per layout pass would be paid on every keystroke in the builder,
/// because the preview relays out as you type.
fn parsed() -> &'static HashMap<(FamilyId, FontStyle), ParsedFont> {
    static PARSED: OnceLock<HashMap<(FamilyId, FontStyle), ParsedFont>> = OnceLock::new();
    PARSED.get_or_init(|| {
        FACES
            .iter()
            .filter_map(|(family, style, bytes)| {
                let mut warnings = Vec::new();
                let font = ParsedFont::from_bytes(bytes, 0, &mut warnings)?;
                for w in &warnings {
                    log::warn!("résumé face {family:?}/{style:?}: {}", w.message);
                }
                Some(((*family, *style), font))
            })
            .collect()
    })
}

/// One parsed face. Falls back to the serif regular, which is always present:
/// a missing face should shift a line, not lose it.
pub fn face(family: FamilyId, style: FontStyle) -> &'static ParsedFont {
    let table = parsed();
    table
        .get(&(family, style))
        .or_else(|| table.get(&(FamilyId::Serif, FontStyle::Regular)))
        .expect("the serif regular face parses")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_face_parses() {
        assert_eq!(parsed().len(), FACES.len(), "a face failed to parse");
    }

    #[test]
    fn faces_carry_metrics() {
        for (family, style, _) in FACES {
            let font = face(*family, *style);
            assert!(
                font.units_per_em > 0,
                "{family:?}/{style:?} has no units_per_em"
            );
            assert!(
                !font.codepoint_to_glyph.is_empty(),
                "{family:?}/{style:?} has no cmap — every glyph would render as .notdef"
            );
            assert!(
                !font.glyph_widths.is_empty(),
                "{family:?}/{style:?} has no advances"
            );
        }
    }

    /// The subsetting script's coverage list, spot-checked. A face that lost the
    /// bullet or the en dash still renders, but drops the character silently.
    #[test]
    fn faces_cover_what_the_layout_emits() {
        for (family, style, _) in FACES {
            let font = face(*family, *style);
            for ch in ['•', '–', '—', '’', '·', 'É', 'ł', '@', '%', '&'] {
                assert!(
                    font.lookup_glyph_index(ch as u32).is_some_and(|g| g != 0),
                    "{family:?}/{style:?} cannot draw {ch:?}"
                );
            }
        }
    }
}
