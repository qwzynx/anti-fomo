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
    // can match at the very start or end of the text. Lowercased in place and
    // in ASCII: every interest keyword is ASCII, so this is exactly as precise
    // as the Unicode fold and does not allocate a second copy of the body.
    let mut text = String::with_capacity(item.title.len() + item.content_text.len() + 3);
    text.push(' ');
    text.push_str(&item.title);
    text.push(' ');
    text.push_str(&item.content_text);
    text.push(' ');
    text.make_ascii_lowercase();

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

/// A deadline this close is at full urgency, and stays there until it passes.
/// The term used to *fall* inside six days on the theory that there might not
/// be time to apply, which ranked the posting you will lose tomorrow below the
/// one you can still lose next week. Whether there is time is the reader's call;
/// the feed's job is to say it is about to close.
const URGENCY_FULL_DAYS: f64 = 3.0;
/// Beyond [`URGENCY_FULL_DAYS`], urgency halves every this many days.
const URGENCY_HALF_LIFE_DAYS: f64 = 7.0;
/// Past this a deadline is not news. A hard edge on a curve that is already
/// under two per cent here, so nothing jumps when it is crossed.
const URGENCY_HORIZON_DAYS: f64 = 45.0;
/// A good posting closing within this many days is a "last call". Wide enough
/// to act on, narrow enough that the band stays short.
pub const LAST_CALL_DAYS: f64 = 7.0;
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
    /// Full value of the deadline term: a posting at the top of the quality
    /// ramp ([`Bar`]) with a deadline inside [`URGENCY_FULL_DAYS`].
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
            urgency: 8.0,
            seniority: 6.0,
            location: 3.0,
            seen_penalty: 3.0,
        }
    }
}

/// Everything the scorer needs to know about the reader. Grouped because the
/// term count has outgrown a positional argument list, and because every
/// caller reads the whole set out of `settings` together anyway.
#[derive(Clone, Debug, Default, PartialEq)]
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
    (
        "Intern",
        &[
            "intern",
            "internship",
            "co-op",
            "coop",
            " co op ",
            "student",
            "placement",
            "apprentice",
        ],
    ),
    (
        "New grad",
        &[
            "new grad",
            "new graduate",
            "recent graduate",
            "campus",
            "early career",
            "early talent",
            "graduate program",
            "rotational program",
            "entry level",
            "entry-level",
        ],
    ),
    ("Junior", &["junior", " jr ", " jr.", " i ", " associate "]),
    (
        "Lead",
        &[
            "director",
            "vp ",
            "vice president",
            "head of",
            "chief",
            "principal",
            "distinguished",
            "fellow",
        ],
    ),
    (
        "Senior",
        &[
            "senior",
            " sr ",
            " sr.",
            "staff ",
            "lead ",
            "manager",
            "architect",
            " iii",
            " iv",
        ],
    ),
];

/// Reads the level off a job title. `None` where the title says nothing —
/// which is most of them, and is a different answer from "mid-level".
/// The seniority bands in the order a reader would list them, for the setup
/// form. [`SENIORITY`] is ordered by *match precedence* — a title naming
/// several resolves to the first — which is the wrong order to show a person,
/// so the display order is stated here and pinned against the table by a test.
pub fn seniority_levels() -> Vec<&'static str> {
    vec!["Intern", "New grad", "Junior", "Senior", "Lead"]
}

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

/// How pressing a deadline is, in -1.0..=1.0 before weighting and before
/// quality is taken into account.
///
/// Something already closed is worth less than something with no deadline at
/// all, which is the whole point of tracking the date. Otherwise urgency only
/// ever rises as the date nears: full strength for the last
/// [`URGENCY_FULL_DAYS`], then halving every [`URGENCY_HALF_LIFE_DAYS`] going
/// the other way, and nothing past [`URGENCY_HORIZON_DAYS`].
pub fn urgency_score(closes_at: Option<DateTime<Utc>>, now: DateTime<Utc>) -> f64 {
    let Some(closes) = closes_at else {
        return 0.0;
    };
    let days = (closes - now).num_minutes() as f64 / (60.0 * 24.0);
    if days < 0.0 {
        return -1.0;
    }
    if days <= URGENCY_FULL_DAYS {
        return 1.0;
    }
    if days > URGENCY_HORIZON_DAYS {
        return 0.0;
    }
    0.5_f64.powf((days - URGENCY_FULL_DAYS) / URGENCY_HALF_LIFE_DAYS)
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

/// Where "really good" starts, for one kind of item.
///
/// Quality is judged against the rest of what is being ranked rather than on
/// an absolute scale, because the scale moves: a reader who has told the app
/// nothing sees a flat field where three thousand postings tie, and one who has
/// picked interests and a level sees real spread. Below `lo` a deadline earns
/// nothing; at `hi` and above it earns all of it; between is a straight ramp.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bar {
    pub lo: f64,
    pub hi: f64,
}

