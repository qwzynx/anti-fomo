//! Does the résumé PDF actually say what it looks like it says?
//!
//! Run with:
//!   cargo run --features dev-tools --bin resume_check
//!
//! A résumé is read twice: once by a person, and once — first, and often
//! decisively — by an applicant tracking system running a text extractor. A PDF
//! that renders perfectly and extracts as nothing is worse than no feature at
//! all, because the failure is invisible from inside the app.
//!
//! So this checks the output the way an ATS would: it writes real files and
//! runs **`pdftotext`** over them. It deliberately does *not* trust
//! `printpdf::extract_text`, which replays the ops it was handed rather than
//! parsing the file back. That distinction is not academic. An early version of
//! `resume::pdf` positioned text with `Td` (a *relative* move) instead of `Tm`,
//! so every line after the first landed at a compounding offset and the third
//! ran off the page. `extract_text` reported all three lines, happily.
//! `pdftotext` saw one. The renderer is the authority; the writer is not.
//!
//! `pdftotext` ships with poppler (`poppler-utils` on Debian, `poppler` on
//! Arch). Without it the geometry checks still run and the extraction check
//! reports itself skipped rather than passing quietly.

use std::collections::BTreeSet;
use std::process::Command;

use anti_fomo_lib::resume::layout::{Draw, Page};
use anti_fomo_lib::resume::model::{Bullet, Contact, Entry, Link, Resume, SectionKind};
use anti_fomo_lib::resume::theme::{FamilyId, FontStyle};
use anti_fomo_lib::resume::theme::{PageSize, Theme, ThemeId, DEFAULT_ACCENT};
use anti_fomo_lib::resume::{self, layout, pdf, tailor};

/// A résumé with the shapes that actually break layouts: an accented name, a
/// long role title next to a long date range, a bullet that has to wrap three
/// times, and a project line full of punctuation.
fn fixture() -> Resume {
    let mut r = Resume::starter();
    r.contact = Contact {
        name: "Ada Lovelace-Bräuer".into(),
        headline: Some("Software Engineer".into()),
        email: "ada@example.com".into(),
        phone: "+1 416 555 0100".into(),
        location: "Toronto, ON".into(),
        links: vec![
            Link {
                label: "GitHub".into(),
                url: "https://github.com/adalovelace".into(),
            },
            Link {
                label: "LinkedIn".into(),
                url: "https://linkedin.com/in/adalovelace".into(),
            },
        ],
    };

    let put = |r: &mut Resume, kind: SectionKind, entry: Entry| {
        r.sections
            .iter_mut()
            .find(|s| s.kind == kind)
            .expect("starter section")
            .entries
            .push(entry);
    };

    put(
        &mut r,
        SectionKind::Education,
        Entry {
            id: "edu".into(),
            org: "York University".into(),
            title: "BSc, Computer Science".into(),
            location: "Toronto, ON".into(),
            start: "Sep 2023".into(),
            end: "Apr 2027".into(),
            detail: Some("GPA 3.9/4.0 — Dean's Honour Roll".into()),
            ..Entry::default()
        },
    );
    put(
        &mut r,
        SectionKind::Experience,
        Entry {
            id: "exp".into(),
            org: "Acme Corporation".into(),
            title: "Software Engineering Intern, Platform".into(),
            location: "Toronto, ON".into(),
            start: "May 2025".into(),
            end: "Aug 2025".into(),
            bullets: vec![
                Bullet::new(
                    "Cut p95 latency 40% by replacing the N+1 query path with a single windowed \
                     aggregate, then held the budget with a regression test that fails the build \
                     when the query count moves.",
                ),
                Bullet::new(
                    "Shipped a Rust service handling 2,000 requests per second on one core.",
                ),
                Bullet::new("Led a team of four through a Postgres migration with no downtime."),
            ],
            ..Entry::default()
        },
    );
    put(
        &mut r,
        SectionKind::Projects,
        Entry {
            id: "proj".into(),
            org: "Anti-FOMO".into(),
            title: "Personal project".into(),
            link: Some("https://github.com/adalovelace/anti-fomo".into()),
            detail: Some("Rust · Tauri · Svelte · SQLite".into()),
            bullets: vec![Bullet::new(
                "Ranked 18,000 scraped postings on-device in under a second using SQLite & \
                 Aho-Corasick keyword extraction.",
            )],
            ..Entry::default()
        },
    );
    put(
        &mut r,
        SectionKind::Skills,
        Entry {
            id: "sk1".into(),
            title: "Languages".into(),
            detail: Some("Rust, TypeScript, Python, SQL".into()),
            ..Entry::default()
        },
    );
    r.normalize();
    r
}

