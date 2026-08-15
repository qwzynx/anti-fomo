//! Discipline classification and feed ranking, ported from the Python
//! pipeline's categorization and prioritization layers.

use chrono::{DateTime, Utc};

use crate::models::{Item, ItemType};

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

/// Points for an item whose discipline equals the user's major.
const MAJOR_WEIGHT: f64 = 10.0;
/// Points per matched interest tag, and the ceiling on their sum. The cap stops
/// a keyword-stuffed job description from outranking everything on breadth alone.
const INTEREST_WEIGHT: f64 = 4.0;
const MAX_INTEREST_SCORE: f64 = 8.0;
/// Full value of the recency term, awarded to something happening right now.
const RECENCY_WEIGHT: f64 = 6.0;
/// Hours for the recency term to halve: two days old is worth half, four days a
/// quarter. Tuned so a fresh article can outrank a stale internship, but a fresh
/// internship still beats a fresh article.
const RECENCY_HALF_LIFE_HOURS: f64 = 48.0;

/// Exponential decay on distance from now, in *either* direction. A week-old
/// article and an event a week out are equally far from "right now" and should
/// both rank below today's — without the absolute value, Luma events dated
/// months ahead would take over the feed.
pub fn recency_score(timestamp: DateTime<Utc>, now: DateTime<Utc>) -> f64 {
    let hours = (now - timestamp).num_minutes().abs() as f64 / 60.0;
    RECENCY_WEIGHT * 0.5_f64.powf(hours / RECENCY_HALF_LIFE_HOURS)
}

/// Scores every item against the user's major and interests, sorts by score,
/// then applies the per-source diversification pass. Also records which
/// interests fired, so the UI can explain the ranking.
pub fn personalize(items: Vec<Item>, user_major: &str, interests: &[String]) -> Vec<Item> {
    personalize_at(items, user_major, interests, Utc::now())
}

/// [`personalize`] with an injectable clock, so the recency term is testable.
pub fn personalize_at(
    mut items: Vec<Item>,
    user_major: &str,
    interests: &[String],
    now: DateTime<Utc>,
) -> Vec<Item> {
    for item in &mut items {
        let mut score = 0.0;

        // 1. Major match (primary weight)
        if item.discipline.as_deref() == Some(user_major) {
            score += MAJOR_WEIGHT;
        }

        // 2. Item type weight
        if item.item_type.is_opportunity() {
            score += 5.0;
        } else if item.item_type == ItemType::Event {
            score += 4.0;
        }

        // 3. Interest match
        let matched = match_interests(item, interests);
        score += (matched.len() as f64 * INTEREST_WEIGHT).min(MAX_INTEREST_SCORE);
        item.matched_interests = matched;

        // 4. Recency
        score += recency_score(item.timestamp, now);

        // 5. Already-read items sink, so the feed keeps moving between refreshes.
        if item.seen {
            score -= 3.0;
        }

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
        let source = item.source_platform.clone();
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

    /// Pins every item to `now` so tests that predate the recency term still
    /// see the flat scores they were written against.
    fn rank(items: Vec<Item>, major: &str) -> Vec<Item> {
        let now = Utc::now();
        let items = items.into_iter().map(|i| i.with_timestamp(now)).collect();
        personalize_at(items, major, &[], now)
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
        assert_eq!(ranked[0].relevance_score, Some(15.0 + RECENCY_WEIGHT)); // major + opportunity
        assert_eq!(ranked[1].relevance_score, Some(14.0 + RECENCY_WEIGHT)); // major + event
        assert_eq!(ranked[2].relevance_score, Some(RECENCY_WEIGHT)); // no discipline match
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

        assert!((fresh - RECENCY_WEIGHT).abs() < 1e-9);
        assert!((two_days - RECENCY_WEIGHT / 2.0).abs() < 1e-6);
        assert!((four_days - RECENCY_WEIGHT / 4.0).abs() < 1e-6);
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

        let ranked = personalize_at(vec![stale, fresh], DEFAULT_MAJOR, &[], now);
        assert_eq!(ranked[0].title, "new");
    }

    #[test]
    fn interests_add_score_and_are_recorded() {
        let now = Utc::now();
        let mut it = item("Research Intern", "S", ItemType::Internship).with_timestamp(now);
        it.content_text = "Work on PyTorch and computer vision for our LLM team".into();

        let ranked = personalize_at(vec![it], "General", &["AI/ML".to_string()], now);
        assert_eq!(ranked[0].matched_interests, vec!["AI/ML"]);
        // opportunity (5) + one interest (4) + full recency (6)
        assert_eq!(ranked[0].relevance_score, Some(5.0 + 4.0 + RECENCY_WEIGHT));
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
        let ranked = personalize_at(vec![it], "General", &wanted, now);

        assert!(ranked[0].matched_interests.len() >= 4);
        // Five matches would be 20 points uncapped; the cap holds it to 8.
        assert_eq!(
            ranked[0].relevance_score,
            Some(MAX_INTEREST_SCORE + RECENCY_WEIGHT)
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
    fn seen_items_sink_below_unseen_equivalents() {
        let now = Utc::now();
        let mut read = item("read", "S", ItemType::Article).with_timestamp(now);
        read.seen = true;
        let unread = item("unread", "S", ItemType::Article).with_timestamp(now);

        let ranked = personalize_at(vec![read, unread], DEFAULT_MAJOR, &[], now);
        assert_eq!(ranked[0].title, "unread");
    }
}
