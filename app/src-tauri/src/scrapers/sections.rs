//! Cutting one posting's description into the parts somebody actually reads.
//!
//! Most job APIs hand back the description as a single HTML blob — Workday,
//! Greenhouse and Eightfold all do — so the "Requirements / Responsibilities /
//! Perks" split the UI renders has to be recovered from the employer's own
//! headings. That is what this module does, and it is deliberately the only
//! place that knows those heading words: five handlers in
//! [`super::details`] feed it, and none of them should grow its own copy.
//!
//! The walk is a flatten, not a query. An HTML description is written by
//! whoever pasted it into the ATS, so `<h3>`, `<p><b>…</b></p>` and
//! `<p>Requirements:</p>` all turn up as the same heading and `<li>`, `<p>`
//! and bare `<div>` all turn up as the same line. Selecting for a fixed
//! structure would work on one employer's postings and nothing else.

use scraper::node::Node;
use scraper::{ElementRef, Html};

use super::collapse_ws;

/// Longest a line may be and still be read as a heading. Real section
/// headings are two or three words; a 200-character paragraph that happens to
/// end in a colon is a sentence introducing a list.
const MAX_HEADING: usize = 90;

/// Lines kept per section. Long enough for the most itemised posting seen
/// (Job Bank's "Experience and specialization" runs to about 40), short
/// enough that a page whose whole body parsed as one section cannot flood the
/// pane.
const MAX_LINES: usize = 60;

/// A description split the way the UI renders it. Every field is a list of
/// lines because that is what the employer wrote — collapsing them into one
/// paragraph and re-splitting later loses the bullets.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Sections {
    /// Prose that is not a requirement, a duty or a perk: the "About us"
    /// opener, and anything under a heading we do not recognise.
    pub overview: Vec<String>,
    pub requirements: Vec<String>,
    pub responsibilities: Vec<String>,
    pub perks: Vec<String>,
}

impl Sections {
    pub fn of(&mut self, section: Section) -> &mut Vec<String> {
        match section {
            Section::Overview => &mut self.overview,
            Section::Requirements => &mut self.requirements,
            Section::Responsibilities => &mut self.responsibilities,
            Section::Perks => &mut self.perks,
        }
    }

