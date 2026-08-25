//! The Harvard layout, as positioned boxes.
//!
//! Pure: a [`Resume`], a [`Selection`] and a [`Theme`] go in, [`Page`]s of
//! absolutely-positioned [`Draw`]s come out. Nothing here knows what a PDF is.
//!
//! That is the point. `resume::pdf` turns these boxes into a PDF and the
//! preview turns the same boxes into SVG, so the two cannot disagree about
//! where a line sits or which word ends it. The alternative — laying the page
//! out twice, once in Rust for the file and once in CSS for the screen — is
//! two engines to keep in step for a document whose entire job is fitting on
//! one page, and the drift shows up as a preview that says "one page" over a
//! PDF that is two.
//!
//! Coordinates are points, origin **top-left**, `y` growing downward, and every
//! `Draw::Text` `y` is a **baseline**. Baselines rather than box tops because
//! both consumers place text that way — PDF's `Tm` sets the baseline origin and
//! SVG's `<text y>` is the baseline — so no consumer has to reason about line
//! boxes or half-leading. `pdf.rs` flips y for PDF's bottom-left origin, and
//! that flip is the only coordinate conversion in the feature.
//!
//! Alignment is resolved here too: a centred name arrives as an already-computed
//! left `x`. Letting each backend centre it with its own measurement would put
//! the name in two different places.

use serde::Serialize;

use super::model::{Resume, Section, SectionKind};
use super::tailor::Selection;
use super::text;
use super::theme::{AccentRoles, FamilyId, FontStyle, HeadingStyle, Rgb, Theme};

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Draw {
    Text {
        x: f32,
        /// The baseline, from the top of the page.
        y: f32,
        size: f32,
        family: FamilyId,
        style: FontStyle,
        color: Rgb,
        /// Extra space per character, in points. Zero for everything but the
        /// name and the section headings.
        tracking: f32,
        text: String,
    },
    /// Rules and heading bands. A rule is a thin one.
    Rect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: Rgb,
    },
    /// A clickable region over text already drawn.
    Link {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        url: String,
    },
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Page {
    pub width: f32,
    pub height: f32,
    pub items: Vec<Draw>,
}

/// A laid-out document, plus what the layout learned on the way.
#[derive(Serialize, Clone, Debug)]
pub struct LaidOut {
    pub pages: Vec<Page>,
    /// How full the last page is, 0.0–1.0. The builder shows it as a "space
    /// left" gauge, which is the question anyone editing a one-page résumé is
    /// actually asking.
    pub fill: f32,
}

/// Only the two the header needs. Everything else is placed by `row`, which
/// positions its right-hand run from a measured width rather than an alignment.
#[derive(Clone, Copy, PartialEq)]
enum Align {
    Left,
    Center,
}

/// Pen and paper. Owns the page break rule so no caller has to remember it.
struct Pen<'a> {
    theme: &'a Theme,
    page_w: f32,
    page_h: f32,
    pages: Vec<Page>,
    items: Vec<Draw>,
    /// Top of the next line, from the top of the page.
    y: f32,
}