/// Every string that has to survive the trip to a text extractor.
fn must_extract(r: &Resume) -> Vec<String> {
    let mut wanted = vec![r.contact.name.to_uppercase(), r.contact.email.clone()];
    for section in &r.sections {
        if section.entries.is_empty() {
            continue;
        }
        wanted.push(section.title.to_uppercase());
        for entry in &section.entries {
            if !entry.org.trim().is_empty() {
                wanted.push(entry.org.clone());
            }
            for bullet in &entry.bullets {
                // First few words: enough to prove the run is there, short
                // enough to survive line wrapping, which puts a newline in the
                // middle of longer phrases.
                let head: String = bullet
                    .text
                    .split_whitespace()
                    .take(4)
                    .collect::<Vec<_>>()
                    .join(" ");
                wanted.push(head);
            }
        }
    }
    wanted
}

/// The same page, as the webview draws it.
///
/// This mirrors `ResumePreview.svelte` exactly — same boxes, same baseline
/// semantics, same font stack — so rasterising this beside the PDF is a direct
/// test of the claim the whole feature rests on: that the preview *is* the
/// file, not a drawing of it. Written next to each PDF so the two can be put
/// side by side.
fn page_to_svg(page: &Page) -> String {
    let family = |f: FamilyId| match f {
        FamilyId::Serif => "Resume Serif, Liberation Serif, Times New Roman, serif",
        FamilyId::Sans => "Resume Sans, Inter, sans-serif",
    };
    let weight = |s: FontStyle| if s.bold() { 700 } else { 400 };
    let slant = |s: FontStyle| if s.italic() { "italic" } else { "normal" };
    let escape = |t: &str| {
        t.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    };

    let mut out = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {w} {h}\" width=\"{w}\" height=\"{h}\">\n\
         <rect x=\"0\" y=\"0\" width=\"{w}\" height=\"{h}\" fill=\"#ffffff\"/>\n",
        w = page.width,
        h = page.height
    );
    for item in &page.items {
        match item {
            Draw::Rect { x, y, w, h, color } => out.push_str(&format!(
                "<rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\" fill=\"rgb({} {} {})\"/>\n",
                color.r, color.g, color.b
            )),
            Draw::Text {
                x,
                y,
                size,
                family: f,
                style,
                color,
                tracking,
                text,
            } => {
                let spacing = if *tracking != 0.0 {
                    format!(" letter-spacing=\"{tracking}\"")
                } else {
                    String::new()
                };
                out.push_str(&format!(
                    "<text x=\"{x}\" y=\"{y}\" font-family=\"{}\" font-size=\"{size}\" \
                     font-weight=\"{}\" font-style=\"{}\" fill=\"rgb({} {} {})\"{spacing} \
                     xml:space=\"preserve\">{}</text>\n",
                    family(*f),
                    weight(*style),
                    slant(*style),
                    color.r,
                    color.g,
                    color.b,
                    escape(text)
                ));
            }
            Draw::Link { .. } => {}
        }
    }
    out.push_str("</svg>\n");
    out
}

fn pdftotext(path: &std::path::Path) -> Option<String> {
    let out = Command::new("pdftotext")
        .arg("-layout")
        .arg(path)
        .arg("-")
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).to_string())
}

