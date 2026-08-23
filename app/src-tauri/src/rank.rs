//! Discipline classification and feed ranking, ported from the Python
//! pipeline's categorization and prioritization layers.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::companies;
use crate::models::{Item, ItemType, ScoreTerm};
use crate::skills;

/// Keyword weights per academic discipline. The Python version shipped a single
/// entry; the structure is kept so more majors slot in without a rewrite.
pub const MAJORS: &[(&str, &[&str])] = &[(
    "Software Engineering",
    &[
        "coding",
        "software",
        "programming",
        "python",
        "java",
        "api",
        "web",
        "cloud",
        "devops",
        "intern",
        "engineer",
    ],
)];

pub const DEFAULT_MAJOR: &str = "Software Engineering";

/// Fine-grained interests the user picks in Settings. A major says *what you
/// study*; these say *what you actually want to read about*, and they are what
/// separates two internships that both classify as Software Engineering.
///
/// Keywords are matched as substrings against the lowercased title + body, so
/// they must be distinctive enough not to fire on unrelated text — this is why
/// the list avoids bare tokens like "ai" or "go" that appear inside ordinary
/// words. [`list_interests`] exposes the names to the UI so the picker never
/// hardcodes a copy of this list.
pub const INTERESTS: &[(&str, &[&str])] = &[
    (
        "AI/ML",
        &[
            "machine learning",
            "deep learning",
            "neural network",
            "llm",
            "language model",
            "pytorch",
            "tensorflow",
            "computer vision",
            "generative ai",
            " ai ",
            "ai/ml",
            "transformer",
            "inference",
        ],
    ),
    (
        "Frontend",
        &[
            "frontend",
            "front-end",
            "react",
            "vue.js",
            "svelte",
            "next.js",
            "tailwind",
            "css",
            "typescript",
            "web ui",
            "user interface",
            "browser",
        ],
    ),
    (
        "Backend",
        &[
            "backend",
            "back-end",
            "microservice",
            "rest api",
            "graphql",
            "postgres",
            "database",
            "server-side",
            "distributed system",
            "golang",
            "django",
            "node.js",
        ],
    ),
    (
        "Systems",
        &[
            "kernel",
            "compiler",
            "operating system",
            "embedded",
            "firmware",
            "low-level",
            "rust",
            "c++",
            "assembly",
            "linux",
            "memory safety",
            "concurrency",
        ],
    ),
    (
        "Security",
        &[
            "security",
            "vulnerability",
            "exploit",
            "cryptography",
            "penetration test",
            "malware",
            "infosec",
            "authentication",
            "zero-day",
            "cve-",
        ],
    ),
    (
        "Data",
        &[
            "data engineer",
            "data scien",
            "analytics",
            "apache spark",
            "etl",
            "data warehouse",
            "data pipeline",
            "sql",
            "bigquery",
            "snowflake",
        ],
    ),
    (
        "DevOps/Cloud",
        &[
            "devops",
            "kubernetes",
            "docker",
            "terraform",
            "aws",
            "azure",
            "gcp",
            "ci/cd",
            "site reliability",
            "observability",
            "infrastructure",
        ],
    ),
    (
        "Mobile",
        &[
            "ios",
            "android",
            "swift",
            "kotlin",
            "react native",
            "flutter",
            "mobile app",
            "app store",
        ],
    ),
    (
        "Hardware",
        &[
            "semiconductor",
            "fpga",
            "chip design",
            "verilog",
            "robotics",
            "circuit",
            "silicon",
            "processor",
            "gpu",
        ],
    ),
    (
        "Product/Design",
        &[
            "product manager",
            "product management",
            "ux ",
            "user experience",
            "product design",
            "figma",
            "design system",
        ],
    ),
    (
        "Game Dev",
        &[
            "game dev",
            "unity",
            "unreal engine",
            "shader",
            "rendering",
            "graphics engine",
            "gamedev",
        ],
    ),
    (
        "Startups",
        &[
            "startup",
            "founder",
            "seed round",
            "series a",
            "y combinator",
            "venture capital",
            "bootstrapped",
        ],
    ),
];

