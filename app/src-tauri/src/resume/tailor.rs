//! Choosing what goes on the page for a particular job.
//!
//! The app already reads a posting and works out which catalog skills it asks
//! for and how hard (`skills::demand`, surfaced as `Item::required_skills`).
//! Every résumé bullet has been read by the same extractor
//! (`skills::from_text`), so matching a bullet to a posting is a set
//! intersection over one shared vocabulary rather than a guess.
//!
//! That vocabulary is about a hundred and fifty named technologies, and a
//! posting says far more than that. So a bullet is scored against a posting
//! twice: once on the catalog skills they share, and once on how much of the
//! posting's own language it speaks — see [`super::terms`]. A bullet about an
//! on-call rotation names no catalog skill at all and is exactly the bullet an
//! SRE posting wants kept.
//!
//! Nothing here writes prose. There is no model in this app and no API key, and
//! a feature that silently reworded somebody's job history would be a liability
//! rather than a convenience. What tailoring does is *choose and order*: which
//! bullets earn their line for this posting, which entries lead the sections
//! that have no chronology to defend, and which skills lead the skills block.
//! The user's own words go on the page unchanged.
//!
//! Every choice is a starting position. A [`Variant`] override always wins, and
//! [`Selection::dropped`] records what the page budget cut and why, so the UI
//! can show the trim rather than let it look like data loss.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use serde::{Deserialize, Serialize};

use super::layout;
use super::model::{Entry, Resume, Section, SectionKind};
use super::terms::Focus;
use super::theme::Theme;
use crate::skills::{self, Emphasis};

/// The user's per-posting overrides. Everything is optional; an empty variant
/// means "whatever the auto pass decides".
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct Variant {
    pub theme: Option<Theme>,
    pub headline: Option<String>,
    /// Section, entry or bullet id → forced in (`true`, and never trimmed) or
    /// out (`false`). Absent means the auto pass decides.
    ///
    /// All three levels, because what is worth putting in front of one
    /// employer is not what is worth putting in front of another: a project
    /// that earns its space on a graphics posting is dead weight on a data
    /// one, and the whole Leadership section can be the wrong thing to spend
    /// six lines on. Only bullets have a meaningful `true`; sections and
    /// entries are never trimmed to fit, so for them this map is a way to say
    /// "not this time" and nothing else.
    pub include: BTreeMap<String, bool>,
    /// Section id → the order its entries should appear in.
    pub order: BTreeMap<String, Vec<String>>,
    /// Bullet id → reworded for this job. The master résumé keeps its wording.
    pub text: BTreeMap<String, String>,
    /// Skills to put first in the skills block, whatever the match says.
    pub skills_lead: Vec<String>,
}

/// The posting a résumé is being tailored to, as everything the choosing needs
/// to know about it.
///
/// Assembled by the caller rather than read here, because `resume/` does not
/// know what an `Item` is and should not learn: the same tailoring runs against
/// a live posting, against a saved snapshot whose text has been pruned, and
/// against a fixture in a test.
#[derive(Debug, Clone, Default)]
pub struct Target {
    /// Skill → how much this posting wants it, normalised so its strongest ask
    /// is 1.0. Normalised because the numbers only ever compare *within* one
    /// posting: a terse Greenhouse blurb and a 9 KB Workday page do not score
    /// on one scale, and nothing here compares two postings.
    demand: HashMap<String, f32>,
    /// The asks, in catalog order — the coverage row's "6 of 9".
    names: Vec<String>,
    /// The subset the posting treats as non-negotiable.
    must: Vec<String>,
    focus: Focus,
}

