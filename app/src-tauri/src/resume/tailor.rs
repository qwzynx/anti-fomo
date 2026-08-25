//! Choosing what goes on the page for a particular job.
//!
//! The app already reads a posting and works out which catalog skills it asks
//! for (`skills::extract`, surfaced as `Item::required_skills`). Every résumé
//! bullet has been read by the same extractor (`skills::from_text`), so
//! matching a bullet to a posting is a set intersection over one shared
//! vocabulary rather than a guess.
//!
//! Nothing here writes prose. There is no model in this app and no API key, and
//! a feature that silently reworded somebody's job history would be a liability
//! rather than a convenience. What tailoring does is *choose and order*: which
//! bullets earn their line for this posting, and which skills lead the skills
//! block. The user's own words go on the page unchanged.
//!
//! Every choice is a starting position. A [`Variant`] override always wins, and
//! [`Selection::dropped`] records what the page budget cut and why, so the UI
//! can show the trim rather than let it look like data loss.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use serde::{Deserialize, Serialize};

use super::layout;
use super::model::{Entry, Resume, Section, SectionKind};
use super::theme::Theme;
use crate::skills;

/// The user's per-posting overrides. Everything is optional; an empty variant
/// means "whatever the auto pass decides".
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct Variant {
    pub theme: Option<Theme>,
    pub headline: Option<String>,
    /// Entry or bullet id → forced in (`true`, and never trimmed) or out
    /// (`false`). Absent means the auto pass decides.
    pub include: BTreeMap<String, bool>,
    /// Section id → the order its entries should appear in.
    pub order: BTreeMap<String, Vec<String>>,
    /// Bullet id → reworded for this job. The master résumé keeps its wording.
    pub text: BTreeMap<String, String>,
    /// Skills to put first in the skills block, whatever the match says.
    pub skills_lead: Vec<String>,
}

/// Why a bullet was cut.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Dropped {
    pub id: String,
    pub reason: &'static str,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct Selection {
    /// Entry ids that made the page.
    pub entries: HashSet<String>,
    /// Bullet ids that made the page.
    pub bullets: HashSet<String>,
    /// The skills block: one labelled line per entry, in render order.
    pub skills: Vec<(String, Vec<String>)>,
    /// Bullet id → the posting skills it covers. Drives the UI's "why".
    pub why: HashMap<String, Vec<String>>,
    /// What the page budget cut.
    pub dropped: Vec<Dropped>,
    /// Per-posting headline, when the variant set one.
    pub headline: Option<String>,
    /// Section id → entry order, carried through from the variant.
    order: BTreeMap<String, Vec<String>>,
    /// Bullet id → reworded text, carried through from the variant.
    pub text: BTreeMap<String, String>,
    /// How many of the posting's skills the selected bullets speak to.
    pub covered: Vec<String>,
    pub required: Vec<String>,
}

impl Selection {
    /// A section's selected entries, in the order they should render.
    pub fn order_for<'a>(&self, section: &'a Section) -> Vec<&'a Entry> {
        let mut entries: Vec<&Entry> = section
            .entries
            .iter()
            .filter(|e| self.entries.contains(&e.id))
            .collect();
        if let Some(order) = self.order.get(&section.id) {
            let rank: HashMap<&str, usize> = order
                .iter()
                .enumerate()
                .map(|(i, id)| (id.as_str(), i))
                .collect();
            // Anything the override does not name keeps its natural position,
            // after the entries that were explicitly placed.
            entries.sort_by_key(|e| rank.get(e.id.as_str()).copied().unwrap_or(usize::MAX));
        }
        entries
    }

    /// The bullet text to draw, honouring a per-posting rewording.
    pub fn text_of<'a>(&'a self, id: &str, original: &'a str) -> &'a str {
        self.text.get(id).map(String::as_str).unwrap_or(original)
    }
}

/// Everything in the résumé, nothing trimmed. What the builder previews, and
/// the starting point every tailoring pass narrows from.
pub fn everything(resume: &Resume) -> Selection {
    let mut selection = Selection::default();
    for section in &resume.sections {
        for entry in &section.entries {
            selection.entries.insert(entry.id.clone());
            for bullet in &entry.bullets {
                selection.bullets.insert(bullet.id.clone());
            }
        }
    }
    selection.skills = skills_block(resume, &[], &[], &[]);
    selection
}