/// The interest names, in declaration order, for the Settings picker.
pub fn list_interests() -> Vec<String> {
    INTERESTS.iter().map(|(name, _)| name.to_string()).collect()
}

/// Counts keyword hits across title + body and returns the best-scoring major,
/// or "General" when nothing matches.
pub fn classify_item(item: &Item) -> String {
    let text = format!("{} {}", item.title, item.content_text).to_lowercase();

    let mut best: Option<(&str, usize)> = None;
    for (major, keywords) in MAJORS {
        let score = keywords.iter().filter(|kw| text.contains(**kw)).count();
        if best.is_none_or(|(_, b)| score > b) {
            best = Some((major, score));
        }
    }

    match best {
        Some((major, score)) if score > 0 => major.to_string(),
        _ => "General".to_string(),
    }
}

/// Which of the user's chosen interests this item matches. Unknown names in
/// `wanted` (a stale setting after an interest is renamed) are simply ignored.
pub fn match_interests(item: &Item, wanted: &[String]) -> Vec<String> {
    if wanted.is_empty() {
        return Vec::new();
    }
    // Pad with spaces so keywords written with leading/trailing spaces (" ai ")
    // can match at the very start or end of the text.
    let text = format!(" {} {} ", item.title, item.content_text).to_lowercase();

    INTERESTS
        .iter()
        .filter(|(name, keywords)| {
            wanted.iter().any(|w| w == name) && keywords.iter().any(|kw| text.contains(kw))
        })
        .map(|(name, _)| name.to_string())
        .collect()
}

/// How many items one source may contribute per round-robin pass. At 1, the
/// first N items of the feed come from N different sources.
const PER_SOURCE_PER_ROUND: usize = 1;

/// A posting naming one lucky skill is not evidence of fit. Below this the
/// skill term is skipped entirely rather than paying out a full-coverage bonus.
const MIN_SKILLS_TO_SCORE: usize = 2;
/// Hours for the recency term to halve: two days old is worth half, four days a
/// quarter. Tuned so a fresh article can outrank a stale internship, but a fresh
/// internship still beats a fresh article.
const RECENCY_HALF_LIFE_HOURS: f64 = 48.0;

/// Pay at or below this scores nothing; at or above the ceiling it scores the
/// full term. A band rather than a raw amount, so one $400k posting cannot
/// flatten the difference between everything else.
const PAY_FLOOR: f64 = 60_000.0;
const PAY_CEILING: f64 = 200_000.0;

/// Where the urgency term peaks. Sooner than this and there may not be time to
/// write an application; later and it is not yet urgent.
const URGENCY_PEAK_DAYS: f64 = 6.0;
/// How fast urgency falls away either side of the peak.
const URGENCY_SPREAD_DAYS: f64 = 10.0;
/// What a closed posting scores. Large enough to sink it under everything
/// else without being infinite, so "show expired" still lists them in a
/// sensible order rather than an arbitrary one.
const CLOSED_PENALTY: f64 = 20.0;

/// What each scoring term is worth. Shipped defaults, overridable from
/// Settings and persisted as JSON in `settings`, because a ranking the reader
/// cannot steer is indistinguishable from an arbitrary one.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Weights {
    /// Points for an item whose discipline equals the user's major.
    ///
    /// Deliberately no longer the largest term. `MAJORS` has one entry, so
    /// this is a binary "is this tech-related" flag, and it used to outweigh
    /// every signal that actually distinguishes two tech postings.
    pub major: f64,
    /// Points per matched interest tag, and the ceiling on their sum. The cap
    /// stops a keyword-stuffed description outranking everything on breadth.
    pub interest: f64,
    pub interest_cap: f64,
    /// Full value of the skill term, awarded when the user has every skill the
    /// posting names. Coverage-proportional rather than count-proportional, so
    /// a posting asking ten things you can all do beats one asking two.
    pub skill: f64,
    /// Full value of the recency term, awarded to something happening now.
    pub recency: f64,
    /// Full value of the prestige term, at tier 1.
    pub prestige: f64,
    /// Full value of the pay term, at or above [`PAY_CEILING`].
    pub pay: f64,
    /// Full value of the deadline term, at the urgency peak.
    pub urgency: f64,
    /// Full value of the seniority term, for a role at the user's level.
    pub seniority: f64,
    /// Full value of the location term, for a role in the user's home region.
    pub location: f64,
    /// Subtracted from an item the user has already opened, so the feed keeps
    /// moving between refreshes.
    pub seen_penalty: f64,
}

