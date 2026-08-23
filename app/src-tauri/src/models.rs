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
    /// The application deadline, when the posting's own page published one —
    /// Workday's `endDate`, schema.org's `validThrough`, Job Bank's
    /// "Advertised until". Stored beside the description for the same reason
    /// the description is: `save_items` rewrites the `items` row on every
    /// refresh and would erase it.
    pub closes_at: Option<DateTime<Utc>>,
    /// Published pay found on the posting's page, normalised by `pay::parse`.
    pub salary_min: Option<f64>,
    pub salary_max: Option<f64>,
    pub salary_currency: Option<String>,
    pub salary_period: Option<String>,
    pub status: DetailStatus,
}

impl JobDetail {
    pub fn with_status(status: DetailStatus) -> Self {
        JobDetail {
            status,
            ..Default::default()
        }
    }

    /// True when the fetch learned a deadline or a pay range. Deliberately
    /// separate from [`has_text`](Self::has_text): a page that yields only a
    /// `validThrough` has still not told us what the job is, so it must not
    /// end the handler chain — but the date is worth keeping.
    pub fn has_facts(&self) -> bool {
        self.closes_at.is_some() || self.salary_min.is_some() || self.salary_max.is_some()
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

/// One term of the relevance score, as the UI shows it. `label` is display
/// text, not an identifier — nothing switches on it.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ScoreTerm {
    pub label: String,
    pub points: f64,
}

impl ScoreTerm {
    pub fn new(label: impl Into<String>, points: f64) -> Self {
        ScoreTerm {
            label: label.into(),
            points,
        }
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

    /// The employer, as its own field rather than a fragment of `title`.
    /// Sources that know it say so; everything else falls back to
    /// [`company_from_title`]. This is what the prestige term, the employer
    /// filter and the A-Z sort all key on, and it is why "Company name" used
    /// to sort by role.
    #[serde(default)]
    pub company: Option<String>,
    /// When applications close. Absent for the majority — most sources simply
    /// do not publish one — which is a different thing from "closes today",
    /// so every sort and filter over this treats `None` as unknown and puts it
    /// last rather than coercing it to a date.
    #[serde(default)]
    pub closes_at: Option<DateTime<Utc>>,
    /// Published compensation, and only published compensation. Nothing here
    /// is ever estimated: a guessed number and a disclosed one look identical
    /// once they are both a range on a card.
    #[serde(default)]
    pub salary_min: Option<f64>,
    #[serde(default)]
    pub salary_max: Option<f64>,
    #[serde(default)]
    pub salary_currency: Option<String>,
    /// "year" | "month" | "week" | "day" | "hour" — what `salary_min`/`max`
    /// are denominated in. `pay::annual_equivalent` is what makes two postings
    /// on different periods comparable.
    #[serde(default)]
    pub salary_period: Option<String>,
    /// Intern | New grad | Junior | Mid | Senior | Lead, read off the title.
    /// Stored rather than derived because the internships hub filters on it
    /// and a filter should not re-run a classifier over the whole cache.
    #[serde(default)]
    pub seniority: Option<String>,

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
    /// 1-4, strongest first, from `companies::tier` with the user's overrides
    /// applied. Derived on read so an override takes effect without a refresh.
    #[serde(default)]
    pub company_tier: Option<u8>,
    /// Every scoring term that fired, with what it contributed. The detail
    /// pane renders this: a ranking the reader cannot interrogate is
    /// indistinguishable from an arbitrary one.
    #[serde(default)]
    pub score_breakdown: Vec<ScoreTerm>,

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
            company: None,
            closes_at: None,
            salary_min: None,
            salary_max: None,
            salary_currency: None,
            salary_period: None,
            seniority: None,
            matched_interests: Vec::new(),
            required_skills: Vec::new(),
            matched_skills: Vec::new(),
            company_tier: None,
            score_breakdown: Vec::new(),
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

    pub fn with_company(mut self, company: impl Into<String>) -> Self {
        let name = company.into();
        self.company = Some(name).filter(|s| !s.trim().is_empty());
        self
    }

    pub fn with_closes_at(mut self, closes_at: Option<DateTime<Utc>>) -> Self {
        self.closes_at = closes_at;
        self
    }
}

/// Recovers the employer from a `"{role} at {company}"` title.
///
/// Splits on the **last** " at ", matching `splitTitle` in `lib/item.ts` —
/// "Software Engineer, Data at Palantir at Denver" is one role at Palantir,
/// not a role called "Software Engineer, Data" at "Palantir at Denver". A
/// title with no separator has no company to recover, and says so.
pub fn company_from_title(title: &str) -> Option<String> {
    let (_, company) = title.rsplit_once(" at ")?;
    let company = company.trim();
    // A trailing fragment that is really a location ("... at Toronto, ON") is
    // worse than no answer, since it would become a bucket in every group-by.
    if company.is_empty() || company.len() > 60 {
        return None;
    }
    Some(company.to_string())
}