fn main() {
    let resume = fixture();
    let dir = std::env::temp_dir().join("anti-fomo-resume-check");
    std::fs::create_dir_all(&dir).expect("create the output directory");

    let have_pdftotext = Command::new("pdftotext").arg("-v").output().is_ok();
    if !have_pdftotext {
        eprintln!(
            "! pdftotext not on PATH — geometry is still checked, extraction is not.\n\
             ! Install poppler to run the check that matters.\n"
        );
    }

    println!(
        "{:<22} {:>6} {:>7} {:>8} {:>6}  extraction",
        "theme / page", "pages", "KB", "fill", "links"
    );
    println!("{}", "-".repeat(78));

    let mut failures: Vec<String> = Vec::new();

    for id in ThemeId::ALL {
        for page in [PageSize::Letter, PageSize::A4] {
            let theme = Theme {
                page,
                ..Theme::preset(id, DEFAULT_ACCENT)
            };
            let laid_out = resume::preview(&resume, &theme);
            let bytes = pdf::render(&laid_out, "Ada Lovelace — Résumé");

            let label = format!("{} / {:?}", id.label(), page);
            let name = format!("{}-{:?}.pdf", id.label().to_lowercase(), page);
            let path = dir.join(&name);
            std::fs::write(&path, &bytes).expect("write the pdf");

            // The preview's own rendering of the same boxes, for a side-by-side.
            let svg_path = dir.join(name.replace(".pdf", ".svg"));
            std::fs::write(&svg_path, page_to_svg(&laid_out.pages[0])).expect("write the svg");

            // Geometry: nothing may sit outside the printable area. A box past
            // the margin is a clipped word on somebody's résumé.
            for p in &laid_out.pages {
                for item in &p.items {
                    if let layout::Draw::Text { x, y, .. } = item {
                        if *x < theme.margin - 6.0 || *y > p.height - theme.margin + 6.0 {
                            failures
                                .push(format!("{label}: text outside the margins at ({x}, {y})"));
                        }
                    }
                }
            }

            let extraction = match (have_pdftotext, pdftotext(&path)) {
                (true, Some(text)) => {
                    let flat: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
                    let missing: Vec<String> = must_extract(&resume)
                        .into_iter()
                        .filter(|want| {
                            let want: String =
                                want.split_whitespace().collect::<Vec<_>>().join(" ");
                            !flat.contains(&want)
                        })
                        .collect();
                    if missing.is_empty() {
                        "ok".to_string()
                    } else {
                        for m in &missing {
                            failures.push(format!("{label}: {m:?} did not extract"));
                        }
                        format!("{} MISSING", missing.len())
                    }
                }
                (true, None) => {
                    failures.push(format!("{label}: pdftotext could not read the file"));
                    "unreadable".to_string()
                }
                _ => "skipped".to_string(),
            };

            // Links are an /Annots array on the page, not content-stream ops,
            // so a PDF can render perfectly with every link silently inert.
            let expected_links = laid_out.pages[0]
                .items
                .iter()
                .filter(|d| matches!(d, layout::Draw::Link { .. }))
                .count();
            let annots = bytes.windows(6).filter(|w| *w == b"/Annot").count();
            if expected_links > 0 && annots == 0 {
                failures.push(format!(
                    "{label}: {expected_links} link(s) laid out, none reached the PDF"
                ));
            }

            println!(
                "{:<22} {:>6} {:>7} {:>7.0}% {:>6}  {}",
                label,
                laid_out.pages.len(),
                bytes.len() / 1024,
                laid_out.fill * 100.0,
                expected_links,
                extraction
            );
        }
    }

    // Tailoring: a posting asking for Rust and Postgres should keep the bullets
    // that say Rust and Postgres, and should say which ones did it.
    println!("\ntailoring against a posting asking for Rust, PostgreSQL, Kubernetes:");
    let required: Vec<String> = ["Rust", "PostgreSQL", "Kubernetes"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let have: Vec<String> = ["Rust", "PostgreSQL", "SQL", "TypeScript"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let (_, selection) = resume::tailored(
        &resume,
        &required,
        &have,
        &tailor::Variant::default(),
        &Theme::default(),
    );
    let covered: BTreeSet<&str> = selection.covered.iter().map(String::as_str).collect();
    println!(
        "  covered {}/{}: {:?}",
        covered.len(),
        required.len(),
        covered
    );
    for (id, skills) in &selection.why {
        let text = resume.bullet(id).map(|b| b.text.as_str()).unwrap_or("");
        let head: String = text
            .split_whitespace()
            .take(6)
            .collect::<Vec<_>>()
            .join(" ");
        println!("  {skills:?} <- {head}…");
    }
    if selection.why.is_empty() {
        failures.push("tailoring matched no bullet against a posting it should have".into());
    }

    println!("\nfiles in {}", dir.display());
    if failures.is_empty() {
        println!("all checks passed");
    } else {
        eprintln!("\n{} FAILURE(S):", failures.len());
        for f in &failures {
            eprintln!("  - {f}");
        }
        std::process::exit(1);
    }
}