impl Default for Weights {
    fn default() -> Self {
        Weights {
            major: 4.0,
            interest: 4.0,
            interest_cap: 8.0,
            skill: 6.0,
            recency: 6.0,
            prestige: 8.0,
            pay: 4.0,
            urgency: 5.0,
            seniority: 6.0,
            location: 3.0,
            seen_penalty: 3.0,
        }
    }
}

/// Everything the scorer needs to know about the reader. Grouped because the
/// term count has outgrown a positional argument list, and because every
/// caller reads the whole set out of `settings` together anyway.
#[derive(Clone, Debug, Default)]
pub struct Profile {
    pub major: String,
    pub interests: Vec<String>,
    pub skills: Vec<String>,
    pub weights: Weights,
    /// Per-company tier overrides, keyed by canonical display name.
    pub company_tiers: HashMap<String, u8>,
    /// A `location_tags` value — "Toronto", "Canada" — that a posting earns
    /// the location term for matching. `None` disables the term.
    pub home_region: Option<String>,
    /// Which seniority bands the reader is looking for. Empty means "no
    /// preference", which scores every level the same rather than nothing.
    pub target_seniority: Vec<String>,
}

/// The seniority bands, weakest signal last. Order matters: a title matching
/// several ("Senior Software Engineer Intern" does exist) resolves to the
/// first, and intern beats senior because the intern word is the specific one.
const SENIORITY: &[(&str, &[&str])] = &[
    ("Intern", &["intern", "internship", "co-op", "coop", " co op ", "student", "placement", "apprentice"]),
    ("New grad", &["new grad", "new graduate", "recent graduate", "campus", "early career", "early talent", "graduate program", "rotational program", "entry level", "entry-level"]),
    ("Junior", &["junior", " jr ", " jr.", " i ", " associate "]),
    ("Lead", &["director", "vp ", "vice president", "head of", "chief", "principal", "distinguished", "fellow"]),
    ("Senior", &["senior", " sr ", " sr.", "staff ", "lead ", "manager", "architect", " iii", " iv"]),
];

/// Reads the level off a job title. `None` where the title says nothing —
/// which is most of them, and is a different answer from "mid-level".
pub fn classify_seniority(title: &str) -> Option<String> {
    let text = format!(" {} ", title.to_lowercase());
    SENIORITY
        .iter()
        .find(|(_, keywords)| keywords.iter().any(|kw| text.contains(kw)))
        .map(|(name, _)| name.to_string())
}

/// How well a posting's level matches what the reader wants, in -1.0..=1.0.
///
/// A student searching for a co-op term does not want a Director posting in
/// the feed at all, and the old scorer gave the two identical scores. With no
/// stated preference every level scores neutral rather than zero, so the term
/// simply does not participate.
fn seniority_fit(item: &Item, wanted: &[String]) -> f64 {
    let Some(level) = item.seniority.as_deref() else {
        return 0.0;
    };
    if wanted.is_empty() {
        return 0.0;
    }
    if wanted.iter().any(|w| w == level) {
        return 1.0;
    }
    // Adjacent levels are a near-miss, not a wrong answer: a new-grad role is
    // worth showing someone looking for internships.
    let adjacent = match level {
        "Intern" => ["New grad"].as_slice(),
        "New grad" => ["Intern", "Junior"].as_slice(),
        "Junior" => ["New grad"].as_slice(),
        "Senior" => ["Lead"].as_slice(),
        "Lead" => ["Senior"].as_slice(),
        _ => [].as_slice(),
    };
    if adjacent.iter().any(|a| wanted.iter().any(|w| w == a)) {
        return 0.25;
    }
    -1.0
}