/// The full pass: pick what speaks to this posting, then trim to the page
/// budget.
pub fn tailor(
    resume: &Resume,
    required: &[String],
    have: &[String],
    variant: &Variant,
    theme: &Theme,
) -> Selection {
    let required_set: HashSet<&str> = required.iter().map(String::as_str).collect();
    let have_set: HashSet<&str> = have.iter().map(String::as_str).collect();

    let mut selection = Selection {
        headline: variant.headline.clone(),
        order: variant.order.clone(),
        text: variant.text.clone(),
        required: required.to_vec(),
        skills: skills_block(resume, required, have, &variant.skills_lead),
        ..Selection::default()
    };

    // Start from everything and take away, rather than building up from the
    // best match. A résumé is a document the user wrote on purpose; the default
    // has to be "all of it", with the trim explained.
    for section in &resume.sections {
        for entry in &section.entries {
            if variant.include.get(&entry.id) == Some(&false) {
                continue;
            }
            selection.entries.insert(entry.id.clone());
            for bullet in &entry.bullets {
                if variant.include.get(&bullet.id) == Some(&false) {
                    continue;
                }
                if bullet.text.trim().is_empty() {
                    continue;
                }
                selection.bullets.insert(bullet.id.clone());
                let covers: Vec<String> = bullet
                    .skills
                    .iter()
                    .filter(|s| required_set.contains(s.as_str()))
                    .cloned()
                    .collect();
                if !covers.is_empty() {
                    selection.why.insert(bullet.id.clone(), covers);
                }
            }
        }
    }

    fit(resume, &mut selection, variant, theme, &have_set);

    // Coverage is reported over what survived the trim, because that is what
    // the reader of the PDF will actually see.
    let mut covered: BTreeSet<String> = BTreeSet::new();
    for (id, skills) in &selection.why {
        if selection.bullets.contains(id) {
            covered.extend(skills.iter().cloned());
        }
    }
    selection.covered = covered.into_iter().collect();
    selection
}

/// The page budget. Lay out, drop the least valuable bullet, lay out again.
///
/// Layout is pure and cheap — no file is written and no font is re-parsed — so
/// asking it repeatedly is the honest way to answer "does this fit", far better
/// than estimating heights and being wrong near the fold.
fn fit(
    resume: &Resume,
    selection: &mut Selection,
    variant: &Variant,
    theme: &Theme,
    have: &HashSet<&str>,
) {
    let budget = theme.sanitized().max_pages as usize;

    // Bounded by construction: every iteration removes one bullet, so the loop
    // cannot run longer than there are bullets to remove.
    let mut guard = selection.bullets.len() + 1;
    while guard > 0 {
        guard -= 1;
        if layout::lay_out(resume, selection, theme).pages.len() <= budget {
            return;
        }
        match weakest(resume, selection, variant, have) {
            Some(id) => {
                selection.bullets.remove(&id);
                selection.dropped.push(Dropped {
                    id,
                    reason: "trimmed to fit the page budget",
                });
            }
            // Nothing left that may be cut. The document runs long, and saying
            // so through an honest page count beats silently dropping a pinned
            // bullet the user asked to keep.
            None => return,
        }
    }
}

/// The bullet whose removal costs the least.
///
/// Value is what a bullet contributes that nothing else selected does: skills
/// the posting asked for and no other surviving bullet covers, weighted up when
/// the user has actually declared the skill. Plus a floor, so a bullet naming
/// no catalog skill at all — "Led a team of four through a migration" — is not
/// free to delete. Ties go to the later bullet, because people put their best
/// one first.
fn weakest(
    resume: &Resume,
    selection: &Selection,
    variant: &Variant,
    have: &HashSet<&str>,
) -> Option<String> {
    // How many surviving bullets cover each required skill, so "unique" means
    // unique among what is still on the page.
    let mut cover_count: HashMap<&str, usize> = HashMap::new();
    for (id, skills) in &selection.why {
        if selection.bullets.contains(id) {
            for skill in skills {
                *cover_count.entry(skill.as_str()).or_default() += 1;
            }
        }
    }

    // Entries down to their last bullet are protected first: trimming an entry
    // to a bare header line loses the reason it is on the résumé at all.
    let mut per_entry: HashMap<&str, usize> = HashMap::new();
    for section in &resume.sections {
        for entry in &section.entries {
            let kept = entry
                .bullets
                .iter()
                .filter(|b| selection.bullets.contains(&b.id))
                .count();
            per_entry.insert(entry.id.as_str(), kept);
        }
    }

    let mut best: Option<(f32, usize, String)> = None;
    for section in &resume.sections {
        for entry in &section.entries {
            let lone = per_entry.get(entry.id.as_str()).copied().unwrap_or(0) <= 1;
            for (position, bullet) in entry.bullets.iter().enumerate() {
                if !selection.bullets.contains(&bullet.id) {
                    continue;
                }
                // Pinned bullets are never trimmed. That is what pinning means.
                if variant.include.get(&bullet.id) == Some(&true) {
                    continue;
                }

                let mut value = 1.0_f32;
                if let Some(covers) = selection.why.get(&bullet.id) {
                    for skill in covers {
                        let unique = cover_count.get(skill.as_str()).copied().unwrap_or(0) <= 1;
                        let weight = if have.contains(skill.as_str()) {
                            3.0
                        } else {
                            2.0
                        };
                        value += if unique { weight } else { weight * 0.25 };
                    }
                }
                // Sorting an entry's only bullet to the back of the queue,
                // rather than forbidding it, keeps the loop able to finish on a
                // résumé that is simply too long.
                if lone {
                    value += 100.0;
                }

                let candidate = (value, usize::MAX - position, bullet.id.clone());
                match &best {
                    Some(current) if (current.0, current.1) <= (candidate.0, candidate.1) => {}
                    _ => best = Some(candidate),
                }
            }
        }
    }
    best.map(|(_, _, id)| id)
}