impl Bar {
    fn new(lo: f64, hi: f64) -> Bar {
        // A ramp with no width would divide by zero, and a field where every
        // item ties is exactly when percentiles collapse onto one number.
        Bar {
            lo,
            hi: hi.max(lo + MIN_RAMP),
        }
    }

    /// How far up the ramp a merit score is, 0.0..=1.0.
    pub fn gate(&self, merit: f64) -> f64 {
        ((merit - self.lo) / (self.hi - self.lo)).clamp(0.0, 1.0)
    }

    /// Whether a merit score is good enough to call a closing posting a last call.
    pub fn clears(&self, merit: f64) -> bool {
        merit >= self.hi
    }
}

/// The bars for opportunities and for events, measured together because the
/// two score on different scales (an event never earns skills, pay or a tier).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bars {
    pub opportunity: Bar,
    pub event: Bar,
}

/// Floors under the measured bars. Without them an empty profile — where the
/// median is simply whatever the baseline terms add up to — would call every
/// posting good, and a feed of mostly-events ties at 4.0 and would too.
///
/// The event floor is high on purpose. An event scores 4 for being one and 4 per
/// matched interest (to 8), so a single interest takes it to 8 — and a hackathon
/// that mentions "AI" is the norm, not a standout. Measured on the live feed, a
/// floor of 7 flagged dozens of hackathons as last calls and, sorted by date,
/// crowded every role out of the band. At 12 it takes two interests, or one
/// plus the reader's field, for an event to count as really good.
const OPPORTUNITY_FLOOR: (f64, f64) = (9.0, 13.0);
const EVENT_FLOOR: (f64, f64) = (6.0, 12.0);
/// The narrowest ramp [`Bar::new`] will build.
const MIN_RAMP: f64 = 1.0;

impl Bars {
    /// Measures the bars from items that have already been through pass one,
    /// so each carries its `merit`: the median is where the ramp starts and the
    /// top decile is where it tops out and a deadline counts as a last call.
    ///
    /// The decile rather than the quartile was settled on measurement. With a
    /// reader who has answered the setup form, a quarter of the roles clear the
    /// upper-quartile bar and deadlines skew towards exactly the student
    /// postings such a reader wants — 26 of the 44 closing within a week were
    /// "last calls", which is a list, not a call. The top tenth leaves the band
    /// short enough to act on.
    pub fn measure(items: &[Item]) -> Bars {
        let mut opportunities = Vec::new();
        let mut events = Vec::new();
        for item in items {
            let Some(merit) = item.merit else { continue };
            if item.item_type.is_opportunity() {
                opportunities.push(merit);
            } else if item.item_type == ItemType::Event {
                events.push(merit);
            }
        }
        Bars {
            opportunity: measured_bar(opportunities, OPPORTUNITY_FLOOR),
            event: measured_bar(events, EVENT_FLOOR),
        }
    }

    fn for_type(&self, item_type: ItemType) -> &Bar {
        if item_type == ItemType::Event {
            &self.event
        } else {
            &self.opportunity
        }
    }
}

fn measured_bar(mut merits: Vec<f64>, (floor_lo, floor_hi): (f64, f64)) -> Bar {
    if merits.is_empty() {
        return Bar::new(floor_lo, floor_hi);
    }
    merits.sort_by(f64::total_cmp);
    let at = |p: f64| merits[((merits.len() - 1) as f64 * p).round() as usize];
    Bar::new(at(0.5).max(floor_lo), at(0.9).max(floor_hi))
}

/// Running total for one item. `merit` is what the item is *worth*; `score` is
/// that plus what the clock says about it, and only `score` decides order.
/// Keeping them apart is what lets a deadline be weighed against quality
/// instead of added to it.
struct Acc {
    /// Whether to keep the per-term breakdown. A caller that wants only the
    /// number ([`fit`]) skips building the labels.
    keep: bool,
    score: f64,
    merit: f64,
    breakdown: Vec<ScoreTerm>,
}