/// Exponential decay on distance from now, in *either* direction. A week-old
/// article and an event a week out are equally far from "right now" and should
/// both rank below today's — without the absolute value, Luma events dated
/// months ahead would take over the feed.
pub fn recency_score(timestamp: DateTime<Utc>, now: DateTime<Utc>) -> f64 {
    recency_score_weighted(timestamp, now, Weights::default().recency)
}

fn recency_score_weighted(timestamp: DateTime<Utc>, now: DateTime<Utc>, weight: f64) -> f64 {
    let hours = (now - timestamp).num_minutes().abs() as f64 / 60.0;
    weight * 0.5_f64.powf(hours / RECENCY_HALF_LIFE_HOURS)
}

/// The deadline term, in -1.0..=1.0 before weighting.
///
/// Something already closed is worth less than something with no deadline at
/// all, which is the whole point of tracking the date. Between those, urgency
/// rises as the deadline approaches and falls again once it is so close that
/// there is no time to apply.
pub fn urgency_score(closes_at: Option<DateTime<Utc>>, now: DateTime<Utc>) -> f64 {
    let Some(closes) = closes_at else {
        return 0.0;
    };
    let days = (closes - now).num_minutes() as f64 / (60.0 * 24.0);
    if days < 0.0 {
        return -1.0;
    }
    let distance = (days - URGENCY_PEAK_DAYS) / URGENCY_SPREAD_DAYS;
    (-distance * distance).exp()
}

/// The pay term, in 0.0..=1.0. Undisclosed pay contributes nothing and is
/// never a penalty: most employers publish nothing, and ranking them below
/// the ones that do would hide most of the market.
pub fn pay_score(item: &Item) -> f64 {
    let pay = crate::pay::Pay {
        min: item.salary_min,
        max: item.salary_max,
        currency: item.salary_currency.clone(),
        period: item.salary_period.clone(),
    };
    let Some(annual) = crate::pay::annual_equivalent(&pay) else {
        return 0.0;
    };
    ((annual - PAY_FLOOR) / (PAY_CEILING - PAY_FLOOR)).clamp(0.0, 1.0)
}

/// Scores every item against the reader's profile, sorts by score, then
/// applies the per-source diversification pass. Also records which interests
/// fired, which skills the posting wants, and what every term contributed, so
/// the UI can explain the ranking rather than asserting it.
pub fn personalize(items: Vec<Item>, profile: &Profile) -> Vec<Item> {
    personalize_at(items, profile, Utc::now())
}

