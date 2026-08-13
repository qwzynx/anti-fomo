use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Mirrors the `ItemType` enum the old FastAPI backend exposed. The variant
/// names serialize verbatim ("Job", "Article", …), which is exactly the shape
/// the UI already switches on — do not add a `rename_all`.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemType {
    Job,
    Article,
    Event,
    Internship,
}

impl ItemType {
    pub fn as_str(self) -> &'static str {
        match self {
            ItemType::Job => "Job",
            ItemType::Article => "Article",
            ItemType::Event => "Event",
            ItemType::Internship => "Internship",
        }
    }

    /// Inverse of [`as_str`](Self::as_str). Infallible by design: an
    /// unrecognised label from an older database row reads back as an article
    /// rather than dropping the row.
    pub fn from_label(s: &str) -> Self {
        match s {
            "Job" => ItemType::Job,
            "Event" => ItemType::Event,
            "Internship" => ItemType::Internship,
            _ => ItemType::Article,
        }
    }

    /// Jobs and internships are the two types the internships hub keeps.
    pub fn is_opportunity(self) -> bool {
        matches!(self, ItemType::Job | ItemType::Internship)
    }
}

/// One feed entry. Field names match the old `ScrapedItem` TypedDict and the
/// TypeScript interface the UI was written against, so the Svelte components
/// consume this without a rename pass.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Item {
    pub title: String,
    pub source_platform: String,
    pub item_type: ItemType,
    pub url: String,
    pub content_text: String,
    pub timestamp: DateTime<Utc>,
    pub discipline: Option<String>,
    pub relevance_score: Option<f64>,
    pub location: Option<String>,
    #[serde(default)]
    pub location_tags: Vec<String>,

    // --- derived, never stored on the `items` row ---
    // These are recomputed on every read: the first by `rank::personalize`, the
    // other two by joining `item_state`. `db::save_items` ignores them and
    // `db::load_items` leaves them at their defaults.
    /// Interest tags that fired for this item, so a card can show *why* it ranked.
    #[serde(default)]
    pub matched_interests: Vec<String>,
    #[serde(default)]
    pub saved: bool,
    #[serde(default)]
    pub seen: bool,
}

impl Item {
    pub fn new(
        title: impl Into<String>,
        source_platform: impl Into<String>,
        item_type: ItemType,
        url: impl Into<String>,
    ) -> Self {
        Item {
            title: title.into(),
            source_platform: source_platform.into(),
            item_type,
            url: url.into(),
            content_text: String::new(),
            timestamp: Utc::now(),
            discipline: None,
            relevance_score: None,
            location: None,
            location_tags: Vec::new(),
            matched_interests: Vec::new(),
            saved: false,
            seen: false,
        }
    }

    pub fn with_content(mut self, content: impl Into<String>) -> Self {
        self.content_text = content.into();
        self
    }

    pub fn with_timestamp(mut self, ts: DateTime<Utc>) -> Self {
        self.timestamp = ts;
        self
    }

    /// Empty strings become `None` so the UI's `location &&` checks behave the
    /// same way they did against the Python `or None` coercion.
    pub fn with_location(mut self, location: Option<String>) -> Self {
        self.location = location.filter(|s| !s.trim().is_empty());
        self
    }
}
