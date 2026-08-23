//! Lever job boards.
//!
//! Lever's public endpoint returns the *whole* posting — description, the
//! employer's own `lists` already split into titled sections, and a
//! `salaryRange` where one is published. So this scraper seeds `job_details`
//! directly instead of leaving several hundred postings on the enrichment
//! queue to be fetched one URL at a time.
//!
//! The sections still go through `scrapers::sections`, per the rule in
//! `CLAUDE.md`: a handler that flattens a description to plain text throws
//! away both the structure the UI renders and the headings that separate
//! requirements from the culture blurb.

use anyhow::Result;
use async_trait::async_trait;
use chrono::{TimeZone, Utc};
use serde::Deserialize;

use super::super::employers::{Board, Employer};
use super::super::sections::{self, Section, Sections};
use super::{fan_out, seed_detail};
use crate::models::{Item, ItemType};
use crate::pay;
use crate::scrapers::{details, Scraper};

const MAX_PER_EMPLOYER: usize = 400;

pub struct LeverBoards;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Posting {
    #[serde(default)]
    text: String,
    #[serde(default)]
    hosted_url: String,
    #[serde(default)]
    created_at: Option<i64>,
    #[serde(default)]
    categories: Option<Categories>,
    /// The opening blurb, then `lists`, then the closing block. Lever splits
    /// them for us, which is better than any heading guess.
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    lists: Vec<TitledList>,
    #[serde(default)]
    additional: Option<String>,
    #[serde(default)]
    salary_range: Option<SalaryRange>,
}

#[derive(Deserialize)]
struct Categories {
    #[serde(default)]
    location: Option<String>,
}

#[derive(Deserialize)]
struct TitledList {
    #[serde(default)]
    text: String,
    #[serde(default)]
    content: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SalaryRange {
    #[serde(default)]
    min: Option<f64>,
    #[serde(default)]
    max: Option<f64>,
    #[serde(default)]
    currency: Option<String>,
    #[serde(default)]
    interval: Option<String>,
}

#[async_trait]
impl Scraper for LeverBoards {
    fn source_name(&self) -> &'static str {
        "Lever"
    }

    async fn fetch(&self, client: &reqwest::Client) -> Result<Vec<Item>> {
        Ok(fan_out(client, |b| matches!(b, Board::Lever(_)), |e, c| Box::pin(fetch_board(e, c))).await)
    }
}

async fn fetch_board(employer: &'static Employer, client: reqwest::Client) -> Result<Vec<Item>> {
    let Board::Lever(slug) = employer.board else {
        return Ok(Vec::new());
    };
    let url = format!("https://api.lever.co/v0/postings/{slug}?mode=json");
    let postings: Vec<Posting> = client
        .get(&url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    Ok(postings
        .into_iter()
        .filter(|p| !p.hosted_url.is_empty() && !p.text.trim().is_empty())
        .take(MAX_PER_EMPLOYER)
        .map(|posting| to_item(employer.name, posting))
        .collect())
}

fn to_item(company: &str, posting: Posting) -> Item {
    let location = posting.categories.as_ref().and_then(|c| c.location.clone());
    let timestamp = posting
        .created_at
        .and_then(|ms| Utc.timestamp_millis_opt(ms).single())
        // Lever always sets createdAt; a row without one is malformed rather
        // than new, so it must not collect the recency bonus.
        .unwrap_or_else(|| Utc::now() - chrono::Duration::days(30));

    let content = match &location {
        Some(l) if !l.trim().is_empty() => format!("{company} · {l}"),
        _ => company.to_string(),
    };

    let mut item = Item::new(posting.text.trim(), "Lever", ItemType::Job, posting.hosted_url.clone())
        .with_company(company)
        .with_location(location)
        .with_timestamp(timestamp)
        .with_content(content);

    if let Some(p) = posting.salary_range.as_ref().and_then(salary) {
        item.salary_min = p.min;
        item.salary_max = p.max;
        item.salary_currency = p.currency.clone();
        item.salary_period = p.period.clone();
    }

    seed_detail(&mut item, details::detail_of(assemble(&posting), Vec::new()));
    item
}

/// Rebuilds the posting from the three blocks Lever keeps apart, routing each
/// titled list by its own heading. This is strictly better than running the
/// heading detector over one blob: Lever already knows where its sections
/// begin, so `section_of` only has to classify a heading it was handed.
fn assemble(posting: &Posting) -> Sections {
    let mut sections = Sections::default();
    if let Some(html) = posting.description.as_deref() {
        sections.merge(sections::split_into(html, Section::Overview));
    }
    for list in &posting.lists {
        let section = if list.text.trim().is_empty() {
            Section::Overview
        } else {
            sections::section_of(&list.text)
        };
        sections.merge(sections::split_into(&list.content, section));
    }
    if let Some(html) = posting.additional.as_deref() {
        sections.merge(sections::split_into(html, Section::Overview));
    }
    sections
}

fn salary(range: &SalaryRange) -> Option<pay::Pay> {
    pay::from_parts(
        range.min,
        range.max,
        range.currency.clone(),
        range.interval.clone(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_each_titled_list_by_its_own_heading() {
        let posting = Posting {
            text: "Software Engineer".into(),
            hosted_url: "https://jobs.lever.co/acme/1".into(),
            created_at: Some(1_711_403_416_463),
            categories: Some(Categories { location: Some("Toronto".into()) }),
            description: Some("<p>We build things.</p>".into()),
            lists: vec![
                TitledList { text: "What We Require".into(), content: "<ul><li>Rust</li></ul>".into() },
                TitledList { text: "Responsibilities".into(), content: "<ul><li>Ship</li></ul>".into() },
            ],
            additional: None,
            salary_range: None,
        };
        let item = to_item("Acme", posting);
        assert!(item.requirements.as_deref().unwrap_or_default().contains("Rust"));
        assert!(item.responsibilities.as_deref().unwrap_or_default().contains("Ship"));
        assert!(item.description.as_deref().unwrap_or_default().contains("We build things"));
    }

    #[test]
    fn reads_a_published_salary_range() {
        let range = SalaryRange {
            min: Some(120_000.0),
            max: Some(160_000.0),
            currency: Some("CAD".into()),
            interval: Some("per-year-salary".into()),
        };
        let p = salary(&range).unwrap();
        assert_eq!(p.min, Some(120_000.0));
        assert_eq!(p.period.as_deref(), Some("year"));
        assert_eq!(p.currency.as_deref(), Some("CAD"));
    }

    #[test]
    fn a_missing_created_at_does_not_read_as_fresh() {
        let posting = Posting {
            text: "Engineer".into(),
            hosted_url: "https://jobs.lever.co/acme/2".into(),
            created_at: None,
            categories: None,
            description: None,
            lists: Vec::new(),
            additional: None,
            salary_range: None,
        };
        let item = to_item("Acme", posting);
        assert!(item.timestamp < Utc::now() - chrono::Duration::days(29));
    }
}