/// [`personalize`] with an injectable clock, so the time-dependent terms are
/// testable.
pub fn personalize_at(mut items: Vec<Item>, profile: &Profile, now: DateTime<Utc>) -> Vec<Item> {
    let w = &profile.weights;

    for item in &mut items {
        let mut score = 0.0;
        let mut breakdown: Vec<ScoreTerm> = Vec::new();
        let add = |label: &str, points: f64, score: &mut f64, breakdown: &mut Vec<ScoreTerm>| {
            if points.abs() > f64::EPSILON {
                *score += points;
                breakdown.push(ScoreTerm::new(label, points));
            }
        };

        // 1. Major match
        if item.discipline.as_deref() == Some(profile.major.as_str()) {
            add("Your field", w.major, &mut score, &mut breakdown);
        }

        // 2. Item type weight
        if item.item_type.is_opportunity() {
            add("Opportunity", 5.0, &mut score, &mut breakdown);
        } else if item.item_type == ItemType::Event {
            add("Event", 4.0, &mut score, &mut breakdown);
        }

        // 3. Interest match
        let matched = match_interests(item, &profile.interests);
        if !matched.is_empty() {
            let points = (matched.len() as f64 * w.interest).min(w.interest_cap);
            add(
                &format!("Interests: {}", matched.join(", ")),
                points,
                &mut score,
                &mut breakdown,
            );
        }
        item.matched_interests = matched;

        // 4. Skill coverage. Opportunities only: extraction is the expensive
        // part of scoring, and "skills this posting asks for" is meaningless
        // for an article that merely mentions a technology.
        if item.item_type.is_opportunity() {
            let required = skills::extract(item);
            let matched: Vec<String> = required
                .iter()
                .filter(|s| profile.skills.iter().any(|u| u == *s))
                .cloned()
                .collect();

            if required.len() >= MIN_SKILLS_TO_SCORE {
                let coverage = matched.len() as f64 / required.len() as f64;
                add(
                    &format!("Skills you have ({}/{})", matched.len(), required.len()),
                    w.skill * coverage,
                    &mut score,
                    &mut breakdown,
                );
            }

            item.matched_skills = matched;
            item.required_skills = required;
        }

        // 5. Recency
        add(
            "Recent",
            recency_score_weighted(item.timestamp, now, w.recency),
            &mut score,
            &mut breakdown,
        );

        // 6. Company prestige. Derived on read rather than stored, so a tier
        // the user overrides in Settings takes effect without a refresh.
        if let Some(company) = item.company.as_deref() {
            let tier = companies::tier(company, &profile.company_tiers);
            item.company_tier = tier;
            add(
                &format!("{company} (tier {})", tier.map(|t| t.to_string()).unwrap_or_else(|| "—".into())),
                w.prestige * companies::tier_value(tier),
                &mut score,
                &mut breakdown,
            );
        }

        // 7. Published pay
        if item.item_type.is_opportunity() {
            add("Pay", w.pay * pay_score(item), &mut score, &mut breakdown);
        }

        // 8. Deadline. Closed postings are floored rather than dropped: the
        // reader can still ask to see them, and when they do the order should
        // mean something.
        if item.item_type.is_opportunity() {
            let urgency = urgency_score(item.closes_at, now);
            if urgency < 0.0 {
                add("Closed", -CLOSED_PENALTY, &mut score, &mut breakdown);
            } else {
                add("Closing soon", w.urgency * urgency, &mut score, &mut breakdown);
            }
        }

        // 9. Seniority fit
        if item.item_type.is_opportunity() {
            let fit = seniority_fit(item, &profile.target_seniority);
            let label = match item.seniority.as_deref() {
                Some(level) if fit >= 0.0 => format!("{level} role"),
                Some(level) => format!("{level} role — not your level"),
                None => "Level".to_string(),
            };
            add(&label, w.seniority * fit, &mut score, &mut breakdown);
        }

        // 10. Location fit
        if let Some(home) = profile.home_region.as_deref() {
            if item.location_tags.iter().any(|t| t == home) {
                add(&format!("In {home}"), w.location, &mut score, &mut breakdown);
            }
        }

        // 11. Already-read items sink, so the feed keeps moving.
        if item.seen {
            add("Already seen", -w.seen_penalty, &mut score, &mut breakdown);
        }

        breakdown.sort_by(|a, b| b.points.total_cmp(&a.points));
        item.score_breakdown = breakdown;
        item.relevance_score = Some(score);
    }

    // Stable sort, matching Python's `sorted`: ties keep scraper order.
    items.sort_by(|a, b| {
        b.relevance_score
            .unwrap_or(0.0)
            .total_cmp(&a.relevance_score.unwrap_or(0.0))
    });

    diversify(items)
}