    /// Folds another split in, keeping this one's lines first. Used where a
    /// posting arrives as several labelled blobs that each split separately.
    pub fn merge(&mut self, other: Sections) {
        for (mine, theirs) in [
            (&mut self.overview, other.overview),
            (&mut self.requirements, other.requirements),
            (&mut self.responsibilities, other.responsibilities),
            (&mut self.perks, other.perks),
        ] {
            for line in theirs {
                push_line(mine, line);
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.overview.is_empty()
            && self.requirements.is_empty()
            && self.responsibilities.is_empty()
            && self.perks.is_empty()
    }
}

/// Which part of a posting a heading opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Overview,
    Requirements,
    Responsibilities,
    Perks,
}

// Matched against a lowercased heading, longest-intent-first: the lists are
// tried in this order, so a heading naming two buckets ("Responsibilities and
// Qualifications") lands in the one it leads with.
//
// Phrases rather than single words wherever the single word is ordinary
// English. "experience" alone is the last thing tried for that reason — it
// appears inside "What experience you'll gain", which is a perk.

const RESPONSIBILITY: &[&str] = &[
    "responsibilit",
    "what you'll do",
    "what you will do",
    "what you'll be doing",
    "what you would be doing",
    "what you’ll do",
    "what you’ll be doing",
    "your day",
    "day-to-day",
    "day to day",
    "your impact",
    "the impact you",
    "key accountab",
    "essential function",
    "essential duties",
    "duties",
    "job duties",
    "the role",
    "in this role",
    "about the role",
    "role overview",
    "what to expect",
    "tasks",
];

const PERK: &[&str] = &[
    "benefit",
    "perk",
    "what we offer",
    "what you'll get",
    "what you’ll get",
    "what's in it for you",
    "what’s in it for you",
    "why join",
    "why you'll love",
    "we offer",
    "total reward",
    "compensation",
    "salary",
    "pay range",
    "pay transparency",
    "remuneration",
];

const REQUIREMENT: &[&str] = &[
    "requirement",
    "qualification",
    "what you'll need",
    "what you will need",
    "what you’ll need",
    "what we're looking for",
    "what we are looking for",
    "what we’re looking for",
    "who you are",
    "about you",
    "you have",
    "you bring",
    "must have",
    "nice to have",
    "preferred",
    "minimum",
    "basic",
    "eligibility",
    "education",
    "competenc",
    "skill",
    "experience and specialization",
    "required experience",
    "experience required",
    "years of experience",
    "relevant experience",
    "professional experience",
    "your experience",
];

/// Headings that name a section only when they *are* the heading. "Experience"
/// on its own opens a requirements list; inside a longer line it is ordinary
/// English, and matching it as a substring put four paragraphs of company
/// slogan ("The Best Experience Company") under Requirements on a real iCIMS
/// posting.
const EXACT: &[(&str, Section)] = &[("experience", Section::Requirements)];

/// Which section a heading opens. Public because Lever hands us its list
/// headings as a separate field rather than inside the HTML, so the caller
/// has to classify one by hand before splitting the body under it.
pub fn section_of(heading: &str) -> Section {
    let h = heading.trim().trim_end_matches(':').trim().to_lowercase();
    if let Some((_, section)) = EXACT.iter().find(|(word, _)| *word == h) {
        return *section;
    }
    for (phrases, section) in [
        (RESPONSIBILITY, Section::Responsibilities),
        (PERK, Section::Perks),
        (REQUIREMENT, Section::Requirements),
    ] {
        if phrases.iter().any(|p| h.contains(p)) {
            return section;
        }
    }
    // An unrecognised heading ends the section it follows rather than
    // extending it: "Equal Opportunity Employer" after a requirements list is
    // not a requirement.
    Section::Overview
}

/// Splits a whole description into sections. Never fails: HTML this arbitrary
/// is always parseable into *something*, and an unheaded description simply
/// comes back as one overview.
pub fn split(html: &str) -> Sections {
    split_into(html, Section::Overview)
}

/// Splits a fragment whose subject the caller already knows — SmartRecruiters
/// labels its `qualifications` block, Workable its `requirements` — so text
/// under no heading lands in `default` rather than in the overview. Headings
/// *inside* it are still honoured, since a "Preferred" sub-heading inside a
/// qualifications block still means what it says.
pub fn split_into(html: &str, default: Section) -> Sections {
    assemble(flatten(html), default)
}

/// One flattened block of the description.
struct Block {
    text: String,
    heading: bool,
}

/// Walks the fragment into blocks, breaking wherever the markup breaks a line.
fn flatten(html: &str) -> Vec<Block> {
    let doc = Html::parse_fragment(html);
    let mut walker = Walker::default();
    walker.visit(doc.root_element(), false);
    walker.flush(false, false);
    walker.blocks
}

#[derive(Default)]
struct Walker {
    blocks: Vec<Block>,
    buf: String,
    /// Characters seen in the current block, and how many of them were inside
    /// a `<b>`/`<strong>`. A block that is entirely bold is a heading however
    /// it was tagged, which is how most pasted-in descriptions mark sections.
    chars: usize,
    bold_chars: usize,
}

impl Walker {
    /// Recurses over elements only; text is consumed where it is found, which
    /// is what keeps a sentence sitting beside a nested `<ul>` from being lost.
    fn visit(&mut self, el: ElementRef<'_>, bold: bool) {
        let name = el.value().name();
        if matches!(name, "script" | "style" | "noscript" | "svg" | "head") {
            return;
        }
        if name == "br" {
            self.flush(false, false);
            return;
        }

        let heading = matches!(name, "h1" | "h2" | "h3" | "h4" | "h5" | "h6");
        let bullet = name == "li";
        let block = heading || bullet || is_block(name);
        let bold = bold || is_bold(name);

        if block {
            self.flush(false, false);
        }
        for child in el.children() {
            match child.value() {
                Node::Text(text) => self.push_text(text, bold),
                Node::Element(_) => {
                    if let Some(child) = ElementRef::wrap(child) {
                        self.visit(child, bold);
                    }
                }
                _ => {}
            }
        }
        if block {
            self.flush(heading, bullet);
        }
    }