/// A skill the posting asked for, in a bullet.
const SKILL_WEIGHT: f32 = 2.0;
/// A skill the posting asked for that only this bullet covers.
const UNIQUE_WEIGHT: f32 = 1.5;
/// How much a bullet the user has actually declared the skill for is worth
/// over one that merely mentions it.
const DECLARED: f32 = 1.3;
/// Speaking the posting's own language, scaled against the skill terms above.
const ECHO_WEIGHT: f32 = 1.5;
/// A number in a bullet. Small, and not about this posting at all — "cut p99
/// latency by 40%" is a better line than "improved latency" in front of
/// anybody, and when two bullets are otherwise tied that is the tiebreak.
const QUANTIFIED: f32 = 0.35;
/// What a bullet is worth for existing. Keeps a bullet that speaks to nothing
/// — "Led a team of four through a migration" — from being free to delete.
const FLOOR: f32 = 0.5;

impl Target {
    /// A posting whose page could not be read. Everything scores zero, so the
    /// résumé keeps its own order and only the page budget trims it.
    pub fn none() -> Target {
        Target::default()
    }

    /// The full reading: what the posting asks for, and what it keeps coming
    /// back to.
    pub fn new(demand: &[skills::Demand], focus: Focus) -> Target {
        let peak = demand
            .iter()
            .map(|d| d.weight)
            .fold(0.0f32, f32::max)
            .max(f32::EPSILON);
        Target {
            demand: demand
                .iter()
                .map(|d| (d.skill.clone(), (d.weight / peak).clamp(0.0, 1.0)))
                .collect(),
            names: demand.iter().map(|d| d.skill.clone()).collect(),
            must: demand
                .iter()
                .filter(|d| d.emphasis == Emphasis::Must)
                .map(|d| d.skill.clone())
                .collect(),
            focus,
        }
    }

    /// A posting known only by the skills it asks for, weighted alike. What a
    /// saved posting whose text has been pruned falls back to, and what the
    /// tests build.
    pub fn for_skills(skills: &[String]) -> Target {
        Target {
            demand: skills.iter().map(|s| (s.clone(), 1.0)).collect(),
            names: skills.to_vec(),
            must: skills.to_vec(),
            focus: Focus::empty(),
        }
    }

    pub fn skills(&self) -> &[String] {
        &self.names
    }

    pub fn is_empty(&self) -> bool {
        self.names.is_empty() && self.focus.is_empty()
    }

    /// How much this posting wants a skill, 0.0 when it never asked.
    fn wants(&self, skill: &str) -> f32 {
        self.demand.get(skill).copied().unwrap_or(0.0)
    }

    /// How much a bullet says to this posting.
    fn relevance(&self, skills: &[String], text: &str, have: &HashSet<&str>) -> f32 {
        let mut score = 0.0;
        for skill in skills {
            let want = self.wants(skill);
            if want <= 0.0 {
                continue;
            }
            // A skill the user has declared on their profile is a claim they
            // can stand behind in the interview; one that only turned up in
            // the bullet's own text is a keyword match.
            let declared = match have.contains(skill.as_str()) {
                true => DECLARED,
                false => 1.0,
            };
            score += SKILL_WEIGHT * want * declared;
        }
        // Logarithmic, so a bullet that shares four of the posting's terms
        // beats one that shares two without a bullet that shares forty
        // sweeping the page. Overlap is evidence, not a quantity.
        score += ECHO_WEIGHT * (1.0 + self.focus.echo(text)).ln();
        if text.bytes().any(|b| b.is_ascii_digit()) {
            score += QUANTIFIED;
        }
        score
    }
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
    /// Bullet id → the posting's own words it echoes, for the bullets that
    /// speak to the job without naming a catalog skill. Kept apart from `why`
    /// because these are a weaker kind of evidence and the UI should not let
    /// them read as "covers this requirement".
    pub echo: HashMap<String, Vec<String>>,
    /// What the page budget cut.
    pub dropped: Vec<Dropped>,
    /// Per-posting headline, when the variant set one.
    pub headline: Option<String>,
    /// Section id → entry order. The variant's, where it named one; otherwise
    /// what the relevance pass worked out for the sections that have no
    /// chronology to defend.
    order: BTreeMap<String, Vec<String>>,
    /// Bullet id → reworded text, carried through from the variant.
    pub text: BTreeMap<String, String>,
    /// How many of the posting's skills the selected bullets speak to.
    pub covered: Vec<String>,
    pub required: Vec<String>,
    /// The subset of `required` the posting treats as non-negotiable.
    pub must: Vec<String>,
    /// What the posting keeps coming back to, in its own words. Shown so the
    /// reader can check what the tailoring aimed at instead of trusting it.
    pub focus: Vec<String>,
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
    selection.skills = skills_block(resume, &Target::none(), &[], &[], &BTreeMap::new());
    selection
}

