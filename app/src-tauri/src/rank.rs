//! Discipline classification and feed ranking, ported from the Python
//! pipeline's categorization and prioritization layers.

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

/// A single source may not place more than this many items in the ranked head,
/// so one large source (an internship repo with 150 rows) can't flood the feed.
const MAX_PER_SOURCE: usize = 8;

/// Scores every item against the user's major, sorts by score, then applies the
/// per-source diversification pass.
pub fn personalize(mut items: Vec<Item>, user_major: &str) -> Vec<Item> {
    for item in &mut items {
        let mut score = 0.0;

        // 1. Major match (primary weight)
        if item.discipline.as_deref() == Some(user_major) {
            score += 10.0;
        }

        // 2. Item type weight
        if item.item_type.is_opportunity() {
            score += 5.0;
        } else if item.item_type == ItemType::Event {
            score += 4.0;
        }

        // 3. Recency weight — still unimplemented, as in the Python original.

        item.relevance_score = Some(score);
    }

    // Stable sort, matching Python's `sorted`: ties keep scraper order.
    items.sort_by(|a, b| {
        b.relevance_score
            .unwrap_or(0.0)
            .total_cmp(&a.relevance_score.unwrap_or(0.0))
    });

    let mut per_source: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut head = Vec::with_capacity(items.len());
    let mut tail = Vec::new();

    for item in items {
        let count = per_source.entry(item.source_platform.clone()).or_insert(0);
        *count += 1;
        if *count <= MAX_PER_SOURCE {
            head.push(item);
        } else {
            tail.push(item);
        }
    }

    head.extend(tail);
    head
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(title: &str, source: &str, ty: ItemType) -> Item {
        Item::new(title, source, ty, format!("https://example.com/{title}"))
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

        let ranked = personalize(vec![article, event, job], "Software Engineering");
        assert_eq!(ranked[0].relevance_score, Some(15.0)); // major + opportunity
        assert_eq!(ranked[1].relevance_score, Some(14.0)); // major + event
        assert_eq!(ranked[2].relevance_score, Some(0.0)); // no discipline match
    }

    #[test]
    fn diversification_pushes_source_overflow_to_the_tail() {
        // 20 items from one flooding source, plus one low-scoring item from another.
        let mut items: Vec<Item> = (0..20)
            .map(|i| {
                let mut it = item(&format!("flood-{i}"), "Flooder", ItemType::Internship);
                it.discipline = Some("Software Engineering".into());
                it
            })
            .collect();
        items.push(item("lonely", "Other", ItemType::Article));

        let ranked = personalize(items, "Software Engineering");

        // The single Other item scores 0 but must still appear before the
        // flooding source's ninth item.
        let other_pos = ranked
            .iter()
            .position(|i| i.source_platform == "Other")
            .unwrap();
        assert_eq!(other_pos, MAX_PER_SOURCE);
        assert_eq!(ranked.len(), 21);
        assert_eq!(ranked[MAX_PER_SOURCE + 1].source_platform, "Flooder");
    }
}
