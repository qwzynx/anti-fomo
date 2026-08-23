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

/// How a description fetch turned out. Every variant is a reason *not* to try
/// that URL again, which is the whole point of storing it: without a recorded
/// failure, a dead link costs one request per refresh forever.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DetailStatus {
    /// Fetched and parsed; at least one field has text.
    Ok,
    /// Fetched and parsed, but the posting carried nothing usable.
    Empty,
    /// Network error, non-200, or a payload we could not parse. The default,
    /// so a half-built `JobDetail` errs towards "do not trust this".
    #[default]
    Failed,
    /// No handler claims this URL — the honest majority of the long tail.
    Unsupported,
}

impl DetailStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            DetailStatus::Ok => "ok",
            DetailStatus::Empty => "empty",
            DetailStatus::Failed => "failed",
            DetailStatus::Unsupported => "unsupported",
        }
    }

    /// Infallible like [`ItemType::from_label`]: an unrecognised label from an
    /// older row reads as a failure, which suppresses a retry rather than
    /// dropping the row.
    pub fn from_label(s: &str) -> Self {
        match s {
            "ok" => DetailStatus::Ok,
            "empty" => DetailStatus::Empty,
            "unsupported" => DetailStatus::Unsupported,
            _ => DetailStatus::Failed,
        }
    }
}

/// What one posting's page told us. Every text field is optional: a page can
/// parse perfectly well and still have no perks section.
#[derive(Clone, Debug, Default)]
pub struct JobDetail {
    pub description: Option<String>,
    pub requirements: Option<String>,
    pub responsibilities: Option<String>,
    pub perks: Option<String>,
    /// Skills the source itself tagged the posting with — the highest-quality
    /// signal available, because a human or the source's own pipeline picked
    /// them rather than a keyword scan.
    pub tagged_skills: Vec<String>,
    pub status: DetailStatus,
}

impl JobDetail {
    pub fn with_status(status: DetailStatus) -> Self {
        JobDetail {
            status,
            ..Default::default()
        }
    }

    /// True when anything worth storing came back.
    pub fn has_text(&self) -> bool {
        [
            &self.description,
            &self.requirements,
            &self.responsibilities,
            &self.perks,
        ]
        .iter()
        .any(|f| f.as_ref().is_some_and(|s| !s.trim().is_empty()))
            || !self.tagged_skills.is_empty()
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
    /// The simplify.jobs posting id, when the source supplied one. Stored on
    /// the row (unlike the derived fields below) because the enrichment pass
    /// reads its work queue out of the database.
    #[serde(default)]
    pub simplify_id: Option<String>,

    // --- derived, never stored on the `items` row ---
    // These are recomputed on every read: the first three by
    // `rank::personalize`, the last two by joining `item_state`.
    // `db::save_items` ignores them and `db::load_items` leaves them at their
    // defaults.
    /// Interest tags that fired for this item, so a card can show *why* it ranked.
    #[serde(default)]
    pub matched_interests: Vec<String>,
    /// Catalog skills this posting asks for. Opportunities only — an article
    /// that mentions Kubernetes is not asking you to know it.
    #[serde(default)]
    pub required_skills: Vec<String>,
    /// The subset of `required_skills` the user has declared they have.
    #[serde(default)]
    pub matched_skills: Vec<String>,

    // The fetched posting, joined from `job_details` on read. Absent for the
    // postings the chain in `scrapers::details` could not serve — those report
    // no skills at all rather than a guess, and the UI says which it is.
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub requirements: Option<String>,
    #[serde(default)]
    pub responsibilities: Option<String>,
    #[serde(default)]
    pub perks: Option<String>,
    /// Skills the source itself tagged the posting with. Higher confidence
    /// than anything a keyword scan can conclude, so extraction trusts these
    /// outright where they name a skill the catalog knows.
    #[serde(default)]
    pub tagged_skills: Vec<String>,

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
            simplify_id: None,
            matched_interests: Vec::new(),
            required_skills: Vec::new(),
            matched_skills: Vec::new(),
            description: None,
            requirements: None,
            responsibilities: None,
            perks: None,
            tagged_skills: Vec::new(),
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