impl Acc {
    fn new(keep: bool) -> Acc {
        Acc {
            keep,
            score: 0.0,
            merit: 0.0,
            breakdown: Vec::new(),
        }
    }

    /// A term about the posting itself.
    fn merit(&mut self, label: impl FnOnce() -> String, points: f64) {
        if points.abs() > f64::EPSILON {
            self.score += points;
            self.merit += points;
            self.push(label, points);
        }
    }

    /// A term about when — recency, a deadline, having already looked.
    fn time(&mut self, label: impl FnOnce() -> String, points: f64) {
        if points.abs() > f64::EPSILON {
            self.score += points;
            self.push(label, points);
        }
    }

    fn push(&mut self, label: impl FnOnce() -> String, points: f64) {
        if self.keep {
            self.breakdown.push(ScoreTerm::new(label(), points));
        }
    }
}

/// The merit terms that need nothing fetched: field, kind, interests read from
/// the title and snippet, employer tier, level and place. Shared by the full
/// score and by [`fit`], so the cheap estimate used to trim a cache cannot
/// drift away from the number the feed is ordered by.
///
/// Returns what the caller records on the item: the interests that fired and
/// the employer's tier.
fn cheap_terms(item: &Item, profile: &Profile, acc: &mut Acc) -> (Vec<String>, Option<u8>) {
    let w = &profile.weights;

    // 1. Major match
    if item.discipline.as_deref() == Some(profile.major.as_str()) {
        acc.merit(|| "Your field".into(), w.major);
    }

    // 2. Item type weight
    if item.item_type.is_opportunity() {
        acc.merit(|| "Opportunity".into(), 5.0);
    } else if item.item_type == ItemType::Event {
        acc.merit(|| "Event".into(), 4.0);
    }

    // 3. Interest match
    let matched = match_interests(item, &profile.interests);
    if !matched.is_empty() {
        let points = (matched.len() as f64 * w.interest).min(w.interest_cap);
        acc.merit(|| format!("Interests: {}", matched.join(", ")), points);
    }

    // 4. Company prestige. Derived on read rather than stored, so a tier the
    // user overrides in Settings takes effect without a refresh.
    let mut tier = None;
    if let Some(company) = item.company.as_deref() {
        tier = companies::tier(company, &profile.company_tiers);
        acc.merit(
            || {
                format!(
                    "{company} (tier {})",
                    tier.map(|t| t.to_string()).unwrap_or_else(|| "—".into())
                )
            },
            w.prestige * companies::tier_value(tier),
        );
    }

    // 5. Seniority fit
    if item.item_type.is_opportunity() {
        let fit = seniority_fit(item, &profile.target_seniority);
        acc.merit(
            || match item.seniority.as_deref() {
                Some(level) if fit >= 0.0 => format!("{level} role"),
                Some(level) => format!("{level} role — not your level"),
                None => "Level".to_string(),
            },
            w.seniority * fit,
        );
    }

    // 6. Location fit
    if let Some(home) = profile.home_region.as_deref() {
        if item.location_tags.iter().any(|t| t == home) {
            acc.merit(|| format!("In {home}"), w.location);
        }
    }

    (matched, tier)
}

/// What a posting is worth from what the scrape alone says about it — no
/// description, no skills, no pay, no clock. Used where a cache has to be cut
/// to size before any of that has been fetched, and so cannot ask the ranker.
pub fn fit(item: &Item, profile: &Profile) -> f64 {
    let mut acc = Acc::new(false);
    cheap_terms(item, profile, &mut acc);
    acc.merit
}

