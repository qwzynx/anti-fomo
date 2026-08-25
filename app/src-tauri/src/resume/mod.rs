//! The résumé builder: a document the user writes once, and a PDF tailored to
//! whichever posting they are looking at.
//!
//! The app already reads a posting's requirements and knows which catalog
//! skills it asks for, and it already knows which of those the user has. That
//! match is shown as "you cover 6 of 9" and then goes nowhere — the user leaves
//! for a word processor to write the résumé that says so. This closes the loop.
//!
//! Layered so that exactly one thing knows about each concern:
//!
//! ```text
//!   model    what a résumé is            (no styling, no layout)
//!   theme    how it should look          (no content)
//!   tailor   what belongs on this page   (pure; uses layout to test the fit)
//!   text     how wide a string is        (pure; the font's own advances)
//!   layout   where every box goes        (pure; emits positioned boxes)
//!      ├── pdf      boxes → a file
//!      └── preview  boxes → SVG in the webview
//!   jsonresume  import/export against the open schema
//! ```
//!
//! `layout` having two consumers is the load-bearing decision. A résumé's whole
//! job is fitting a page, so a preview that disagrees with the file about where
//! a line breaks is worse than no preview. One layout pass, two backends.

pub mod fonts;
pub mod jsonresume;
pub mod layout;
pub mod model;
pub mod pdf;
pub mod tailor;
pub mod text;
pub mod theme;

pub use model::Resume;
pub use tailor::{Selection, Variant};
pub use theme::Theme;

/// Lay out a résumé exactly as the builder previews it — everything included.
pub fn preview(resume: &Resume, theme: &Theme) -> layout::LaidOut {
    layout::lay_out(resume, &tailor::everything(resume), theme)
}

/// Lay out a résumé tailored to one posting.
pub fn tailored(
    resume: &Resume,
    required: &[String],
    have: &[String],
    variant: &Variant,
    theme: &Theme,
) -> (layout::LaidOut, Selection) {
    let selection = tailor::tailor(resume, required, have, variant, theme);
    let laid_out = layout::lay_out(resume, &selection, theme);
    (laid_out, selection)
}

/// The filename a saved résumé should default to.
///
/// "Ada Lovelace — Acme Corp.pdf" rather than "resume.pdf": these end up in a
/// recruiter's downloads folder next to a hundred others, and the ones that
/// are just "resume(3).pdf" are the ones nobody can find again.
pub fn suggested_filename(name: &str, company: Option<&str>) -> String {
    let clean = |s: &str| -> String {
        s.chars()
            .map(|c| {
                if c.is_alphanumeric() || c == ' ' || c == '-' || c == '&' {
                    c
                } else {
                    ' '
                }
            })
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    };
    let who = clean(name);
    let who = if who.is_empty() {
        "Resume".to_string()
    } else {
        who
    };
    match company.map(clean).filter(|c| !c.is_empty()) {
        Some(company) => format!("{who} - {company}.pdf"),
        None => format!("{who} - Resume.pdf"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filenames_are_readable_and_safe() {
        assert_eq!(
            suggested_filename("Ada Lovelace", Some("Acme Corp")),
            "Ada Lovelace - Acme Corp.pdf"
        );
        assert_eq!(
            suggested_filename("Ada Lovelace", None),
            "Ada Lovelace - Resume.pdf"
        );
        assert_eq!(suggested_filename("", None), "Resume - Resume.pdf");
    }

    /// Path separators and the rest must never reach a filename.
    #[test]
    fn filenames_drop_dangerous_characters() {
        let name = suggested_filename("../../etc/passwd", Some("A/B:C*?"));
        assert!(!name.contains('/'), "{name}");
        assert!(!name.contains(':'), "{name}");
        assert!(!name.contains('*'), "{name}");
    }
}
