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
pub mod terms;
pub mod text;
pub mod theme;

pub use model::Resume;
pub use tailor::{Selection, Target, Variant};
pub use theme::Theme;

/// Lay out a résumé exactly as the builder previews it — everything included.
pub fn preview(resume: &Resume, theme: &Theme) -> layout::LaidOut {
    layout::lay_out(resume, &tailor::everything(resume), theme)
}

/// Lay out a résumé tailored to one posting.
pub fn tailored(
    resume: &Resume,
    target: &tailor::Target,
    have: &[String],
    variant: &Variant,
    theme: &Theme,
) -> (layout::LaidOut, Selection) {
    let prepared = prepare(resume, variant);
    let resume = prepared.as_ref().unwrap_or(resume);
    let selection = tailor::tailor(resume, target, have, variant, theme);
    let laid_out = layout::lay_out(resume, &selection, theme);
    (laid_out, selection)
}

/// The résumé as this variant shows it before any choosing: sections in the
/// reader's order, and the detail items they left out (a technology in a
/// project's stack, say) gone from the line. `None` when the variant changes
/// neither, so the common case costs no copy.
fn prepare(resume: &Resume, variant: &Variant) -> Option<Resume> {
    let leaves_out_details = variant
        .include
        .iter()
        .any(|(key, on)| !on && key.contains("/detail:"));
    let mut out = in_section_order(resume, &variant.section_order);
    if leaves_out_details {
        let doc = out.get_or_insert_with(|| resume.clone());
        for section in &mut doc.sections {
            // A skills line's run is filtered by skill name in the tailoring,
            // the same whether the line was written or generated.
            if section.kind == model::SectionKind::Skills {
                continue;
            }
            for entry in &mut section.entries {
                let Some(detail) = entry.detail.as_deref() else {
                    continue;
                };
                let (items, joiner) = model::detail_items(detail);
                let kept: Vec<&str> = items
                    .into_iter()
                    .filter(|item| {
                        variant.include.get(&model::detail_key(&entry.id, item)) != Some(&false)
                    })
                    .collect();
                entry.detail = (!kept.is_empty()).then(|| kept.join(joiner));
            }
        }
    }
    out
}

/// The résumé with its sections in the order a variant asked for, or `None`
/// when it asked for none. Moving a section is the reader's call alone — the
/// tailoring never reorders sections by itself — so this is applied before
/// the pass rather than decided inside it. A stable sort: sections the order
/// does not name (one added since) keep their relative place, after it.
fn in_section_order(resume: &Resume, order: &[String]) -> Option<Resume> {
    if order.is_empty() {
        return None;
    }
    let rank: std::collections::HashMap<&str, usize> = order
        .iter()
        .enumerate()
        .map(|(i, id)| (id.as_str(), i))
        .collect();
    let mut resume = resume.clone();
    resume
        .sections
        .sort_by_key(|s| rank.get(s.id.as_str()).copied().unwrap_or(usize::MAX));
    Some(resume)
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

    #[test]
    fn a_left_out_detail_item_leaves_the_line() {
        let entry = model::Entry {
            id: "p1".into(),
            org: "Fretnote".into(),
            detail: Some("Svelte · TypeScript · Rust".into()),
            ..Default::default()
        };
        let resume = Resume {
            sections: vec![model::Section {
                id: "proj".into(),
                kind: model::SectionKind::Projects,
                entries: vec![entry],
                ..Default::default()
            }],
            ..Default::default()
        };
        let mut variant = Variant::default();
        assert!(prepare(&resume, &variant).is_none());
        variant
            .include
            .insert(model::detail_key("p1", "TypeScript"), false);
        let out = prepare(&resume, &variant).expect("an item was left out");
        assert_eq!(
            out.sections[0].entries[0].detail.as_deref(),
            Some("Svelte · Rust")
        );
        for item in ["Svelte", "Rust"] {
            variant.include.insert(model::detail_key("p1", item), false);
        }
        let out = prepare(&resume, &variant).unwrap();
        assert_eq!(
            out.sections[0].entries[0].detail, None,
            "an emptied line goes"
        );
    }

    #[test]
    fn detail_lines_split_on_what_they_were_written_with() {
        assert_eq!(
            model::detail_items("Rust · SQLite, bundled"),
            (vec!["Rust", "SQLite, bundled"], " · ")
        );
        assert_eq!(model::detail_items("a, b"), (vec!["a", "b"], ", "));
        assert_eq!(
            model::detail_items("GPA 3.9/4.0"),
            (vec!["GPA 3.9/4.0"], ", ")
        );
        assert_eq!(model::detail_items("  "), (vec![], ", "));
    }

    #[test]
    fn sections_follow_the_variant_order_and_unnamed_ones_follow() {
        let section = |id: &str| model::Section {
            id: id.into(),
            ..Default::default()
        };
        let resume = Resume {
            sections: vec![
                section("edu"),
                section("exp"),
                section("proj"),
                section("new"),
            ],
            ..Default::default()
        };
        assert!(in_section_order(&resume, &[]).is_none());
        let moved = in_section_order(&resume, &["proj".into(), "edu".into(), "exp".into()])
            .expect("an order was given");
        let ids: Vec<&str> = moved.sections.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["proj", "edu", "exp", "new"]);
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
