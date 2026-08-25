//! Positioned boxes to a PDF file.
//!
//! The only module in the feature that knows what a PDF is, and it does no
//! layout — [`layout`](super::layout) already decided where everything goes,
//! and doing any of that thinking twice is how a preview stops matching its
//! file.
//!
//! Two things here are not obvious and both were found the hard way:
//!
//! 1. **Text is positioned with `Tm`, never `Td`.** printpdf's `SetTextCursor`
//!    emits `Td`, which PDF defines as a move *relative to the start of the
//!    previous line*. Absolute coordinates fed through it accumulate: the
//!    second line of a résumé landed at twice its intended offset and the third
//!    ran off the page entirely. The page still *looked* fine to printpdf's own
//!    `extract_text`, which replays ops rather than reading the file back, so
//!    the bug survived a passing round-trip check. `pdftotext` saw one line out
//!    of three. `SetTextMatrix` emits `Tm`, which replaces the matrix outright.
//!
//! 2. **Extraction is verified against a real PDF reader**, not against
//!    printpdf. `resume_check` runs `pdftotext` over the output, because an
//!    applicant tracking system runs something like it and a résumé it cannot
//!    read is worse than no résumé at all.
//!
//! 3. **Every rectangle states its paint mode.** `Rect::from_xywh` leaves
//!    `mode: None`, and printpdf serializes that as `re` followed by `n` —
//!    "append the path, paint nothing". The rules under the section headings
//!    and the Banner theme's tinted bands were all being emitted and all
//!    invisible, and nothing caught it: the geometry tests check where the
//!    boxes are, the extraction check reads text, and neither looks at ink.
//!    Rendering a page to an image and looking at it did.
//!
//! Each run is drawn in its own `BT`/`ET` with one `Tm`. Slightly more verbose
//! than holding a text section open, and it means no run can inherit a stale
//! matrix from the one before it.

use printpdf::{
    Actions, Color, LinkAnnotation, Mm, Op, PaintMode, ParsedFont, PdfDocument, PdfFontHandle,
    PdfPage, PdfSaveOptions, Pt, Rect, Rgb as PdfRgb, TextItem, TextMatrix, WindingOrder,
};

use super::fonts;
use super::layout::{Draw, LaidOut, Page};
use super::theme::{FamilyId, FontStyle, Rgb};

/// Renders laid-out pages to PDF bytes.
///
/// `title` becomes the document title, which is what a reader shows in its
/// window chrome and what some systems index — "Ada Lovelace — Résumé" rather
/// than an untitled document.
pub fn render(laid_out: &LaidOut, title: &str) -> Vec<u8> {
    let mut doc = PdfDocument::new(title);

    // Only the faces actually used are embedded. A résumé that never sets an
    // italic should not carry one: the font is the file.
    let mut used: Vec<(FamilyId, FontStyle, PdfFontHandle)> = Vec::new();
    for page in &laid_out.pages {
        for item in &page.items {
            if let Draw::Text { family, style, .. } = item {
                if !used.iter().any(|(f, s, _)| f == family && s == style) {
                    let handle = embed(&mut doc, *family, *style);
                    used.push((*family, *style, handle));
                }
            }
        }
    }
    let handle_for = |family: FamilyId, style: FontStyle| -> Option<PdfFontHandle> {
        used.iter()
            .find(|(f, s, _)| *f == family && *s == style)
            .map(|(_, _, h)| h.clone())
    };

    let pages: Vec<PdfPage> = laid_out
        .pages
        .iter()
        .map(|p| page_ops(p, &handle_for))
        .collect();
    doc.with_pages(pages);

    let mut warnings = Vec::new();
    let bytes = doc.save(&PdfSaveOptions::default(), &mut warnings);
    for w in &warnings {
        log::warn!("résumé PDF: {}", w.msg);
    }
    bytes
}

fn embed(doc: &mut PdfDocument, family: FamilyId, style: FontStyle) -> PdfFontHandle {
    let mut warnings = Vec::new();
    let parsed = ParsedFont::from_bytes(fonts::bytes(family, style), 0, &mut warnings)
        .expect("the vendored faces parse; `fonts` tests hold that");
    PdfFontHandle::External(doc.add_font(&parsed))
}