    fn push_text(&mut self, text: &str, bold: bool) {
        let weight = text.chars().filter(|c| !c.is_whitespace()).count();
        self.chars += weight;
        if bold {
            self.bold_chars += weight;
        }
        self.buf.push(' ');
        self.buf.push_str(text);
    }

    /// Ends the current block. `tagged` is true when the markup itself said
    /// heading (`<h1>`…`<h6>`); `bullet` suppresses the styled-heading guess,
    /// because a bold list item is emphasis, not a new section.
    fn flush(&mut self, tagged: bool, bullet: bool) {
        let text = collapse_ws(&std::mem::take(&mut self.buf));
        let (chars, bold_chars) = (self.chars, self.bold_chars);
        self.chars = 0;
        self.bold_chars = 0;
        if text.is_empty() {
            return;
        }

        let styled = !bullet
            && text.len() <= MAX_HEADING
            && ((chars > 0 && bold_chars == chars) || text.ends_with(':'));
        self.blocks.push(Block {
            text,
            heading: tagged || styled,
        });
    }
}

/// Tags that end a line of text. Inline tags (`span`, `a`, `em`) deliberately
/// do not, or every linked word would become its own bullet.
fn is_block(name: &str) -> bool {
    matches!(
        name,
        "p" | "div"
            | "ul"
            | "ol"
            | "section"
            | "article"
            | "table"
            | "tbody"
            | "tr"
            | "td"
            | "th"
            | "dl"
            | "dt"
            | "dd"
            | "blockquote"
            | "header"
            | "footer"
            | "main"
            | "aside"
            | "figure"
            | "pre"
            | "hr"
            | "body"
            | "html"
    )
}

fn is_bold(name: &str) -> bool {
    matches!(name, "b" | "strong")
}

fn assemble(blocks: Vec<Block>, default: Section) -> Sections {
    let mut out = Sections::default();
    let mut current = default;

    for block in blocks {
        if block.heading {
            // Under a known default, an unrecognised heading cannot demote the
            // block to prose — the caller already told us what it is.
            current = match section_of(&block.text) {
                Section::Overview => default,
                named => named,
            };
            // The heading names the section the UI already labels, so it is
            // dropped — except in the overview, where "About Us" is the only
            // thing telling the reader what the paragraph under it is.
            if current == Section::Overview {
                push_line(&mut out.overview, block.text);
            }
            continue;
        }
        push_line(out.of(current), block.text);
    }
    out
}

/// Appends a line, dropping blanks, immediate repeats and anything past the
/// cap. Repeats are common: an ATS that renders both a plain-text and a rich
/// description into one page hands us each line twice.
fn push_line(lines: &mut Vec<String>, text: String) {
    if text.is_empty() || lines.len() >= MAX_LINES || lines.contains(&text) {
        return;
    }
    lines.push(text);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_on_tagged_headings() {
        let s = split(
            r#"<p>We build rockets.</p>
               <h3>Responsibilities</h3><ul><li>Write firmware</li><li>Review code</li></ul>
               <h3>Minimum Qualifications</h3><ul><li>C++</li></ul>
               <h3>Benefits</h3><ul><li>Free lunch</li></ul>"#,
        );
        assert_eq!(s.overview, ["We build rockets."]);
        assert_eq!(s.responsibilities, ["Write firmware", "Review code"]);
        assert_eq!(s.requirements, ["C++"]);
        assert_eq!(s.perks, ["Free lunch"]);
    }

    #[test]
    fn a_fully_bold_paragraph_is_a_heading() {
        // How most descriptions pasted into Workday mark their sections.
        let s = split("<p><b>What You'll Do</b></p><p>Ship features</p>");
        assert_eq!(s.responsibilities, ["Ship features"]);
        assert!(s.overview.is_empty());
    }

    #[test]
    fn a_partly_bold_line_is_not_a_heading() {
        let s = split("<h3>Requirements</h3><ul><li><b>Python</b> — three years</li></ul>");
        assert_eq!(s.requirements, ["Python — three years"]);
    }

    #[test]
    fn a_trailing_colon_opens_a_section() {
        let s = split("<div>Requirements:</div><div>A degree</div>");
        assert_eq!(s.requirements, ["A degree"]);
    }

    #[test]
    fn a_long_line_ending_in_a_colon_is_still_a_line() {
        let long = format!("{}:", "we are looking for someone who ".repeat(4));
        let s = split(&format!("<p>{long}</p>"));
        assert_eq!(s.overview.len(), 1);
    }

    #[test]
    fn an_unrecognised_heading_ends_the_section_before_it() {
        let s = split(
            "<h3>Qualifications</h3><p>A degree</p><h3>Equal Opportunity Employer</h3><p>We do not discriminate</p>",
        );
        assert_eq!(s.requirements, ["A degree"]);
        assert_eq!(s.overview, ["Equal Opportunity Employer", "We do not discriminate"]);
    }

    #[test]
    fn a_section_word_inside_a_slogan_is_not_a_heading_for_it() {
        // Measured on a real iCIMS posting: a bold company slogan opened what
        // the UI then labelled Requirements.
        assert_eq!(section_of("The Best Experience Company"), Section::Overview);
        assert_eq!(section_of("Experience"), Section::Requirements);
        assert_eq!(section_of("Experience:"), Section::Requirements);
        assert_eq!(section_of("Years of experience"), Section::Requirements);
    }

    #[test]
    fn a_heading_naming_two_buckets_takes_the_first_it_names() {
        assert_eq!(section_of("Responsibilities and Qualifications"), Section::Responsibilities);
        assert_eq!(section_of("Compensation and Benefits"), Section::Perks);
        assert_eq!(section_of("Preferred Experience"), Section::Requirements);
    }

    #[test]
    fn an_unheaded_description_is_all_overview() {
        let s = split("<p>Come work with us.</p><p>It is a great place.</p>");
        assert_eq!(s.overview, ["Come work with us.", "It is a great place."]);
        assert!(s.requirements.is_empty());
    }

    #[test]
    fn inline_tags_do_not_break_a_line() {
        let s = split(r##"<p>Work with <a href="#">Rust</a> and <em>Python</em> daily.</p>"##);
        assert_eq!(s.overview, ["Work with Rust and Python daily."]);
    }

    #[test]
    fn br_breaks_a_line() {
        let s = split("<p>First<br/>Second</p>");
        assert_eq!(s.overview, ["First", "Second"]);
    }

    #[test]
    fn text_beside_a_nested_list_is_kept() {
        let s = split("<div>Intro sentence<ul><li>One</li></ul></div>");
        assert_eq!(s.overview, ["Intro sentence", "One"]);
    }

    #[test]
    fn repeated_lines_are_dropped() {
        let s = split("<p>Same line</p><p>Same line</p>");
        assert_eq!(s.overview, ["Same line"]);
    }

    #[test]
    fn a_known_default_keeps_unheaded_text_out_of_the_overview() {
        let s = split_into("<p>A degree</p><h3>Preferred</h3><li>Rust</li>", Section::Requirements);
        assert_eq!(s.requirements, ["A degree", "Rust"]);
        assert!(s.overview.is_empty());
    }

    #[test]
    fn merging_keeps_the_first_copy_of_a_repeated_line() {
        let mut a = split("<h3>Requirements</h3><li>Rust</li>");
        a.merge(split("<h3>Requirements</h3><li>Rust</li><li>Go</li>"));
        assert_eq!(a.requirements, ["Rust", "Go"]);
    }

    #[test]
    fn a_section_is_capped() {
        let html: String = (0..MAX_LINES + 20).map(|i| format!("<li>line {i}</li>")).collect();
        assert_eq!(split(&html).overview.len(), MAX_LINES);
    }
}