impl<'a> Pen<'a> {
    fn new(theme: &'a Theme) -> Pen<'a> {
        let (page_w, page_h) = theme.page.dimensions();
        Pen {
            theme,
            page_w,
            page_h,
            pages: Vec::new(),
            items: Vec::new(),
            y: theme.margin,
        }
    }

    fn left(&self) -> f32 {
        self.theme.margin
    }
    fn right(&self) -> f32 {
        self.page_w - self.theme.margin
    }
    fn bottom(&self) -> f32 {
        self.page_h - self.theme.margin
    }
    fn remaining(&self) -> f32 {
        self.bottom() - self.y
    }

    fn break_page(&mut self) {
        self.pages.push(Page {
            width: self.page_w,
            height: self.page_h,
            items: std::mem::take(&mut self.items),
        });
        self.y = self.theme.margin;
    }

    /// Breaks the page if `height` will not fit, so a block can be kept whole.
    ///
    /// Callers pass the height of a block that must not be split — an entry's
    /// organisation line together with its first bullet, say. An orphaned
    /// heading at the foot of a page is the one layout error a reader always
    /// notices.
    fn reserve(&mut self, height: f32) {
        if self.remaining() < height && !self.items.is_empty() {
            self.break_page();
        }
    }

    fn x_for(&self, align: Align, width: f32) -> f32 {
        match align {
            Align::Left => self.left(),
            Align::Center => self.left() + (self.theme.column() - width) / 2.0,
        }
    }

    /// Draws one line at the cursor and advances past it.
    ///
    /// `y` is passed to the box as a baseline: the cursor tracks the top of the
    /// line, and the baseline sits `size` below it. Using the font's real
    /// ascent would be marginally tighter and would make the drop depend on
    /// which face is set, so a bold run and a regular run on the same visual
    /// line would not share a baseline.
    #[allow(clippy::too_many_arguments)]
    fn line(
        &mut self,
        s: &str,
        size: f32,
        style: FontStyle,
        color: Rgb,
        align: Align,
        tracking: f32,
        advance: f32,
    ) {
        let family = self.theme.family;
        let w = text::width(s, family, style, size)
            + tracking * (s.chars().count() as f32 - 1.0).max(0.0);
        let x = self.x_for(align, w);
        self.items.push(Draw::Text {
            x,
            y: self.y + size,
            size,
            family,
            style,
            color,
            tracking,
            text: s.to_string(),
        });
        self.y += advance;
    }

    /// A left run and a right run sharing one baseline — the format's spine.
    #[allow(clippy::too_many_arguments)]
    fn row(
        &mut self,
        left: &str,
        left_style: FontStyle,
        left_color: Rgb,
        right: &str,
        right_style: FontStyle,
        size: f32,
    ) {
        let family = self.theme.family;
        let baseline = self.y + size;
        let right = right.trim();
        let mut room = self.theme.column();

        if !right.is_empty() {
            let rw = text::width(right, family, right_style, size);
            self.items.push(Draw::Text {
                x: self.right() - rw,
                y: baseline,
                size,
                family,
                style: right_style,
                color: Rgb::BLACK,
                tracking: 0.0,
                text: right.to_string(),
            });
            // Keep a gap so a long title cannot run into the dates.
            room -= rw + size;
        }

        let left = left.trim();
        if !left.is_empty() {
            let fitted = text::ellipsize(left, family, left_style, size, room.max(size));
            self.items.push(Draw::Text {
                x: self.left(),
                y: baseline,
                size,
                family,
                style: left_style,
                color: left_color,
                tracking: 0.0,
                text: fitted,
            });
        }
        self.y += size * self.theme.leading;
    }

    /// Wrapped body text, optionally with a hanging marker.
    fn paragraph(
        &mut self,
        body: &str,
        marker: Option<&str>,
        indent: f32,
        size: f32,
        style: FontStyle,
    ) {
        let family = self.theme.family;
        let width = self.theme.column() - indent;
        let lines = text::wrap(body, family, style, size, width);
        let lh = size * self.theme.leading;

        for (n, line) in lines.iter().enumerate() {
            // A bullet that spills onto a third page has stopped being a
            // bullet; break rather than lose it.
            if self.remaining() < lh {
                self.break_page();
            }
            let baseline = self.y + size;
            if n == 0 {
                if let Some(mark) = marker {
                    let mw = text::width(mark, family, FontStyle::Regular, size);
                    self.items.push(Draw::Text {
                        x: self.left() + indent - mw - size * 0.34,
                        y: baseline,
                        size,
                        family,
                        style: FontStyle::Regular,
                        color: self.theme.ink(|r: &AccentRoles| r.bullet_marks),
                        tracking: 0.0,
                        text: mark.to_string(),
                    });
                }
            }
            self.items.push(Draw::Text {
                x: self.left() + indent,
                y: baseline,
                size,
                family,
                style,
                color: Rgb::BLACK,
                tracking: 0.0,
                text: line.clone(),
            });
            self.y += lh;
        }
    }

    /// A clickable region over a substring of a line already drawn.
    ///
    /// The line is drawn as **one** run and the hit box measured on top of it,
    /// rather than split into linked and unlinked runs. Splitting would put a
    /// `Tj` boundary mid-line, and a text extractor — which is the first thing
    /// to read a résumé — can turn those boundaries into word breaks. A
    /// clickable link is worth much less than a machine-readable one.
    fn link_over(&mut self, line: &str, needle: &str, url: &str, x: f32, baseline: f32, size: f32) {
        let Some(at) = line.find(needle) else { return };
        let family = self.theme.family;
        let before = text::width(&line[..at], family, FontStyle::Regular, size);
        let w = text::width(needle, family, FontStyle::Regular, size);
        self.items.push(Draw::Link {
            x: x + before,
            // Up from the baseline by roughly the cap height, so the target
            // covers the text rather than sitting under it.
            y: baseline - size * 0.8,
            w,
            h: size * 1.05,
            url: url.to_string(),
        });
    }

    fn rule(&mut self, thickness: f32, color: Rgb) {
        self.items.push(Draw::Rect {
            x: self.left(),
            y: self.y,
            w: self.theme.column(),
            h: thickness,
            color,
        });
        self.y += thickness;
    }

    fn finish(mut self) -> LaidOut {
        let used = self.y - self.theme.margin;
        let usable = self.bottom() - self.theme.margin;
        if !self.items.is_empty() || self.pages.is_empty() {
            self.break_page();
        }
        LaidOut {
            pages: self.pages,
            fill: (used / usable.max(1.0)).clamp(0.0, 1.0),
        }
    }
}

/// Lays the selected parts of `resume` onto pages.
pub fn lay_out(resume: &Resume, selection: &Selection, theme: &Theme) -> LaidOut {
    let theme = theme.sanitized();
    let mut pen = Pen::new(&theme);
    let base = theme.base_size;

    header(&mut pen, resume, selection, base);

    for section in &resume.sections {
        let has_content = match section.kind {
            SectionKind::Skills => !selection.skills.is_empty(),
            _ => section
                .entries
                .iter()
                .any(|e| selection.entries.contains(&e.id)),
        };
        if !has_content {
            continue;
        }
        heading(&mut pen, &section.title, base);
        match section.kind {
            SectionKind::Skills => skills_block(&mut pen, selection, base),
            _ => entries_block(&mut pen, section, selection, base),
        }
    }

    pen.finish()
}

fn header(pen: &mut Pen, resume: &Resume, selection: &Selection, base: f32) {
    let contact = &resume.contact;
    let name = contact.name.trim();
    if !name.is_empty() {
        let size = base * 1.85;
        let color = pen.theme.ink(|r: &AccentRoles| r.name);
        pen.line(
            &name.to_uppercase(),
            size,
            FontStyle::Bold,
            color,
            Align::Center,
            size * 0.06,
            size * 1.16,
        );
    }

    if let Some(headline) = selection
        .headline
        .as_deref()
        .or(contact.headline.as_deref())
    {
        let headline = headline.trim();
        if !headline.is_empty() {
            pen.line(
                headline,
                base * 1.05,
                FontStyle::Italic,
                Rgb::BLACK,
                Align::Center,
                0.0,
                base * 1.35,
            );
        }
    }

    // One `•`-separated line, wrapped if it has to be. Everything the reader
    // needs to reach you, in the order they would use it.
    let mut parts: Vec<String> = Vec::new();
    for field in [&contact.location, &contact.phone, &contact.email] {
        let field = field.trim();
        if !field.is_empty() {
            parts.push(field.to_string());
        }
    }
    for link in &contact.links {
        let shown = display_url(&link.url, &link.label);
        if !shown.is_empty() {
            parts.push(shown);
        }
    }
    if !parts.is_empty() {
        let size = base * 0.95;
        let joined = parts.join("  •  ");
        for line in text::wrap(
            &joined,
            pen.theme.family,
            FontStyle::Regular,
            size,
            pen.theme.column(),
        ) {
            let width = text::width(&line, pen.theme.family, FontStyle::Regular, size);
            let x = pen.x_for(Align::Center, width);
            let baseline = pen.y + size;
            pen.line(
                &line,
                size,
                FontStyle::Regular,
                Rgb::BLACK,
                Align::Center,
                0.0,
                size * 1.25,
            );

            // "github.com/you" on paper, the real URL under the click. The
            // email too, which a reader turns into a mailto:.
            for link in &contact.links {
                let shown = display_url(&link.url, &link.label);
                if !shown.is_empty() {
                    pen.link_over(&line, &shown, &link.url, x, baseline, size);
                }
            }
            let email = contact.email.trim();
            if !email.is_empty() {
                let mailto = format!("mailto:{email}");
                pen.link_over(&line, email, &mailto, x, baseline, size);
            }
        }
    }
    pen.y += base * 0.5;
}

/// What a link should read as on paper. A résumé prints "github.com/you", not
/// "https://github.com/you?utm_source=" — but the annotation still points at
/// the real URL, so it stays clickable in a PDF reader.
fn display_url(url: &str, label: &str) -> String {
    let label = label.trim();
    let trimmed = url
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("www.")
        .trim_end_matches('/');
    if trimmed.is_empty() {
        return label.to_string();
    }
    if label.is_empty() {
        trimmed.to_string()
    } else {
        trimmed.to_string()
    }
}

fn heading(pen: &mut Pen, title: &str, base: f32) {
    let size = base * 1.02;
    let text_color = pen.theme.ink(|r: &AccentRoles| r.headings);
    let rule_color = pen.theme.ink(|r: &AccentRoles| r.rules);

    // A heading with nothing under it is worse than a page break before it.
    pen.reserve(size * 4.0);
    pen.y += base * 0.62;

    match pen.theme.heading {
        HeadingStyle::Band => {
            let pad = size * 0.42;
            let band_h = size + pad * 2.0;
            pen.items.push(Draw::Rect {
                x: pen.left() - pad * 0.5,
                y: pen.y,
                w: pen.theme.column() + pad,
                h: band_h,
                color: pen.theme.accent.tint(0.84),
            });
            pen.y += pad;
            pen.line(
                &title.to_uppercase(),
                size,
                FontStyle::Bold,
                text_color,
                Align::Left,
                size * 0.09,
                size + pad,
            );
        }
        HeadingStyle::Rule => {
            pen.line(
                &title.to_uppercase(),
                size,
                FontStyle::Bold,
                text_color,
                Align::Left,
                size * 0.09,
                size * 1.16,
            );
            pen.rule(0.7, rule_color);
        }
        HeadingStyle::Plain => {
            pen.line(
                &title.to_uppercase(),
                size,
                FontStyle::Bold,
                text_color,
                Align::Left,
                size * 0.09,
                size * 1.16,
            );
        }
    }
    pen.y += base * 0.32;
}

fn entries_block(pen: &mut Pen, section: &Section, selection: &Selection, base: f32) {
    let ordered = selection.order_for(section);
    for (n, entry) in ordered.iter().enumerate() {
        if n > 0 {
            pen.y += base * 0.5;
        }

        // Keep the two header lines with at least one bullet. Splitting an
        // entry across the fold so that only "Acme Corp" is left behind reads
        // as a rendering bug.
        let lh = base * pen.theme.leading;
        pen.reserve(lh * 3.0);

        let dates = date_range(&entry.start, &entry.end);
        let org_baseline = pen.y + base;
        pen.row(
            &entry.org,
            FontStyle::Bold,
            pen.theme.ink(|r: &AccentRoles| r.org),
            &entry.location,
            FontStyle::Regular,
            base,
        );
        // A project's repository, an employer's careers page. The organisation
        // is the natural target — it is what a reader would click.
        if let Some(url) = entry
            .link
            .as_deref()
            .map(str::trim)
            .filter(|u| !u.is_empty())
        {
            let org = entry.org.trim();
            if !org.is_empty() {
                let w = text::width(org, pen.theme.family, FontStyle::Bold, base);
                let x = pen.left();
                pen.items.push(Draw::Link {
                    x,
                    y: org_baseline - base * 0.8,
                    w,
                    h: base * 1.05,
                    url: url.to_string(),
                });
            }
        }
        if !entry.title.trim().is_empty() || !dates.is_empty() {
            pen.row(
                &entry.title,
                FontStyle::Italic,
                Rgb::BLACK,
                &dates,
                FontStyle::Italic,
                base,
            );
        }
        if let Some(detail) = entry.detail.as_deref() {
            if !detail.trim().is_empty() {
                pen.paragraph(detail, None, 0.0, base * 0.96, FontStyle::Regular);
            }
        }

        for bullet in &entry.bullets {
            if !selection.bullets.contains(&bullet.id) {
                continue;
            }
            if bullet.text.trim().is_empty() {
                continue;
            }
            pen.y += base * 0.12;
            pen.paragraph(
                &bullet.text,
                Some("•"),
                base * 0.95,
                base,
                FontStyle::Regular,
            );
        }
    }
}

/// The skills section: one `Label: a, b, c` line per catalog category, wrapped
/// with a hanging indent so the second line lines up under the first skill
/// rather than under the label.
fn skills_block(pen: &mut Pen, selection: &Selection, base: f32) {
    let family = pen.theme.family;
    let label_width = selection
        .skills
        .iter()
        .map(|(category, _)| text::width(&format!("{category}:  "), family, FontStyle::Bold, base))
        .fold(0.0_f32, f32::max);

    for (n, (category, skills)) in selection.skills.iter().enumerate() {
        if skills.is_empty() {
            continue;
        }
        if n > 0 {
            pen.y += base * 0.12;
        }
        let lh = base * pen.theme.leading;
        pen.reserve(lh * 2.0);

        let baseline = pen.y + base;
        pen.items.push(Draw::Text {
            x: pen.left(),
            y: baseline,
            size: base,
            family,
            style: FontStyle::Bold,
            color: Rgb::BLACK,
            tracking: 0.0,
            text: format!("{category}:"),
        });
        pen.paragraph(
            &skills.join(", "),
            None,
            label_width,
            base,
            FontStyle::Regular,
        );
    }
}

/// "May 2025 – Aug 2025", with an en dash because that is what a date range
/// takes. An open end reads "Present" only if the user wrote it.
fn date_range(start: &str, end: &str) -> String {
    let (start, end) = (start.trim(), end.trim());
    match (start.is_empty(), end.is_empty()) {
        (true, true) => String::new(),
        (true, false) => end.to_string(),
        (false, true) => start.to_string(),
        (false, false) => format!("{start} – {end}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resume::model::{Bullet, Contact, Entry, Link};
    use crate::resume::tailor;
    use crate::resume::theme::{ThemeId, DEFAULT_ACCENT};

    fn fixture() -> Resume {
        let mut r = Resume::starter();
        r.contact = Contact {
            name: "Ada Lovelace".into(),
            headline: Some("Software Engineer".into()),
            email: "ada@example.com".into(),
            phone: "+1 416 555 0100".into(),
            location: "Toronto, ON".into(),
            links: vec![Link {
                label: "GitHub".into(),
                url: "https://github.com/ada".into(),
            }],
        };
        let experience = r
            .sections
            .iter_mut()
            .find(|s| s.kind == SectionKind::Experience)
            .expect("starter has an experience section");
        experience.entries.push(Entry {
            id: "e1".into(),
            org: "Acme Corp".into(),
            title: "Software Engineer Intern".into(),
            location: "Toronto, ON".into(),
            start: "May 2025".into(),
            end: "Aug 2025".into(),
            link: None,
            detail: None,
            bullets: vec![
                Bullet { id: "b1".into(), text: "Cut p95 latency 40% by replacing the N+1 query path with a windowed aggregate.".into(), skills: vec!["SQL".into()] },
                Bullet { id: "b2".into(), text: "Shipped a Rust service handling 2,000 requests per second.".into(), skills: vec!["Rust".into()] },
            ],
        });
        r
    }

    fn all_of(resume: &Resume) -> Selection {
        tailor::everything(resume)
    }

    fn texts(out: &LaidOut) -> Vec<String> {
        out.pages
            .iter()
            .flat_map(|p| &p.items)
            .filter_map(|d| match d {
                Draw::Text { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn lays_out_onto_one_page() {
        let r = fixture();
        let out = lay_out(&r, &all_of(&r), &Theme::default());
        assert_eq!(out.pages.len(), 1);
        assert!(out.fill > 0.0 && out.fill <= 1.0);
    }

    #[test]
    fn every_box_sits_inside_the_margins() {
        let r = fixture();
        for id in ThemeId::ALL {
            let theme = Theme::preset(id, DEFAULT_ACCENT);
            let out = lay_out(&r, &all_of(&r), &theme);
            for page in &out.pages {
                for item in &page.items {
                    let (x, y, w) = match item {
                        Draw::Text {
                            x,
                            y,
                            size,
                            family,
                            style,
                            text,
                            tracking,
                            ..
                        } => (
                            *x,
                            *y,
                            text::width(text, *family, *style, *size)
                                + tracking * (text.chars().count() as f32 - 1.0).max(0.0),
                        ),
                        Draw::Rect { x, y, w, .. } | Draw::Link { x, y, w, .. } => (*x, *y, *w),
                    };
                    assert!(
                        x >= theme.margin - 6.0,
                        "{id:?}: box starts at {x}, margin {}",
                        theme.margin
                    );
                    assert!(
                        x + w <= page.width - theme.margin + 6.0,
                        "{id:?}: box ends at {}, page {} margin {}",
                        x + w,
                        page.width,
                        theme.margin
                    );
                    assert!(y <= page.height, "{id:?}: box below the page at {y}");
                }
            }
        }
    }

    #[test]
    fn the_name_is_rendered_in_caps() {
        let r = fixture();
        let out = lay_out(&r, &all_of(&r), &Theme::default());
        assert!(texts(&out).contains(&"ADA LOVELACE".to_string()));
    }

    #[test]
    fn bullet_text_survives_wrapping() {
        let r = fixture();
        let out = lay_out(&r, &all_of(&r), &Theme::default());
        let joined = texts(&out).join(" ");
        assert!(joined.contains("Cut p95 latency 40%"), "{joined}");
        assert!(joined.contains("windowed aggregate"), "{joined}");
    }

    /// A bullet the selection dropped must not reach the page — that is the
    /// entire contract the tailoring pass depends on.
    #[test]
    fn deselected_bullets_are_not_drawn() {
        let r = fixture();
        let mut selection = all_of(&r);
        selection.bullets.retain(|id| id != "b2");
        let joined = texts(&lay_out(&r, &selection, &Theme::default())).join(" ");
        assert!(
            !joined.contains("2,000 requests"),
            "dropped bullet was drawn"
        );
        assert!(
            joined.contains("Cut p95 latency"),
            "kept bullet was not drawn"
        );
    }

    #[test]
    fn an_empty_section_draws_no_heading() {
        let r = fixture();
        let out = lay_out(&r, &all_of(&r), &Theme::default());
        // The starter résumé has Education and Projects with no entries.
        assert!(!texts(&out).contains(&"EDUCATION".to_string()));
    }

    #[test]
    fn long_documents_paginate() {
        let mut r = fixture();
        let experience = r
            .sections
            .iter_mut()
            .find(|s| s.kind == SectionKind::Experience)
            .unwrap();
        let template = experience.entries[0].clone();
        for n in 0..14 {
            let mut entry = template.clone();
            entry.id = format!("e{n}");
            entry.bullets = entry
                .bullets
                .iter()
                .enumerate()
                .map(|(i, b)| Bullet {
                    id: format!("e{n}b{i}"),
                    ..b.clone()
                })
                .collect();
            experience.entries.push(entry);
        }
        let out = lay_out(&r, &all_of(&r), &Theme::default());
        assert!(out.pages.len() > 1, "15 entries should not fit one page");
        for page in &out.pages {
            assert!(!page.items.is_empty(), "produced an empty page");
        }
    }

    #[test]
    fn every_theme_produces_boxes() {
        let r = fixture();
        for id in ThemeId::ALL {
            let out = lay_out(&r, &all_of(&r), &Theme::preset(id, DEFAULT_ACCENT));
            assert!(!out.pages[0].items.is_empty(), "{id:?} drew nothing");
        }
    }

    #[test]
    fn the_banner_theme_draws_a_band_behind_its_headings() {
        let r = fixture();
        let out = lay_out(
            &r,
            &all_of(&r),
            &Theme::preset(ThemeId::Banner, DEFAULT_ACCENT),
        );
        let bands = out.pages[0]
            .items
            .iter()
            .filter(|d| matches!(d, Draw::Rect { h, .. } if *h > 4.0))
            .count();
        assert!(bands > 0, "banner theme drew no heading band");
    }

    fn links_of(out: &LaidOut) -> Vec<(String, f32, f32)> {
        out.pages
            .iter()
            .flat_map(|p| &p.items)
            .filter_map(|d| match d {
                Draw::Link { url, x, w, .. } => Some((url.clone(), *x, *w)),
                _ => None,
            })
            .collect()
    }

    /// The builder tells the user their links are clickable in the PDF, so they
    /// had better be — and the box has to sit over the text it links rather
    /// than at the left margin.
    #[test]
    fn contact_links_get_a_clickable_box_over_their_text() {
        let r = fixture();
        let out = lay_out(&r, &all_of(&r), &Theme::default());
        let links = links_of(&out);

        let github = links
            .iter()
            .find(|(url, _, _)| url == "https://github.com/ada")
            .expect("the GitHub link has no clickable box");
        assert!(github.2 > 0.0, "the box has no width");
        assert!(
            github.1 > Theme::default().margin,
            "the box sits at the margin rather than over the text"
        );
        assert!(
            links
                .iter()
                .any(|(url, _, _)| url == "mailto:ada@example.com"),
            "the email is not clickable"
        );
    }

    #[test]
    fn an_entry_link_covers_its_organisation() {
        let mut r = fixture();
        let experience = r
            .sections
            .iter_mut()
            .find(|s| s.kind == SectionKind::Experience)
            .unwrap();
        experience.entries[0].link = Some("https://acme.example".into());
        let out = lay_out(&r, &all_of(&r), &Theme::default());
        let link = links_of(&out)
            .into_iter()
            .find(|(url, _, _)| url == "https://acme.example")
            .expect("the entry link has no box");
        let org_width = text::width(
            "Acme Corp",
            FamilyId::Serif,
            FontStyle::Bold,
            Theme::default().base_size,
        );
        assert!(
            (link.2 - org_width).abs() < 0.5,
            "box is {} wide, the organisation is {org_width}",
            link.2
        );
    }

    #[test]
    fn an_entry_without_a_link_gets_no_box() {
        let r = fixture();
        let out = lay_out(&r, &all_of(&r), &Theme::default());
        assert!(
            !links_of(&out)
                .iter()
                .any(|(url, _, _)| url.contains("acme")),
            "a link appeared for an entry that has none"
        );
    }

    #[test]
    fn urls_print_without_their_scheme() {
        assert_eq!(
            display_url("https://github.com/ada/", "GitHub"),
            "github.com/ada"
        );
        assert_eq!(display_url("http://www.example.com", ""), "example.com");
    }

    #[test]
    fn date_ranges_read_correctly() {
        assert_eq!(date_range("May 2025", "Aug 2025"), "May 2025 – Aug 2025");
        assert_eq!(date_range("May 2025", ""), "May 2025");
        assert_eq!(date_range("", ""), "");
    }
}