fn page_ops(
    page: &Page,
    handle_for: &impl Fn(FamilyId, FontStyle) -> Option<PdfFontHandle>,
) -> PdfPage {
    let mut ops = Vec::with_capacity(page.items.len() * 6);

    // Fills first, so a heading band cannot paint over the heading it sits
    // behind. Within each pass the layout's own order is kept.
    for item in &page.items {
        if let Draw::Rect { x, y, w, h, color } = item {
            ops.push(Op::SetFillColor {
                col: to_color(*color),
            });
            ops.push(Op::DrawRectangle {
                rectangle: Rect {
                    x: Pt(*x),
                    // The layout's y is the top of the rect and PDF's is the
                    // bottom, so the flip has to subtract the height too.
                    y: Pt(page.height - y - h),
                    width: Pt(*w),
                    height: Pt(*h),
                    // Explicit, and not `Rect::from_xywh`, which leaves this
                    // `None` and paints nothing at all. See the module note.
                    mode: Some(PaintMode::Fill),
                    winding_order: Some(WindingOrder::NonZero),
                },
            });
        }
    }

    for item in &page.items {
        match item {
            Draw::Text {
                x,
                y,
                size,
                family,
                style,
                color,
                tracking,
                text,
            } => {
                if text.is_empty() {
                    continue;
                }
                let Some(handle) = handle_for(*family, *style) else {
                    continue;
                };
                ops.push(Op::StartTextSection);
                ops.push(Op::SetFillColor {
                    col: to_color(*color),
                });
                ops.push(Op::SetFont {
                    font: handle,
                    size: Pt(*size),
                });
                // `Tc` is in unscaled text space units, i.e. points, and it
                // applies to a whole run — so letterspacing a heading does not
                // cost the extraction its word boundaries the way drawing each
                // character separately would.
                if *tracking != 0.0 {
                    ops.push(Op::SetCharacterSpacing {
                        multiplier: *tracking,
                    });
                }
                ops.push(Op::SetTextMatrix {
                    matrix: TextMatrix::Translate(Pt(*x), Pt(page.height - y)),
                });
                ops.push(Op::ShowText {
                    items: vec![TextItem::Text(text.clone())],
                });
                if *tracking != 0.0 {
                    ops.push(Op::SetCharacterSpacing { multiplier: 0.0 });
                }
                ops.push(Op::EndTextSection);
            }
            Draw::Link { x, y, w, h, url } => {
                ops.push(Op::LinkAnnotation {
                    link: LinkAnnotation::new(
                        // An annotation is a region, not a path, so it has no
                        // paint mode to state.
                        Rect::from_xywh(Pt(*x), Pt(page.height - y - h), Pt(*w), Pt(*h)),
                        Actions::uri(url.clone()),
                        // No border and no highlight: a printed résumé should
                        // not grow boxes around its links.
                        None,
                        None,
                        None,
                    ),
                });
            }
            Draw::Rect { .. } => {}
        }
    }

    PdfPage::new(Mm::from(Pt(page.width)), Mm::from(Pt(page.height)), ops)
}

fn to_color(c: Rgb) -> Color {
    let (r, g, b) = c.to_f32();
    Color::Rgb(PdfRgb {
        r,
        g,
        b,
        icc_profile: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resume::model::{Bullet, Contact, Entry, Resume, SectionKind};
    use crate::resume::theme::{Theme, ThemeId, DEFAULT_ACCENT};
    use crate::resume::{layout, tailor};

    fn fixture() -> Resume {
        let mut r = Resume::starter();
        r.contact = Contact {
            name: "Ada Lovelace".into(),
            email: "ada@example.com".into(),
            location: "Toronto, ON".into(),
            ..Contact::default()
        };
        let experience = r
            .sections
            .iter_mut()
            .find(|s| s.kind == SectionKind::Experience)
            .unwrap();
        experience.entries.push(Entry {
            id: "e1".into(),
            org: "Acme Corp".into(),
            title: "Software Engineer Intern".into(),
            start: "May 2025".into(),
            end: "Aug 2025".into(),
            bullets: vec![Bullet {
                id: "b1".into(),
                text: "Cut p95 latency 40% by replacing the N+1 query path.".into(),
                skills: vec![],
            }],
            ..Entry::default()
        });
        r
    }

    fn bytes_for(theme: &Theme) -> Vec<u8> {
        let r = fixture();
        let laid_out = layout::lay_out(&r, &tailor::everything(&r), theme);
        render(&laid_out, "Ada Lovelace — Résumé")
    }

    #[test]
    fn produces_a_pdf() {
        let bytes = bytes_for(&Theme::default());
        assert!(bytes.starts_with(b"%PDF-"), "not a PDF");
        assert!(
            bytes.len() > 2_000,
            "suspiciously small: {} bytes",
            bytes.len()
        );
    }

    /// The whole point of subsetting the faces. Unsubsetted, one page of text
    /// came out a 397 KB file.
    #[test]
    fn stays_small_enough_to_email() {
        let bytes = bytes_for(&Theme::default());
        assert!(
            bytes.len() < 250_000,
            "{} bytes is too big to email",
            bytes.len()
        );
    }

    #[test]
    fn embeds_the_font_and_a_unicode_map() {
        let bytes = bytes_for(&Theme::default());
        let has = |needle: &[u8]| bytes.windows(needle.len()).any(|w| w == needle);
        // Without /ToUnicode the text renders and extracts as mojibake, which
        // is exactly the failure an applicant tracking system would hit.
        assert!(has(b"/ToUnicode"), "no ToUnicode CMap");
        assert!(has(b"/FontFile2"), "the font was not embedded");
    }

    /// Every rectangle must say how it is painted. Without an explicit mode
    /// printpdf emits `re` then `n`, which appends the path and paints nothing
    /// — the section rules and the Banner theme's heading bands were emitted,
    /// serialized, and completely invisible. No geometry or extraction check
    /// can see that; this one can.
    #[test]
    fn rectangles_are_filled_not_merely_appended() {
        let r = fixture();
        let theme = Theme::preset(ThemeId::Banner, DEFAULT_ACCENT);
        let laid_out = layout::lay_out(&r, &tailor::everything(&r), &theme);
        let page = page_ops(&laid_out.pages[0], &|_, _| {
            Some(PdfFontHandle::Builtin(printpdf::BuiltinFont::Helvetica))
        });

        let rects: Vec<&Rect> = page
            .ops
            .iter()
            .filter_map(|op| match op {
                Op::DrawRectangle { rectangle } => Some(rectangle),
                _ => None,
            })
            .collect();
        assert!(
            !rects.is_empty(),
            "the banner theme drew no rectangle at all"
        );
        for rect in rects {
            assert_eq!(
                rect.mode,
                Some(PaintMode::Fill),
                "a rectangle with no paint mode is invisible"
            );
        }
    }

    #[test]
    fn every_theme_renders() {
        for id in ThemeId::ALL {
            let bytes = bytes_for(&Theme::preset(id, DEFAULT_ACCENT));
            assert!(bytes.starts_with(b"%PDF-"), "{id:?} produced no PDF");
        }
    }
}
