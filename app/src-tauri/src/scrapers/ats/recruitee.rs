//! Recruitee career sites.
//!
//! `https://{slug}.recruitee.com/api/offers/` returns every open offer with
//! its full description and requirements, an `experience_code`, a salary range
//! where one is published and — unusually — `close_at`, a real application
//! deadline. Like Lever, the text arrives with the list, so it is seeded into
//! `job_details` and costs the enrichment pass nothing.

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use serde::Deserialize;

use super::super::employers::{Board, Employer};
use super::super::sections::{self, Section, Sections};
use super::{fan_out, keep_best, seed_detail};
use crate::models::{Item, ItemType};
use crate::pay;
use crate::scrapers::{details, Scraper};

const MAX_PER_EMPLOYER: usize = 300;

pub struct RecruiteeBoards;

#[derive(Deserialize)]
struct Offers {
    #[serde(default)]
    offers: Vec<Offer>,
}

#[derive(Deserialize)]
struct Offer {
    #[serde(default)]
    title: String,
    #[serde(default)]
    careers_url: String,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    close_at: Option<String>,
    #[serde(default)]
    location: Option<String>,
    #[serde(default)]
    remote: bool,
    #[serde(default)]
    employment_type_code: Option<String>,
    #[serde(default)]
    experience_code: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    requirements: Option<String>,
    #[serde(default)]
    salary: Option<Salary>,
}

/// A number that some boards send as a string ("85000").
fn lenient_number<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<f64>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Number {
        Float(f64),
        Text(String),
    }
    Ok(match Option::<Number>::deserialize(d)? {
        Some(Number::Float(n)) => Some(n),
        Some(Number::Text(t)) => t.trim().replace(',', "").parse().ok(),
        None => None,
    })
}

#[derive(Deserialize)]
struct Salary {
    #[serde(default, deserialize_with = "lenient_number")]
    min: Option<f64>,
    #[serde(default, deserialize_with = "lenient_number")]
    max: Option<f64>,
    #[serde(default)]
    currency: Option<String>,
    #[serde(default)]
    period: Option<String>,
}

#[async_trait]
impl Scraper for RecruiteeBoards {
    fn source_name(&self) -> &'static str {
        "Recruitee"
    }

    async fn fetch(&self, client: &reqwest::Client) -> Result<Vec<Item>> {
        crate::scrapers::collect_batches(self, client).await
    }

    async fn fetch_batches(
        &self,
        client: &reqwest::Client,
        sink: crate::scrapers::Sink<'_>,
    ) -> Result<()> {
        fan_out(
            client,
            |b| matches!(b, Board::Recruitee(_)),
            |e, c| Box::pin(fetch_board(e, c)),
            super::RECRUITEE_CONCURRENCY,
            sink,
        )
        .await;
        Ok(())
    }
}

pub async fn fetch_board(
    employer: &'static Employer,
    client: reqwest::Client,
) -> Result<Vec<Item>> {
    let Board::Recruitee(slug) = employer.board else {
        return Ok(Vec::new());
    };
    let url = format!("https://{slug}.recruitee.com/api/offers/");
    let offers: Offers = super::get_json(&client, &url).await?;
    let items = offers
        .offers
        .into_iter()
        .filter(|o| !o.careers_url.is_empty() && !o.title.trim().is_empty())
        .filter(|o| o.status.as_deref().is_none_or(|s| s == "published"))
        .map(|o| to_item(employer.name, o))
        .collect();
    Ok(keep_best(items, MAX_PER_EMPLOYER))
}

/// `2026-11-12 19:04:13 UTC`, which is not RFC 3339.
fn parse_when(raw: &str) -> Option<DateTime<Utc>> {
    let trimmed = raw.trim().trim_end_matches(" UTC");
    if let Ok(full) = DateTime::parse_from_rfc3339(trimmed) {
        return Some(full.with_timezone(&Utc));
    }
    let naive = NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%d %H:%M:%S").ok()?;
    Utc.from_local_datetime(&naive).single()
}

fn to_item(company: &str, offer: Offer) -> Item {
    let location = offer
        .location
        .clone()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            if offer.remote {
                format!("{l} | Remote")
            } else {
                l
            }
        })
        .or_else(|| offer.remote.then(|| "Remote".to_string()));
    let timestamp = offer
        .published_at
        .as_deref()
        .and_then(parse_when)
        .unwrap_or_else(|| Utc::now() - chrono::Duration::days(30));
    let content = match &location {
        Some(l) => format!("{company} · {l}"),
        None => company.to_string(),
    };
    let intern = offer.employment_type_code.as_deref() == Some("internship")
        || offer.experience_code.as_deref() == Some("internship");

    let mut item = Item::new(
        offer.title.trim(),
        "Recruitee",
        if intern {
            ItemType::Internship
        } else {
            ItemType::Job
        },
        offer.careers_url.clone(),
    )
    .with_company(company)
    .with_location(location)
    .with_timestamp(timestamp)
    .with_content(content)
    .with_closes_at(offer.close_at.as_deref().and_then(parse_when));

    if let Some(range) = offer
        .salary
        .as_ref()
        .and_then(|s| pay::from_parts(s.min, s.max, s.currency.clone(), s.period.clone()))
    {
        item.salary_min = range.min;
        item.salary_max = range.max;
        item.salary_currency = range.currency.clone();
        item.salary_period = range.period.clone();
    }

    let mut parts = Sections::default();
    if let Some(html) = offer.description.as_deref() {
        parts.merge(sections::split_into(html, Section::Overview));
    }
    if let Some(html) = offer.requirements.as_deref() {
        parts.merge(sections::split_into(html, Section::Requirements));
    }
    seed_detail(&mut item, details::detail_of(parts, Vec::new()));
    item
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offer() -> Offer {
        Offer {
            title: "Junior Developer".into(),
            careers_url: "https://acme.recruitee.com/o/junior-developer".into(),
            status: Some("published".into()),
            published_at: Some("2026-11-12 19:04:13 UTC".into()),
            close_at: Some("2026-12-01 00:00:00 UTC".into()),
            location: Some("Toronto, Ontario, Canada".into()),
            remote: false,
            employment_type_code: Some("internship".into()),
            experience_code: Some("entry_level".into()),
            description: Some("<p>We build things.</p>".into()),
            requirements: Some("<ul><li>Know Rust</li></ul>".into()),
            salary: None,
        }
    }

    #[test]
    fn a_recruitee_offer_carries_its_deadline_text_and_internship_flag() {
        let item = to_item("Acme", offer());
        assert_eq!(item.item_type, ItemType::Internship);
        assert_eq!(
            item.closes_at.map(|d| d.format("%Y-%m-%d").to_string()),
            Some("2026-12-01".into())
        );
        assert!(item
            .requirements
            .as_deref()
            .is_some_and(|r| r.contains("Rust")));
        assert_eq!(item.timestamp.format("%Y-%m-%d").to_string(), "2026-11-12");
    }

    #[test]
    fn a_salary_sent_as_text_is_still_a_salary() {
        let salary: Salary = serde_json::from_str(
            r#"{"min":"85,000","max":95000,"currency":"EUR","period":"year"}"#,
        )
        .unwrap();
        assert_eq!((salary.min, salary.max), (Some(85_000.0), Some(95_000.0)));
    }

    #[test]
    fn the_odd_utc_suffix_is_read() {
        assert!(parse_when("2026-11-12 19:04:13 UTC").is_some());
        assert!(parse_when("2026-11-12T19:04:13Z").is_some());
        assert!(parse_when("garbage").is_none());
    }
}