/// Interleaves the scored items so consecutive entries come from different
/// sources.
///
/// Score order alone is not a usable feed here. The three GitHub internship
/// repos contribute ~450 of ~700 items and every one of them scores identically
/// (major match + opportunity), so a straight sort buries news and events
/// hundreds of rows down — the exact FOMO this app exists to prevent.
///
/// Each pass takes the best remaining item from every source in turn, with
/// sources ordered by the score of the item they are offering. Quality still
/// decides position within a pass; breadth is guaranteed across it.
fn diversify(items: Vec<Item>) -> Vec<Item> {
    // Preserves score order inside each bucket, and first-seen order across
    // them, so the result is deterministic.
    let mut order: Vec<String> = Vec::new();
    let mut buckets: std::collections::HashMap<String, std::collections::VecDeque<Item>> =
        std::collections::HashMap::new();

    for item in items {
        // Bucket on the employer where there is one. One Workday tenant can
        // publish two thousand postings; if every direct board shared the
        // bucket "Workday" they would get a single slot per round against
        // twenty news feeds, and the roles this app exists for would sit
        // below the news. `source_platform` stays the ATS family so the
        // Sources health list still reports five sources rather than a
        // hundred.
        let source = item
            .company
            .clone()
            .unwrap_or_else(|| item.source_platform.clone());
        if !buckets.contains_key(&source) {
            order.push(source.clone());
        }
        buckets.entry(source).or_default().push_back(item);
    }

    let mut out = Vec::new();
    loop {
        // Sources still holding items, best offer first.
        let mut round: Vec<&String> = order
            .iter()
            .filter(|s| buckets.get(*s).is_some_and(|b| !b.is_empty()))
            .collect();
        if round.is_empty() {
            break;
        }
        round.sort_by(|a, b| {
            let score = |s: &String| {
                buckets[s]
                    .front()
                    .and_then(|i| i.relevance_score)
                    .unwrap_or(0.0)
            };
            score(b).total_cmp(&score(a))
        });

        for source in round {
            let bucket = buckets
                .get_mut(source)
                .expect("source in round has a bucket");
            for _ in 0..PER_SOURCE_PER_ROUND {
                match bucket.pop_front() {
                    Some(item) => out.push(item),
                    None => break,
                }
            }
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(title: &str, source: &str, ty: ItemType) -> Item {
        Item::new(title, source, ty, format!("https://example.com/{title}"))
    }

    /// The shipped weights, so a test asserts against the same numbers the
    /// app ships with rather than a copy that can drift from them.
    fn w() -> Weights {
        Weights::default()
    }

    /// A reader with nothing set: no interests, no skills, no home region and
    /// no level preference, so only the terms under test fire.
    fn profile(major: &str, interests: &[String], skills: &[String]) -> Profile {
        Profile {
            major: major.to_string(),
            interests: interests.to_vec(),
            skills: skills.to_vec(),
            ..Profile::default()
        }
    }

    /// Pins every item to `now` so tests that predate the recency term still
    /// see the flat scores they were written against.
    fn rank(items: Vec<Item>, major: &str) -> Vec<Item> {
        let now = Utc::now();
        let items = items.into_iter().map(|i| i.with_timestamp(now)).collect();
        personalize_at(items, &profile(major, &[], &[]), now)
    }

    #[test]
    fn classifies_on_keyword_hits() {
        let mut it = item("Software Engineer Intern", "Test", ItemType::Internship);
        it.content_text = "Python and cloud work".into();
        assert_eq!(classify_item(&it), "Software Engineering");
    }

    #[test]
    fn falls_back_to_general() {
        let it = item("Poetry reading night", "Test", ItemType::Event);
        assert_eq!(classify_item(&it), "General");
    }

    #[test]
    fn scores_major_match_and_item_type() {
        let mut job = item("A", "S", ItemType::Internship);
        job.discipline = Some("Software Engineering".into());
        let mut event = item("B", "S", ItemType::Event);
        event.discipline = Some("Software Engineering".into());
        let article = item("C", "S", ItemType::Article);

        let ranked = rank(vec![article, event, job], "Software Engineering");
        // Every item is timestamped `now`, so each carries the full recency weight.
        assert_eq!(ranked[0].relevance_score, Some(w().major + 5.0 + w().recency)); // field + opportunity
        assert_eq!(ranked[1].relevance_score, Some(w().major + 4.0 + w().recency)); // field + event
        assert_eq!(ranked[2].relevance_score, Some(w().recency)); // no discipline match
    }

    #[test]
    fn a_flooding_source_cannot_take_two_slots_in_a_row() {
        // 20 high-scoring items from one source, plus one low-scoring item from
        // another. Without diversification the lonely item lands at position 20.
        let mut items: Vec<Item> = (0..20)
            .map(|i| {
                let mut it = item(&format!("flood-{i}"), "Flooder", ItemType::Internship);
                it.discipline = Some("Software Engineering".into());
                it
            })
            .collect();
        items.push(item("lonely", "Other", ItemType::Article));

        let ranked = rank(items, "Software Engineering");

        assert_eq!(ranked.len(), 21);
        // Round one: the flooder's best, then the only other source.
        assert_eq!(ranked[0].source_platform, "Flooder");
        assert_eq!(ranked[1].source_platform, "Other");
        // With nothing left to interleave, the rest drains in score order.
        assert!(ranked[2..].iter().all(|i| i.source_platform == "Flooder"));
    }

    #[test]
    fn the_first_page_spans_every_source() {
        // The real failure this guards: three internship repos contributing
        // hundreds of identically-scored rows, burying news and events.
        let mut items: Vec<Item> = Vec::new();
        for repo in ["Pitt CSC Repo", "Simplify", "New Grad Positions"] {
            for i in 0..100 {
                let mut it = item(&format!("{repo}-{i}"), repo, ItemType::Internship);
                it.discipline = Some("Software Engineering".into());
                items.push(it);
            }
        }
        items.push(item("a story", "Hacker News", ItemType::Article));
        items.push(item("a meetup", "Luma", ItemType::Event));

        let ranked = rank(items, "Software Engineering");
        let head: std::collections::HashSet<&str> = ranked
            .iter()
            .take(5)
            .map(|i| i.source_platform.as_str())
            .collect();

        assert_eq!(head.len(), 5, "top 5 should come from 5 distinct sources");
        assert!(head.contains("Hacker News"));
        assert!(head.contains("Luma"));
    }

    #[test]
    fn diversify_preserves_every_item() {
        let items: Vec<Item> = (0..50)
            .map(|i| {
                item(
                    &format!("i-{i}"),
                    &format!("src-{}", i % 7),
                    ItemType::Article,
                )
            })
            .collect();
        let ranked = rank(items, DEFAULT_MAJOR);
        assert_eq!(ranked.len(), 50);
        assert_eq!(
            ranked
                .iter()
                .map(|i| i.title.clone())
                .collect::<std::collections::HashSet<_>>()
                .len(),
            50
        );
    }

    #[test]
    fn recency_halves_every_half_life() {
        let now = Utc::now();
        let fresh = recency_score(now, now);
        let two_days = recency_score(now - chrono::Duration::hours(48), now);
        let four_days = recency_score(now - chrono::Duration::hours(96), now);

        assert!((fresh - w().recency).abs() < 1e-9);
        assert!((two_days - w().recency / 2.0).abs() < 1e-6);
        assert!((four_days - w().recency / 4.0).abs() < 1e-6);
    }

    #[test]
    fn future_events_decay_like_past_ones() {
        // A Luma event three months out must not outrank tonight's meetup.
        let now = Utc::now();
        let tonight = recency_score(now + chrono::Duration::hours(6), now);
        let far_off = recency_score(now + chrono::Duration::days(90), now);
        assert!(tonight > far_off);
        assert!(far_off < 0.01);
    }

    #[test]
    fn fresher_item_outranks_stale_one_of_equal_kind() {
        let now = Utc::now();
        let stale =
            item("old", "S", ItemType::Article).with_timestamp(now - chrono::Duration::days(10));
        let fresh = item("new", "S", ItemType::Article).with_timestamp(now);

        let ranked = personalize_at(vec![stale, fresh], &profile(DEFAULT_MAJOR, &[], &[]), now);
        assert_eq!(ranked[0].title, "new");
    }

    #[test]
    fn interests_add_score_and_are_recorded() {
        let now = Utc::now();
        let mut it = item("Research Intern", "S", ItemType::Internship).with_timestamp(now);
        it.content_text = "Work on PyTorch and computer vision for our LLM team".into();

        let ranked = personalize_at(vec![it], &profile("General", &["AI/ML".to_string()], &[]), now);
        assert_eq!(ranked[0].matched_interests, vec!["AI/ML"]);
        // opportunity (5) + one interest (4) + full recency (6)
        assert_eq!(ranked[0].relevance_score, Some(5.0 + 4.0 + w().recency));
    }

    #[test]
    fn interest_bonus_is_capped() {
        let now = Utc::now();
        let mut it = item("Everything Engineer", "S", ItemType::Article).with_timestamp(now);
        it.content_text =
            "kubernetes terraform react svelte pytorch machine learning cryptography exploit sql etl"
                .into();

        let wanted: Vec<String> = ["AI/ML", "Frontend", "Security", "Data", "DevOps/Cloud"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let ranked = personalize_at(vec![it], &profile("General", &wanted, &[]), now);

        assert!(ranked[0].matched_interests.len() >= 4);
        // Five matches would be 20 points uncapped; the cap holds it to 8.
        assert_eq!(
            ranked[0].relevance_score,
            Some(w().interest_cap + w().recency)
        );
    }

    #[test]
    fn unselected_interests_never_match() {
        let mut it = item("Kubernetes at scale", "S", ItemType::Article);
        it.content_text = "terraform and docker".into();
        assert!(match_interests(&it, &[]).is_empty());
        assert!(match_interests(&it, &["Frontend".to_string()]).is_empty());
    }

    #[test]
    fn skill_coverage_scales_the_bonus() {
        let now = Utc::now();
        // A title with no recognisable role, so the only skills in play are
        // the four the body names and the coverage maths is readable.
        let mut it = item("Intern at Foo", "S", ItemType::Internship).with_timestamp(now);
        it.content_text = "Python, Django and PostgreSQL on AWS.".into();

        let have_all: Vec<String> = ["Python", "Django", "PostgreSQL", "AWS"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let full = personalize_at(vec![it.clone()], &profile("General", &[], &have_all), now);
        let half = personalize_at(
            vec![it.clone()],
            &profile("General", &[], &["Python".to_string(), "Django".to_string()]),
            now,
        );
        let none = personalize_at(vec![it], &profile("General", &[], &[]), now);

        // opportunity (5) + coverage × 6 + full recency
        let base = 5.0 + w().recency;
        assert_eq!(full[0].relevance_score, Some(base + w().skill));
        assert_eq!(half[0].relevance_score, Some(base + w().skill / 2.0));
        assert_eq!(none[0].relevance_score, Some(base));

        assert_eq!(full[0].matched_skills.len(), 4);
        assert!(full[0].required_skills.iter().any(|s| s == "PostgreSQL"));
        assert!(none[0].matched_skills.is_empty());
        // Required is recorded regardless of what the user has.
        assert!(!none[0].required_skills.is_empty());
    }

    #[test]
    fn a_posting_naming_one_skill_earns_nothing() {
        let now = Utc::now();
        let mut it = item("Ops Intern", "S", ItemType::Internship).with_timestamp(now);
        it.content_text = "Some familiarity with Terraform.".into();

        let ranked = personalize_at(vec![it], &profile("General", &[], &["Terraform".to_string()]), now);
        assert_eq!(ranked[0].required_skills, vec!["Terraform"]);
        assert_eq!(ranked[0].matched_skills, vec!["Terraform"]);
        // Below MIN_SKILLS_TO_SCORE, so the term is skipped entirely.
        assert_eq!(ranked[0].relevance_score, Some(5.0 + w().recency));
    }

    #[test]
    fn articles_carry_no_skills() {
        let now = Utc::now();
        let mut it = item("Kubernetes at scale", "S", ItemType::Article).with_timestamp(now);
        it.content_text = "Docker, Terraform and Python throughout.".into();

        let ranked = personalize_at(vec![it], &profile("General", &[], &["Docker".to_string()]), now);
        assert!(ranked[0].required_skills.is_empty());
        assert!(ranked[0].matched_skills.is_empty());
        assert_eq!(ranked[0].relevance_score, Some(w().recency));
    }

    #[test]
    fn seen_items_sink_below_unseen_equivalents() {
        let now = Utc::now();
        let mut read = item("read", "S", ItemType::Article).with_timestamp(now);
        read.seen = true;
        let unread = item("unread", "S", ItemType::Article).with_timestamp(now);

        let ranked = personalize_at(vec![read, unread], &profile(DEFAULT_MAJOR, &[], &[]), now);
        assert_eq!(ranked[0].title, "unread");
    }
}