/// The full pass: pick what speaks to this posting, order what has no order to
/// defend, then trim to the page budget.
pub fn tailor(
    resume: &Resume,
    target: &Target,
    have: &[String],
    variant: &Variant,
    theme: &Theme,
) -> Selection {
    let have_set: HashSet<&str> = have.iter().map(String::as_str).collect();

    let mut selection = Selection {
        headline: variant.headline.clone(),
        order: variant.order.clone(),
        text: variant.text.clone(),
        required: target.names.clone(),
        must: target.must.clone(),
        focus: target.focus.top().to_vec(),
        skills: skills_block(resume, target, have, &variant.skills_lead, &variant.include),
        ..Selection::default()
    };

    // Start from everything and take away, rather than building up from the
    // best match. A résumé is a document the user wrote on purpose; the default
    // has to be "all of it", with the trim explained.
    let mut relevance: HashMap<String, f32> = HashMap::new();
    for section in &resume.sections {
        // A section switched off takes its entries with it. Layout draws no
        // heading over nothing, so this is all it takes to leave one out.
        if variant.include.get(&section.id) == Some(&false) {
            continue;
        }
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

                // Scored on the text that will actually be drawn: a bullet the
                // user reworded for this posting is the wording the posting
                // gets read against.
                let text = selection.text_of(&bullet.id, &bullet.text).to_string();
                relevance.insert(
                    bullet.id.clone(),
                    target.relevance(&bullet.skills, &text, &have_set),
                );

                let covers: Vec<String> = bullet
                    .skills
                    .iter()
                    .filter(|s| target.wants(s) > 0.0)
                    .cloned()
                    .collect();
                if !covers.is_empty() {
                    selection.why.insert(bullet.id.clone(), covers);
                }
                let echoes = target.focus.shared(&text, 3);
                if !echoes.is_empty() {
                    selection.echo.insert(bullet.id.clone(), echoes);
                }
            }
        }
    }

    promote(resume, &mut selection, target, variant, &relevance);
    fit(resume, &mut selection, variant, theme, target, &relevance);

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

/// Sections whose entries have no chronology to defend, so putting the
/// relevant one first is what tailoring means.
///
/// Experience and Education keep the order the user wrote them in. A résumé
/// that lists last summer's internship above this year's reads as a mistake
/// however relevant last summer's was, and reordering a work history is the
/// one rearrangement a recruiter will notice and hold against the document.
fn reorderable(kind: SectionKind) -> bool {
    matches!(
        kind,
        SectionKind::Projects | SectionKind::Leadership | SectionKind::Awards | SectionKind::Custom
    )
}