/// The skills block.
///
/// A curated `Skills` section wins if the résumé has one — its entries are
/// labelled lines, `title` the label and `detail` the comma-separated list.
/// Otherwise the block is generated from the profile the user already filled in
/// for job matching, grouped into the catalog's own categories. One skill list
/// per person, not two that drift.
///
/// Either way the posting's skills lead: within a line, then across lines.
fn skills_block(
    resume: &Resume,
    required: &[String],
    have: &[String],
    lead: &[String],
) -> Vec<(String, Vec<String>)> {
    let priority: HashSet<&str> = required
        .iter()
        .chain(lead.iter())
        .map(String::as_str)
        .collect();

    let curated: Vec<(String, Vec<String>)> = resume
        .sections
        .iter()
        .filter(|s| s.kind == SectionKind::Skills)
        .flat_map(|s| &s.entries)
        .filter_map(|entry| {
            let label = entry.title.trim();
            let list = entry.detail.as_deref().unwrap_or("");
            let items: Vec<String> = list
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect();
            (!label.is_empty() && !items.is_empty()).then(|| (label.to_string(), items))
        })
        .collect();

    let mut lines = if curated.is_empty() {
        // Catalog order, so the generated block reads the way the app's own
        // skill panel does.
        skills::list_skills()
            .into_iter()
            .map(|category| {
                let owned: Vec<String> = category
                    .skills
                    .into_iter()
                    .filter(|s| have.iter().any(|h| h == s))
                    .collect();
                (category.name, owned)
            })
            .filter(|(_, skills)| !skills.is_empty())
            .collect()
    } else {
        curated
    };

    // Lead with what the posting asked for — first inside each line, then by
    // promoting the lines that matched at all.
    for (_, items) in &mut lines {
        items.sort_by_key(|s| !priority.contains(s.as_str()));
    }
    lines.sort_by_key(|(_, items)| !items.iter().any(|s| priority.contains(s.as_str())));
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resume::model::{Bullet, Entry, SectionKind};
    use crate::resume::theme::{Theme, ThemeId, DEFAULT_ACCENT};

    fn bullet(id: &str, text: &str, skills: &[&str]) -> Bullet {
        Bullet {
            id: id.into(),
            text: text.into(),
            skills: skills.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn resume_with(bullets: Vec<Bullet>) -> Resume {
        let mut r = Resume::starter();
        let experience = r
            .sections
            .iter_mut()
            .find(|s| s.kind == SectionKind::Experience)
            .unwrap();
        experience.entries.push(Entry {
            id: "e1".into(),
            org: "Acme Corp".into(),
            title: "Software Engineer Intern".into(),
            location: "Toronto, ON".into(),
            start: "May 2025".into(),
            end: "Aug 2025".into(),
            bullets,
            ..Entry::default()
        });
        r
    }

    fn sample() -> Resume {
        resume_with(vec![
            bullet(
                "b-rust",
                "Shipped a Rust service handling 2,000 rps",
                &["Rust"],
            ),
            bullet("b-pg", "Modelled the schema in PostgreSQL", &["PostgreSQL"]),
            bullet("b-none", "Led a team of four through a migration", &[]),
        ])
    }

    #[test]
    fn everything_selects_everything() {
        let r = sample();
        let s = everything(&r);
        assert_eq!(s.bullets.len(), 3);
        assert!(s.entries.contains("e1"));
    }

    #[test]
    fn matching_bullets_record_why() {
        let r = sample();
        let required = vec!["Rust".to_string(), "PostgreSQL".to_string()];
        let s = tailor(
            &r,
            &required,
            &["Rust".to_string()],
            &Variant::default(),
            &Theme::default(),
        );
        assert_eq!(
            s.why.get("b-rust").map(Vec::as_slice),
            Some(&["Rust".to_string()][..])
        );
        assert_eq!(
            s.why.get("b-pg").map(Vec::as_slice),
            Some(&["PostgreSQL".to_string()][..])
        );
        assert!(
            !s.why.contains_key("b-none"),
            "a bullet naming no skill should have no why"
        );
    }

    #[test]
    fn coverage_reports_what_survived() {
        let r = sample();
        let required = vec!["Rust".into(), "PostgreSQL".into(), "Kubernetes".into()];
        let s = tailor(&r, &required, &[], &Variant::default(), &Theme::default());
        assert_eq!(
            s.covered,
            vec!["PostgreSQL".to_string(), "Rust".to_string()]
        );
        assert_eq!(s.required.len(), 3);
    }

    /// The override contract: an explicit `false` beats the auto pass.
    #[test]
    fn an_excluded_bullet_stays_out() {
        let r = sample();
        let mut variant = Variant::default();
        variant.include.insert("b-rust".into(), false);
        let s = tailor(&r, &["Rust".to_string()], &[], &variant, &Theme::default());
        assert!(!s.bullets.contains("b-rust"));
        assert!(s.bullets.contains("b-pg"));
    }

    #[test]
    fn an_excluded_entry_takes_its_bullets_with_it() {
        let r = sample();
        let mut variant = Variant::default();
        variant.include.insert("e1".into(), false);
        let s = tailor(&r, &[], &[], &variant, &Theme::default());
        assert!(!s.entries.contains("e1"));
        assert!(s.bullets.is_empty());
    }

    #[test]
    fn a_short_resume_keeps_every_bullet() {
        let r = sample();
        let s = tailor(
            &r,
            &["Rust".to_string()],
            &[],
            &Variant::default(),
            &Theme::default(),
        );
        assert_eq!(s.bullets.len(), 3);
        assert!(s.dropped.is_empty());
    }

    /// The fit loop has to converge and has to respect the budget.
    #[test]
    fn an_overlong_resume_is_trimmed_to_the_budget() {
        let many: Vec<Bullet> = (0..90)
            .map(|n| {
                bullet(
                    &format!("b{n}"),
                    "Rebuilt the ingest path end to end and held the latency budget with a \
                     regression test that fails the build when the query count moves",
                    &[],
                )
            })
            .collect();
        let r = resume_with(many);
        let s = tailor(&r, &[], &[], &Variant::default(), &Theme::default());
        assert!(s.bullets.len() < 90, "nothing was trimmed");
        assert!(!s.dropped.is_empty(), "a trim was not recorded");
        let pages = layout::lay_out(&r, &s, &Theme::default()).pages.len();
        assert_eq!(pages, 1, "trimmed selection still runs to {pages} pages");
    }

    /// Trimming must prefer the bullets that say nothing about the job.
    #[test]
    fn trimming_keeps_the_bullets_that_match() {
        let long = "Rebuilt the ingest path end to end and held the latency budget with a \
                    regression test that fails the build when the query count moves";
        let mut bullets = vec![bullet("b-keep", &format!("Rust: {long}"), &["Rust"])];
        bullets.extend((0..80).map(|n| bullet(&format!("b{n}"), long, &[])));
        let r = resume_with(bullets);
        let s = tailor(
            &r,
            &["Rust".to_string()],
            &["Rust".to_string()],
            &Variant::default(),
            &Theme::default(),
        );
        assert!(
            s.bullets.contains("b-keep"),
            "the one matching bullet was trimmed"
        );
    }

    /// Pinning is the escape hatch, so it has to survive the trim.
    #[test]
    fn a_pinned_bullet_survives_the_trim() {
        let long = "Rebuilt the ingest path end to end and held the latency budget with a \
                    regression test that fails the build when the query count moves";
        let mut bullets = vec![bullet("b-pin", long, &[])];
        bullets.extend((0..80).map(|n| bullet(&format!("b{n}"), long, &[])));
        let r = resume_with(bullets);
        let mut variant = Variant::default();
        variant.include.insert("b-pin".into(), true);
        let s = tailor(&r, &[], &[], &variant, &Theme::default());
        assert!(s.bullets.contains("b-pin"), "a pinned bullet was trimmed");
    }

    #[test]
    fn a_bigger_budget_keeps_more() {
        let long = "Rebuilt the ingest path end to end and held the latency budget with a \
                    regression test that fails the build when the query count moves";
        let r = resume_with(
            (0..80)
                .map(|n| bullet(&format!("b{n}"), long, &[]))
                .collect(),
        );
        let one = tailor(
            &r,
            &[],
            &[],
            &Variant::default(),
            &Theme {
                max_pages: 1,
                ..Theme::default()
            },
        );
        let three = tailor(
            &r,
            &[],
            &[],
            &Variant::default(),
            &Theme {
                max_pages: 3,
                ..Theme::default()
            },
        );
        assert!(three.bullets.len() > one.bullets.len());
    }

    #[test]
    fn generated_skills_come_from_the_profile_and_lead_with_matches() {
        let r = sample();
        let have = vec![
            "Python".to_string(),
            "Rust".to_string(),
            "React".to_string(),
        ];
        let s = tailor(
            &r,
            &["React".to_string()],
            &have,
            &Variant::default(),
            &Theme::default(),
        );
        assert!(!s.skills.is_empty(), "no skills block was generated");
        let flat: Vec<&str> = s
            .skills
            .iter()
            .flat_map(|(_, v)| v.iter().map(String::as_str))
            .collect();
        assert!(flat.contains(&"Rust") && flat.contains(&"React"));
        assert!(
            !flat.contains(&"Java"),
            "a skill the user never declared appeared"
        );
        assert_eq!(s.skills[0].1[0], "React", "the matching skill did not lead");
    }

    #[test]
    fn a_curated_skills_section_wins_over_the_profile() {
        let mut r = sample();
        let skills_section = r
            .sections
            .iter_mut()
            .find(|s| s.kind == SectionKind::Skills)
            .unwrap();
        skills_section.entries.push(Entry {
            id: "s1".into(),
            title: "Languages".into(),
            detail: Some("Rust, TypeScript, Figma".into()),
            ..Entry::default()
        });
        let s = tailor(
            &r,
            &["TypeScript".to_string()],
            &["Python".to_string()],
            &Variant::default(),
            &Theme::default(),
        );
        assert_eq!(s.skills.len(), 1);
        assert_eq!(s.skills[0].0, "Languages");
        // Curated lines carry things the catalog has never heard of.
        assert!(s.skills[0].1.contains(&"Figma".to_string()));
        assert_eq!(
            s.skills[0].1[0], "TypeScript",
            "the matching skill did not lead"
        );
    }

    #[test]
    fn entry_order_follows_the_variant() {
        let mut r = sample();
        let experience = r
            .sections
            .iter_mut()
            .find(|s| s.kind == SectionKind::Experience)
            .unwrap();
        experience.id = "sec".into();
        let second = Entry {
            id: "e2".into(),
            org: "Beta".into(),
            ..Entry::default()
        };
        experience.entries.push(second);

        let mut variant = Variant::default();
        variant
            .order
            .insert("sec".into(), vec!["e2".into(), "e1".into()]);
        let s = tailor(&r, &[], &[], &variant, &Theme::default());
        let section = r.sections.iter().find(|s| s.id == "sec").unwrap();
        let ids: Vec<&str> = s.order_for(section).iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, vec!["e2", "e1"]);
    }

    #[test]
    fn reworded_text_is_used_for_this_posting_only() {
        let r = sample();
        let mut variant = Variant::default();
        variant
            .text
            .insert("b-rust".into(), "Rewritten for this job".into());
        let s = tailor(&r, &[], &[], &variant, &Theme::default());
        assert_eq!(s.text_of("b-rust", "original"), "Rewritten for this job");
        assert_eq!(s.text_of("b-pg", "original"), "original");
        // The master résumé is untouched.
        assert_eq!(
            r.bullet("b-rust").unwrap().text,
            "Shipped a Rust service handling 2,000 rps"
        );
    }

    #[test]
    fn every_theme_converges() {
        let long = "Rebuilt the ingest path end to end and held the latency budget";
        let r = resume_with(
            (0..60)
                .map(|n| bullet(&format!("b{n}"), long, &[]))
                .collect(),
        );
        for id in ThemeId::ALL {
            let theme = Theme::preset(id, DEFAULT_ACCENT);
            let s = tailor(&r, &[], &[], &Variant::default(), &theme);
            assert_eq!(
                layout::lay_out(&r, &s, &theme).pages.len(),
                1,
                "{id:?} did not converge"
            );
        }
    }
}