/// Cuts the opportunities in `items` down to `cap`, keeping the best by
/// [`fit`]. Everything that is not an opportunity passes through untouched, and
/// the survivors keep their relative order. Ties go to the newer posting.
///
/// This is the cache's ceiling, and it exists so that "keep everything that is
/// still open" cannot grow without bound. It is deliberately not an age cut: a
/// posting is not worse for being old, only for being a worse fit.
pub fn cap_opportunities(items: Vec<Item>, profile: &Profile, cap: usize) -> Vec<Item> {
    let opportunities = items
        .iter()
        .filter(|i| i.item_type.is_opportunity())
        .count();
    if opportunities <= cap {
        return items;
    }

    // Scored once per row; the sort would otherwise repeat it per comparison.
    let mut scored: Vec<(usize, f64)> = items
        .iter()
        .enumerate()
        .filter(|(_, i)| i.item_type.is_opportunity())
        .map(|(at, i)| (at, fit(i, profile)))
        .collect();
    scored.sort_by(|a, b| {
        b.1.total_cmp(&a.1)
            .then(items[b.0].timestamp.cmp(&items[a.0].timestamp))
    });
    let keep: std::collections::HashSet<usize> =
        scored.into_iter().take(cap).map(|(at, _)| at).collect();

    items
        .into_iter()
        .enumerate()
        .filter(|(at, i)| !i.item_type.is_opportunity() || keep.contains(at))
        .map(|(_, i)| i)
        .collect()
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
pub fn personalize_at(items: Vec<Item>, profile: &Profile, now: DateTime<Utc>) -> Vec<Item> {
    personalize_with(items, profile, now, None).0
}

/// The scorer proper. Two passes, because a deadline is worth more on a good
/// posting than on a poor one and "good" is relative to everything else:
///
/// 1. Every item gets its `merit` and its time-dependent terms.
/// 2. With the whole set's [`Bars`] known, an item with a deadline gains the
///    urgency term scaled by how far its merit is up the ramp, and is flagged
///    `last_call` if it clears the bar with the deadline inside a week.
///
/// `bars` is `None` when ranking a whole set, which measures its own. Pass the
/// set's bars back in to score a single item (the detail pane's rescore): a
/// percentile over one item means nothing, and it would disagree with the list.
pub fn personalize_with(
    mut items: Vec<Item>,
    profile: &Profile,
    now: DateTime<Utc>,
    bars: Option<&Bars>,
) -> (Vec<Item>, Bars) {
    let w = &profile.weights;

    for item in &mut items {
        let mut acc = Acc::new(true);

        let (matched, tier) = cheap_terms(item, profile, &mut acc);
        item.matched_interests = matched;
        item.company_tier = tier;

        // Skill coverage. Opportunities only: extraction is the expensive
        // part of scoring, and "skills this posting asks for" is meaningless
        // for an article that merely mentions a technology.
        if item.item_type.is_opportunity() {
            let demand = skills::demand(item);
            let has = |skill: &str| profile.skills.iter().any(|u| u == skill);
            let matched: Vec<String> = demand
                .iter()
                .filter(|d| has(&d.skill))
                .map(|d| d.skill.clone())
                .collect();

            if demand.len() >= MIN_SKILLS_TO_SCORE {
                // Weighted by how hard the posting asked, not by how many
                // chips there are. Covering the two things a posting called
                // non-negotiable is a better fit than covering six it merely
                // preferred, and the unweighted ratio said the opposite —
                // which is how a posting with a long "nice to have" list
                // ranked below one that wanted nothing in particular.
                let total: f32 = demand.iter().map(|d| d.weight).sum();
                let met: f32 = demand
                    .iter()
                    .filter(|d| has(&d.skill))
                    .map(|d| d.weight)
                    .sum();
                let coverage = match total > 0.0 {
                    true => (met / total) as f64,
                    false => 0.0,
                };
                acc.merit(
                    || format!("Skills you have ({}/{})", matched.len(), demand.len()),
                    w.skill * coverage,
                );
            }

            item.key_skills = demand
                .iter()
                .filter(|d| d.emphasis == skills::Emphasis::Must)
                .map(|d| d.skill.clone())
                .collect();
            item.matched_skills = matched;
            item.required_skills = demand.into_iter().map(|d| d.skill).collect();
        }

        // Published pay
        if item.item_type.is_opportunity() {
            acc.merit(|| "Pay".into(), w.pay * pay_score(item));
        }

        // Recency
        acc.time(
            || "Recent".into(),
            recency_score_weighted(item.timestamp, now, w.recency),
        );

        // A closed deadline is floored rather than dropped: the reader can
        // still ask to see these, and when they do the order should mean
        // something. An *open* one waits for pass two.
        if item.closes_at.is_some()
            && (item.item_type.is_opportunity() || item.item_type == ItemType::Event)
            && urgency_score(item.closes_at, now) < 0.0
        {
            acc.time(|| "Closed".into(), -CLOSED_PENALTY);
        }

        // Already-read items sink, so the feed keeps moving.
        if item.seen {
            acc.time(|| "Already seen".into(), -w.seen_penalty);
        }

        acc.breakdown.sort_by(|a, b| b.points.total_cmp(&a.points));
        item.merit = Some(acc.merit);
        item.last_call = false;
        item.score_breakdown = acc.breakdown;
        item.relevance_score = Some(acc.score);
    }

    let bars = bars.copied().unwrap_or_else(|| Bars::measure(&items));

    for item in &mut items {
        let eligible = item.item_type.is_opportunity() || item.item_type == ItemType::Event;
        let Some(closes) = item.closes_at.filter(|_| eligible) else {
            continue;
        };
        let urgency = urgency_score(Some(closes), now);
        if urgency < 0.0 {
            continue; // closed, and already floored in pass one
        }

        let merit = item.merit.unwrap_or(0.0);
        let bar = bars.for_type(item.item_type);
        let points = w.urgency * urgency * bar.gate(merit);
        if points.abs() > f64::EPSILON {
            item.relevance_score = Some(item.relevance_score.unwrap_or(0.0) + points);
            item.score_breakdown
                .push(ScoreTerm::new("Closing soon", points));
            item.score_breakdown
                .sort_by(|a, b| b.points.total_cmp(&a.points));
        }

        let days = (closes - now).num_minutes() as f64 / (60.0 * 24.0);
        item.last_call = bar.clears(merit) && days <= LAST_CALL_DAYS;
    }

    // Stable sort, matching Python's `sorted`: ties keep scraper order.
    items.sort_by(|a, b| {
        b.relevance_score
            .unwrap_or(0.0)
            .total_cmp(&a.relevance_score.unwrap_or(0.0))
    });

    (diversify(items), bars)
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
pub(crate) fn diversify(items: Vec<Item>) -> Vec<Item> {
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
        assert_eq!(
            ranked[0].relevance_score,
            Some(w().major + 5.0 + w().recency)
        ); // field + opportunity
        assert_eq!(
            ranked[1].relevance_score,
            Some(w().major + 4.0 + w().recency)
        ); // field + event
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

        let ranked = personalize_at(
            vec![it],
            &profile("General", &["AI/ML".to_string()], &[]),
            now,
        );
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
            &profile(
                "General",
                &[],
                &["Python".to_string(), "Django".to_string()],
            ),
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

    /// Coverage is weighed by how hard the posting asked, not by how many
    /// chips it produced. Two readers who each cover two of four skills are
    /// not equally good fits when one covers the pair the posting called
    /// non-negotiable and the other covers the pair it called optional.
    #[test]
    fn coverage_weighs_what_the_posting_insisted_on() {
        let now = Utc::now();
        // A title of its own: `item` derives the URL from the title and
        // `skills::demand` memoizes on the URL, so two fixtures sharing a
        // title would have the second read back the first one's answer.
        let mut it = item("Intern at Bar", "S", ItemType::Internship).with_timestamp(now);
        it.requirements = Some(
            "Strong Python and 3+ years of PostgreSQL.\n             Nice to have:\n             Docker.\n             Kubernetes."
                .into(),
        );

        let required = personalize_at(
            vec![it.clone()],
            &profile(
                "General",
                &[],
                &["Python".to_string(), "PostgreSQL".to_string()],
            ),
            now,
        );
        let preferred = personalize_at(
            vec![it],
            &profile(
                "General",
                &[],
                &["Docker".to_string(), "Kubernetes".to_string()],
            ),
            now,
        );

        assert_eq!(required[0].matched_skills.len(), 2);
        assert_eq!(preferred[0].matched_skills.len(), 2);
        assert!(
            required[0].relevance_score > preferred[0].relevance_score,
            "covering the must-haves scored {:?}, the nice-to-haves {:?}",
            required[0].relevance_score,
            preferred[0].relevance_score
        );
        assert_eq!(required[0].key_skills, vec!["Python", "PostgreSQL"]);
    }

    #[test]
    fn a_posting_naming_one_skill_earns_nothing() {
        let now = Utc::now();
        let mut it = item("Ops Intern", "S", ItemType::Internship).with_timestamp(now);
        it.content_text = "Some familiarity with Terraform.".into();

        let ranked = personalize_at(
            vec![it],
            &profile("General", &[], &["Terraform".to_string()]),
            now,
        );
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

        let ranked = personalize_at(
            vec![it],
            &profile("General", &[], &["Docker".to_string()]),
            now,
        );
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

    // --- a deadline, weighed against how good the posting is ---
    //
    // The ramp is pinned (5 → 13 for roles, 4 → 8 for events) so each case
    // reads as arithmetic: a posting at merit 9 is halfway up and earns half
    // the deadline boost. Interest keywords are the lever for merit — "pytorch"
    // fires AI/ML and "react" fires Frontend, four points each.

    use chrono::Duration;

    fn pinned_bars() -> Bars {
        Bars {
            opportunity: Bar { lo: 5.0, hi: 13.0 },
            event: Bar { lo: 4.0, hi: 8.0 },
        }
    }

    fn reader() -> Profile {
        profile(
            DEFAULT_MAJOR,
            &["AI/ML".to_string(), "Frontend".to_string()],
            &[],
        )
    }

    const WEAK: &str = "general duties";
    const MIDDLING: &str = "pytorch";
    const STRONG: &str = "pytorch react";

    fn closing(
        title: &str,
        ty: ItemType,
        text: &str,
        in_days: Option<f64>,
        now: DateTime<Utc>,
    ) -> Item {
        let mut it = item(title, "S", ty).with_timestamp(now);
        it.content_text = text.into();
        it.closes_at = in_days.map(|d| now + Duration::minutes((d * 1440.0) as i64));
        it
    }

    fn scored(items: Vec<Item>, now: DateTime<Utc>) -> Vec<Item> {
        personalize_with(items, &reader(), now, Some(&pinned_bars())).0
    }

    fn named<'a>(items: &'a [Item], title: &str) -> &'a Item {
        items.iter().find(|i| i.title == title).expect(title)
    }

    fn term(item: &Item, label: &str) -> f64 {
        item.score_breakdown
            .iter()
            .find(|t| t.label == label)
            .map_or(0.0, |t| t.points)
    }

    fn close_to(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-6, "{a} != {b}");
    }

    #[test]
    fn urgency_is_full_for_the_last_days_then_halves_each_week() {
        let now = Utc::now();
        let at =
            |days: f64| urgency_score(Some(now + Duration::minutes((days * 1440.0) as i64)), now);

        close_to(at(0.5), 1.0);
        close_to(at(URGENCY_FULL_DAYS), 1.0);
        close_to(at(URGENCY_FULL_DAYS + URGENCY_HALF_LIFE_DAYS), 0.5);
        close_to(at(URGENCY_FULL_DAYS + 2.0 * URGENCY_HALF_LIFE_DAYS), 0.25);
        close_to(at(URGENCY_HORIZON_DAYS + 1.0), 0.0);
        close_to(at(-0.5), -1.0);
        close_to(urgency_score(None, now), 0.0);

        // Never falls as the date gets closer — the old curve did, inside six days.
        let days = [40.0, 20.0, 10.0, 5.0, 3.0, 1.0, 0.1];
        for pair in days.windows(2) {
            assert!(at(pair[1]) >= at(pair[0]), "urgency fell between {pair:?}");
        }
    }

    #[test]
    fn a_deadline_helps_a_good_posting_more_than_a_poor_one() {
        let now = Utc::now();
        let ranked = scored(
            vec![
                closing("dl-weak", ItemType::Job, WEAK, Some(2.0), now),
                closing("dl-mid", ItemType::Job, MIDDLING, Some(2.0), now),
                closing("dl-strong", ItemType::Job, STRONG, Some(2.0), now),
            ],
            now,
        );
        close_to(term(named(&ranked, "dl-weak"), "Closing soon"), 0.0);
        close_to(
            term(named(&ranked, "dl-mid"), "Closing soon"),
            w().urgency * 0.5,
        );
        close_to(
            term(named(&ranked, "dl-strong"), "Closing soon"),
            w().urgency,
        );
    }

    /// The failure the additive term had: the same bonus for everything, so a
    /// poor posting closing tomorrow could outrank a better one closing later.
    /// Merit 9 against 5 is a four-point gap, which an ungated boost of up to
    /// eight would erase — so this fails if the quality gate is removed.
    #[test]
    fn a_weak_posting_closing_tomorrow_does_not_jump_a_better_one_closing_later() {
        let now = Utc::now();
        let ranked = scored(
            vec![
                closing("jump-weak", ItemType::Job, WEAK, Some(1.0), now),
                closing("jump-better", ItemType::Job, MIDDLING, Some(30.0), now),
            ],
            now,
        );
        assert!(
            named(&ranked, "jump-better").relevance_score
                > named(&ranked, "jump-weak").relevance_score
        );
    }

    #[test]
    fn the_boost_never_shrinks_as_the_deadline_gets_closer() {
        let now = Utc::now();
        let boosts: Vec<f64> = [40.0, 20.0, 10.0, 5.0, 3.0, 1.0, 0.1]
            .iter()
            .map(|d| {
                let ranked = scored(
                    vec![closing("monotone", ItemType::Job, STRONG, Some(*d), now)],
                    now,
                );
                term(&ranked[0], "Closing soon")
            })
            .collect();
        for pair in boosts.windows(2) {
            assert!(pair[1] >= pair[0], "boost fell: {boosts:?}");
        }
        assert!(boosts[0] > 0.0 && boosts[6] > boosts[0]);
    }

    #[test]
    fn a_closed_posting_is_floored_and_is_never_a_last_call() {
        let now = Utc::now();
        let ranked = scored(
            vec![closing("shut", ItemType::Job, STRONG, Some(-1.0), now)],
            now,
        );
        close_to(term(&ranked[0], "Closed"), -CLOSED_PENALTY);
        close_to(term(&ranked[0], "Closing soon"), 0.0);
        assert!(!ranked[0].last_call);
    }

    /// Devpost's deadline used to be stored and shown but never scored, because
    /// the whole term was gated on being a job.
    #[test]
    fn an_event_with_a_deadline_is_scored_and_one_without_is_not() {
        let now = Utc::now();
        let ranked = scored(
            vec![
                closing("hack-with", ItemType::Event, MIDDLING, Some(2.0), now),
                closing("hack-without", ItemType::Event, MIDDLING, None, now),
                closing("hack-over", ItemType::Event, MIDDLING, Some(-2.0), now),
            ],
            now,
        );
        // Merit 4 + 4 = 8 clears the event bar (4 → 8) completely.
        close_to(
            term(named(&ranked, "hack-with"), "Closing soon"),
            w().urgency,
        );
        assert!(named(&ranked, "hack-with").last_call);
        close_to(term(named(&ranked, "hack-without"), "Closing soon"), 0.0);
        assert!(!named(&ranked, "hack-without").last_call);
        close_to(term(named(&ranked, "hack-over"), "Closed"), -CLOSED_PENALTY);
    }

    #[test]
    fn a_last_call_needs_both_quality_and_time() {
        let now = Utc::now();
        let ranked = scored(
            vec![
                closing("lc-yes", ItemType::Job, STRONG, Some(6.0), now),
                closing(
                    "lc-late",
                    ItemType::Job,
                    STRONG,
                    Some(LAST_CALL_DAYS + 1.0),
                    now,
                ),
                closing("lc-meh", ItemType::Job, MIDDLING, Some(2.0), now),
                closing("lc-open", ItemType::Job, STRONG, None, now),
            ],
            now,
        );
        assert!(named(&ranked, "lc-yes").last_call);
        assert!(
            !named(&ranked, "lc-late").last_call,
            "a week out is not a last call"
        );
        assert!(
            !named(&ranked, "lc-meh").last_call,
            "good-ish is not good enough"
        );
        assert!(
            !named(&ranked, "lc-open").last_call,
            "no deadline, no urgency"
        );
    }

    /// Merit is what a posting is worth; the clock must not move it, or a
    /// deadline would be weighed against how long ago it was posted.
    #[test]
    fn merit_ignores_the_clock_and_having_already_read_it() {
        let now = Utc::now();
        let fresh = closing("clock-a", ItemType::Job, STRONG, None, now);
        let mut stale = closing("clock-b", ItemType::Job, STRONG, None, now);
        stale.timestamp = now - Duration::days(90);
        stale.seen = true;

        let ranked = scored(vec![fresh, stale], now);
        close_to(
            named(&ranked, "clock-a").merit.unwrap(),
            named(&ranked, "clock-b").merit.unwrap(),
        );
        assert!(
            named(&ranked, "clock-a").relevance_score > named(&ranked, "clock-b").relevance_score
        );
    }

    #[test]
    fn the_bars_are_measured_from_the_set_and_never_fall_below_their_floor() {
        let now = Utc::now();
        // Everything ties at merit 5, which is what an empty profile looks like.
        let flat: Vec<Item> = (0..6)
            .map(|i| closing(&format!("flat-{i}"), ItemType::Job, WEAK, None, now))
            .collect();
        let (_, bars) = personalize_with(flat, &reader(), now, None);
        assert_eq!(bars.opportunity, Bar { lo: 9.0, hi: 13.0 });
        assert_eq!(
            bars.event,
            Bar { lo: 6.0, hi: 12.0 },
            "no events at all falls back to the floor"
        );

        // A spread: half at 5 and half at 13 puts both percentiles on 13, and
        // a ramp with no width would divide by zero.
        let spread: Vec<Item> = (0..20)
            .map(|i| {
                let text = if i < 10 { WEAK } else { STRONG };
                closing(&format!("spread-{i}"), ItemType::Job, text, None, now)
            })
            .collect();
        let (_, bars) = personalize_with(spread, &reader(), now, None);
        assert_eq!(bars.opportunity.lo, 13.0);
        assert!(bars.opportunity.hi >= bars.opportunity.lo + MIN_RAMP);
        assert!(bars.opportunity.gate(13.0).is_finite());
    }

    /// The detail pane rescoring one item must agree with the list it was
    /// clicked from, which a percentile over a single item could not.
    #[test]
    fn one_item_rescored_with_the_sets_bars_agrees_with_the_list() {
        let now = Utc::now();
        let items: Vec<Item> = (0..10)
            .map(|i| {
                let text = if i % 2 == 0 { WEAK } else { STRONG };
                closing(&format!("agree-{i}"), ItemType::Job, text, Some(2.0), now)
            })
            .collect();
        let alone = items[3].clone();

        let (list, bars) = personalize_with(items, &reader(), now, None);
        let (single, _) = personalize_with(vec![alone], &reader(), now, Some(&bars));

        let in_list = named(&list, "agree-3");
        assert_eq!(single[0].relevance_score, in_list.relevance_score);
        assert_eq!(single[0].last_call, in_list.last_call);
    }

    /// `fit` trims a cache before anything is fetched, and has to be the same
    /// arithmetic as the ranker's merit for what the scrape alone can see.
    #[test]
    fn fit_is_the_part_of_merit_a_scrape_can_see() {
        let now = Utc::now();
        let mut it = closing("fit-one", ItemType::Job, STRONG, None, now);
        it.company = Some("Google".into());

        let ranked = scored(vec![it.clone()], now);
        close_to(fit(&it, &reader()), ranked[0].merit.unwrap());
        // 5 (opportunity) + 8 (two interests) + 8 (tier 1)
        close_to(fit(&it, &reader()), 21.0);
    }

    // --- the ceiling ---

    fn titles_of(items: &[Item]) -> Vec<&str> {
        items.iter().map(|i| i.title.as_str()).collect()
    }

    #[test]
    fn under_the_ceiling_nothing_moves() {
        let now = Utc::now();
        let items = vec![
            closing("cap-a", ItemType::Job, WEAK, None, now),
            closing("cap-b", ItemType::Job, STRONG, None, now),
        ];
        let kept = cap_opportunities(items, &reader(), 2);
        assert_eq!(titles_of(&kept), ["cap-a", "cap-b"]);
    }

    #[test]
    fn over_the_ceiling_the_best_fit_stays_and_news_is_never_cut() {
        let now = Utc::now();
        let items = vec![
            closing("cut-weak-1", ItemType::Job, WEAK, None, now),
            closing("cut-strong", ItemType::Job, STRONG, None, now),
            closing("cut-news", ItemType::Article, WEAK, None, now),
            closing("cut-mid", ItemType::Internship, MIDDLING, None, now),
            closing("cut-weak-2", ItemType::Job, WEAK, None, now),
        ];
        let kept = cap_opportunities(items, &reader(), 2);
        // Survivors keep their original order, and the article is not counted.
        assert_eq!(titles_of(&kept), ["cut-strong", "cut-news", "cut-mid"]);
    }

    #[test]
    fn a_tie_at_the_ceiling_goes_to_the_newer_posting() {
        let now = Utc::now();
        let mut older = closing("tie-older", ItemType::Job, WEAK, None, now);
        older.timestamp = now - Duration::days(10);
        let newer = closing("tie-newer", ItemType::Job, WEAK, None, now);
        let kept = cap_opportunities(vec![older, newer], &reader(), 1);
        assert_eq!(titles_of(&kept), ["tie-newer"]);
    }

    /// The form offers exactly the bands the classifier can produce: a band
    /// the classifier never assigns would be a preference that matches nothing,
    /// and one it assigns but the form omits could never be chosen.
    #[test]
    fn the_seniority_choices_are_the_classifiers_bands() {
        let mut offered: Vec<&str> = seniority_levels();
        let mut classified: Vec<&str> = SENIORITY.iter().map(|(name, _)| *name).collect();
        offered.sort_unstable();
        classified.sort_unstable();
        assert_eq!(offered, classified);
    }
}