/// Leads the reorderable sections with the entry that speaks to this posting.
///
/// Only where the variant has not already said what the order is, and only
/// when something actually scored: a posting we could not read must not
/// shuffle a résumé for no reason.
fn promote(
    resume: &Resume,
    selection: &mut Selection,
    target: &Target,
    variant: &Variant,
    relevance: &HashMap<String, f32>,
) {
    if target.is_empty() {
        return;
    }
    // An entry is as good as its best bullet, plus a fraction of the rest: two
    // relevant bullets beat one, but a long list of weak ones does not add up
    // to a strong entry.
    let score = |entry: &Entry| -> f32 {
        let mut best = 0.0f32;
        let mut total = 0.0f32;
        for bullet in &entry.bullets {
            let Some(value) = relevance.get(&bullet.id) else {
                continue;
            };
            best = best.max(*value);
            total += *value;
        }
        best + 0.3 * (total - best)
    };

    for section in &resume.sections {
        if !reorderable(section.kind) || variant.order.contains_key(&section.id) {
            continue;
        }
        let mut entries: Vec<&Entry> = section
            .entries
            .iter()
            .filter(|e| selection.entries.contains(&e.id))
            .collect();
        if entries.len() < 2 || entries.iter().all(|e| score(e) <= 0.0) {
            continue;
        }
        // A stable sort, so entries that tie keep the order the user wrote.
        entries.sort_by(|a, b| score(b).total_cmp(&score(a)));
        selection.order.insert(
            section.id.clone(),
            entries.iter().map(|e| e.id.clone()).collect(),
        );
    }
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
    target: &Target,
    relevance: &HashMap<String, f32>,
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
        match weakest(resume, selection, variant, target, relevance) {
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
/// Value is [`Target::relevance`] — the posting's skills it names and the
/// posting's language it speaks — plus what it contributes that nothing else
/// still on the page does: a skill the posting asked for and no surviving
/// bullet covers is worth keeping a line for even when the bullet is otherwise
/// unremarkable. Plus a floor, so a bullet naming nothing at all is not free
/// to delete. Ties go to the later bullet, because people put their best one
/// first.
fn weakest(
    resume: &Resume,
    selection: &Selection,
    variant: &Variant,
    target: &Target,
    relevance: &HashMap<String, f32>,
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

                let mut value = FLOOR + relevance.get(&bullet.id).copied().unwrap_or(0.0);
                if let Some(covers) = selection.why.get(&bullet.id) {
                    for skill in covers {
                        let unique = cover_count.get(skill.as_str()).copied().unwrap_or(0) <= 1;
                        if unique {
                            value += UNIQUE_WEIGHT * target.wants(skill);
                        }
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
/// Either way the posting's skills lead: within a line, then across lines, and
/// the ones it called non-negotiable lead the ones it merely preferred.
fn skills_block(
    resume: &Resume,
    target: &Target,
    have: &[String],
    lead: &[String],
    include: &BTreeMap<String, bool>,
) -> Vec<(String, Vec<String>)> {
    let kept = |id: &String| include.get(id) != Some(&false);
    // An explicit lead outranks everything the posting asked for; after that
    // it is how hard the posting asked.
    let rank = |skill: &str| -> f32 {
        match lead.iter().any(|s| s == skill) {
            true => 2.0,
            false => target.wants(skill),
        }
    };

    let curated: Vec<(String, Vec<String>)> = resume
        .sections
        .iter()
        .filter(|s| s.kind == SectionKind::Skills && kept(&s.id))
        .flat_map(|s| &s.entries)
        // A skills line is an entry like any other, and it is the one a
        // posting most often wants a different half of. Excluding it here
        // rather than only in the entry loop is what makes it disappear: the
        // block is built from the résumé, not from `selection.entries`.
        .filter(|entry| kept(&entry.id))
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
    // promoting the lines that matched at all. Both sorts are stable, so
    // everything the posting did not name keeps the order the user wrote.
    for (_, items) in &mut lines {
        items.sort_by(|a, b| rank(b).total_cmp(&rank(a)));
    }
    lines.sort_by(|(_, a), (_, b)| {
        let best = |items: &[String]| items.iter().map(|s| rank(s)).fold(0.0f32, f32::max);
        best(b).total_cmp(&best(a))
    });
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resume::model::{Bullet, Entry, SectionKind};
    use crate::resume::theme::{Theme, ThemeId, DEFAULT_ACCENT};

    /// A posting known only by the skills it asks for — no page text, so the
    /// term overlap contributes nothing and the skill match is the whole score.
    fn asking(skills: &[&str]) -> Target {
        Target::for_skills(&skills.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }

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
        let s = tailor(
            &r,
            &asking(&["Rust", "PostgreSQL"]),
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
        let target = asking(&["Rust", "PostgreSQL", "Kubernetes"]);
        let s = tailor(&r, &target, &[], &Variant::default(), &Theme::default());
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
        let s = tailor(&r, &asking(&["Rust"]), &[], &variant, &Theme::default());
        assert!(!s.bullets.contains("b-rust"));
        assert!(s.bullets.contains("b-pg"));
    }

    #[test]
    fn an_excluded_entry_takes_its_bullets_with_it() {
        let r = sample();
        let mut variant = Variant::default();
        variant.include.insert("e1".into(), false);
        let s = tailor(&r, &Target::none(), &[], &variant, &Theme::default());
        assert!(!s.entries.contains("e1"));
        assert!(s.bullets.is_empty());
    }

    /// A whole section switched off for one posting — six lines of Leadership
    /// that a research lab will not read.
    #[test]
    fn an_excluded_section_takes_its_entries_with_it() {
        let r = sample();
        let experience = r
            .sections
            .iter()
            .find(|s| s.kind == SectionKind::Experience)
            .unwrap();
        let mut variant = Variant::default();
        variant.include.insert(experience.id.clone(), false);
        let s = tailor(&r, &Target::none(), &[], &variant, &Theme::default());
        assert!(!s.entries.contains("e1"), "the section's entry survived");
        assert!(s.bullets.is_empty());
    }

    /// The skills block is built from the résumé rather than from the selected
    /// entries, so it needs the exclusion applied to it directly or a line the
    /// user switched off would print anyway.
    #[test]
    fn an_excluded_skills_line_leaves_the_block() {
        let mut r = sample();
        let skills_section = r
            .sections
            .iter_mut()
            .find(|s| s.kind == SectionKind::Skills)
            .unwrap();
        skills_section.entries.push(Entry {
            id: "s-lang".into(),
            title: "Languages".into(),
            detail: Some("Rust, TypeScript".into()),
            ..Entry::default()
        });
        skills_section.entries.push(Entry {
            id: "s-design".into(),
            title: "Design".into(),
            detail: Some("Figma".into()),
            ..Entry::default()
        });

        let mut variant = Variant::default();
        variant.include.insert("s-design".into(), false);
        let s = tailor(&r, &Target::none(), &[], &variant, &Theme::default());
        let labels: Vec<&str> = s.skills.iter().map(|(l, _)| l.as_str()).collect();
        assert_eq!(labels, vec!["Languages"], "the excluded line still printed");
    }

    #[test]
    fn a_short_resume_keeps_every_bullet() {
        let r = sample();
        let s = tailor(
            &r,
            &asking(&["Rust"]),
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
        let s = tailor(
            &r,
            &Target::none(),
            &[],
            &Variant::default(),
            &Theme::default(),
        );
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
            &asking(&["Rust"]),
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
        let s = tailor(&r, &Target::none(), &[], &variant, &Theme::default());
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
            &Target::none(),
            &[],
            &Variant::default(),
            &Theme {
                max_pages: 1,
                ..Theme::default()
            },
        );
        let three = tailor(
            &r,
            &Target::none(),
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
            &asking(&["React"]),
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
            &asking(&["TypeScript"]),
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
        let s = tailor(&r, &Target::none(), &[], &variant, &Theme::default());
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
        let s = tailor(&r, &Target::none(), &[], &variant, &Theme::default());
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
            let s = tailor(&r, &Target::none(), &[], &Variant::default(), &theme);
            assert_eq!(
                layout::lay_out(&r, &s, &theme).pages.len(),
                1,
                "{id:?} did not converge"
            );
        }
    }

    /// A posting read in full, the way `commands::target_of` builds one.
    fn posting(requirements: &str) -> Target {
        Target::new(
            &[],
            crate::resume::terms::Focus::read(&[(
                requirements,
                crate::resume::terms::REQUIREMENTS,
            )]),
        )
    }

    /// The half of the match the catalog cannot see. "On-call rotation" is not
    /// a skill in any catalog and is the whole subject of a great many
    /// infrastructure postings, and the bullet that speaks to it has to
    /// outlive the one that does not.
    #[test]
    fn a_bullet_that_speaks_the_postings_language_survives() {
        let filler = "Reorganised the annual charity bake sale and kept the spreadsheet tidy \
                      for everyone involved across the whole of the calendar year";
        let mut bullets = vec![bullet(
            "b-oncall",
            "Ran the on-call rotation and cut paging volume",
            &[],
        )];
        bullets.extend((0..80).map(|n| bullet(&format!("b{n}"), filler, &[])));
        let r = resume_with(bullets);

        let target = posting(
            "You have run an on-call rotation.\n             You have cut paging volume for a team that owns its own uptime.",
        );
        let s = tailor(&r, &target, &[], &Variant::default(), &Theme::default());
        assert!(
            s.bullets.contains("b-oncall"),
            "the bullet about the actual job was trimmed"
        );
        assert!(
            !s.echo["b-oncall"].is_empty(),
            "nothing was recorded about why it was kept"
        );
    }

    /// Echoing the posting is weaker evidence than naming what it asked for,
    /// and the UI has to be able to say which happened.
    #[test]
    fn why_and_echo_are_kept_apart() {
        let r = sample();
        let mut target = Target::new(
            &[skills::Demand {
                skill: "Rust".into(),
                weight: 3.0,
                emphasis: Emphasis::Must,
            }],
            crate::resume::terms::Focus::read(&[(
                "You will model schemas and own the migration",
                crate::resume::terms::REQUIREMENTS,
            )]),
        );
        target.must = vec!["Rust".into()];
        let s = tailor(&r, &target, &[], &Variant::default(), &Theme::default());

        assert_eq!(
            s.why.get("b-rust").map(Vec::as_slice),
            Some(&["Rust".to_string()][..])
        );
        assert!(
            !s.why.contains_key("b-pg"),
            "an echo was reported as coverage"
        );
        assert!(
            s.echo.contains_key("b-none"),
            "the migration bullet echoed nothing: {:?}",
            s.echo
        );
        assert_eq!(s.must, vec!["Rust".to_string()]);
        assert!(
            !s.focus.is_empty(),
            "the posting's own words were not reported"
        );
    }

    /// Projects have no chronology to defend, so the one that speaks to the
    /// posting leads. This is the difference between a tailored résumé and the
    /// same résumé with fewer lines.
    #[test]
    fn the_relevant_project_leads() {
        let mut r = sample();
        let projects = Section {
            id: "projects".into(),
            kind: SectionKind::Projects,
            title: "Projects".into(),
            entries: vec![
                Entry {
                    id: "p-game".into(),
                    title: "Tile-matching game".into(),
                    bullets: vec![bullet("pb-game", "Built a puzzle game in Unity", &[])],
                    ..Entry::default()
                },
                Entry {
                    id: "p-infra".into(),
                    title: "Deploy tooling".into(),
                    bullets: vec![bullet(
                        "pb-infra",
                        "Wrote the on-call rotation tooling and the paging runbook",
                        &[],
                    )],
                    ..Entry::default()
                },
            ],
        };
        r.sections.push(projects);

        let target = posting("You have run an on-call rotation and written a paging runbook.");
        let s = tailor(&r, &target, &[], &Variant::default(), &Theme::default());
        let section = r.sections.iter().find(|s| s.id == "projects").unwrap();
        let order: Vec<&str> = s.order_for(section).iter().map(|e| e.id.as_str()).collect();
        assert_eq!(
            order,
            vec!["p-infra", "p-game"],
            "the relevant project did not lead"
        );
    }

    /// Work history is the one thing tailoring must not rearrange.
    #[test]
    fn experience_keeps_the_order_it_was_written_in() {
        let mut r = sample();
        let experience = r
            .sections
            .iter_mut()
            .find(|s| s.kind == SectionKind::Experience)
            .unwrap();
        experience.id = "sec".into();
        experience.entries.push(Entry {
            id: "e2".into(),
            org: "Beta".into(),
            bullets: vec![bullet("b2", "Ran the on-call rotation", &[])],
            ..Entry::default()
        });

        let target = posting("You have run an on-call rotation.");
        let s = tailor(&r, &target, &[], &Variant::default(), &Theme::default());
        let section = r.sections.iter().find(|s| s.id == "sec").unwrap();
        let ids: Vec<&str> = s.order_for(section).iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, vec!["e1", "e2"], "a work history was reordered");
    }

    /// An explicit order is the user's decision and outranks the guess.
    #[test]
    fn a_variant_order_beats_the_relevance_guess() {
        let mut r = sample();
        r.sections.push(Section {
            id: "projects".into(),
            kind: SectionKind::Projects,
            title: "Projects".into(),
            entries: vec![
                Entry {
                    id: "p-game".into(),
                    bullets: vec![bullet("pb-game", "Built a puzzle game in Unity", &[])],
                    ..Entry::default()
                },
                Entry {
                    id: "p-infra".into(),
                    bullets: vec![bullet(
                        "pb-infra",
                        "Wrote the on-call rotation tooling",
                        &[],
                    )],
                    ..Entry::default()
                },
            ],
        });
        let mut variant = Variant::default();
        variant
            .order
            .insert("projects".into(), vec!["p-game".into(), "p-infra".into()]);

        let target = posting("You have run an on-call rotation.");
        let s = tailor(&r, &target, &[], &variant, &Theme::default());
        let section = r.sections.iter().find(|s| s.id == "projects").unwrap();
        let order: Vec<&str> = s.order_for(section).iter().map(|e| e.id.as_str()).collect();
        assert_eq!(order, vec!["p-game", "p-infra"]);
    }

    /// A posting we could not read must leave the document exactly as written.
    #[test]
    fn an_unread_posting_shuffles_nothing() {
        let mut r = sample();
        r.sections.push(Section {
            id: "projects".into(),
            kind: SectionKind::Projects,
            title: "Projects".into(),
            entries: vec![
                Entry {
                    id: "p-one".into(),
                    bullets: vec![bullet("pb-one", "Built a puzzle game", &[])],
                    ..Entry::default()
                },
                Entry {
                    id: "p-two".into(),
                    bullets: vec![bullet("pb-two", "Wrote deploy tooling", &[])],
                    ..Entry::default()
                },
            ],
        });
        let s = tailor(
            &r,
            &Target::none(),
            &[],
            &Variant::default(),
            &Theme::default(),
        );
        let section = r.sections.iter().find(|s| s.id == "projects").unwrap();
        let order: Vec<&str> = s.order_for(section).iter().map(|e| e.id.as_str()).collect();
        assert_eq!(order, vec!["p-one", "p-two"]);
        assert!(s.focus.is_empty());
    }

    /// How hard the posting asked has to reach the score, or "nice to have:
    /// Figma" is worth the same line as "5+ years of Rust".
    #[test]
    fn a_must_have_outscores_a_nice_to_have() {
        let target = Target::new(
            &[
                skills::Demand {
                    skill: "Rust".into(),
                    weight: 4.5,
                    emphasis: Emphasis::Must,
                },
                skills::Demand {
                    skill: "Figma".into(),
                    weight: 0.9,
                    emphasis: Emphasis::Nice,
                },
            ],
            crate::resume::terms::Focus::empty(),
        );
        let none = HashSet::new();
        let text = "Rebuilt the ingest path end to end";
        let must = target.relevance(&["Rust".to_string()], text, &none);
        let nice = target.relevance(&["Figma".to_string()], text, &none);
        assert!(must > nice, "must {must} did not outscore nice {nice}");
        assert_eq!(target.skills(), ["Rust".to_string(), "Figma".to_string()]);
    }

    /// A skill the user has declared is a claim they can defend in the room;
    /// one that only turned up in the bullet's own wording is a keyword match.
    #[test]
    fn a_declared_skill_counts_for_more() {
        let target = asking(&["Rust"]);
        let text = "Shipped a Rust service";
        let declared: HashSet<&str> = ["Rust"].into_iter().collect();
        assert!(
            target.relevance(&["Rust".to_string()], text, &declared)
                > target.relevance(&["Rust".to_string()], text, &HashSet::new())
        );
    }
}
