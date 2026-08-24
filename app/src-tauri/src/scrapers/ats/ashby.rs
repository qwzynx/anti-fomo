//! Ashby job boards.
//!
//! Like Lever, the board endpoint returns the whole posting, so descriptions
//! are seeded straight into `job_details` rather than queued for enrichment.
//! Unlike anything else in the pipeline, `?includeCompensation=true` returns a
//! *structured* pay range — `minValue`, `maxValue`, `currencyCode`, `interval`
//! — instead of a sentence to be regexed.

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::Deserialize;

use super::super::employers::{Board, Employer};
use super::super::sections;
use super::{fan_out, seed_detail};
use crate::models::{Item, ItemType};
use crate::pay;
use crate::scrapers::{details, Scraper};

const MAX_PER_EMPLOYER: usize = 400;

pub struct AshbyBoards;

#[derive(Deserialize)]
struct BoardResponse {
    #[serde(default)]
    jobs: Vec<Job>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Job {
    #[serde(default)]
    title: String,
    #[serde(default)]
    job_url: String,
    #[serde(default)]
    location: Option<String>,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    is_listed: Option<bool>,
    #[serde(default)]
    description_html: Option<String>,
    #[serde(default)]
    compensation: Option<Compensation>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Compensation {
    /// The flattened view of the tiers, which is what a reader is shown. Using
    /// it avoids having to decide which of several tiers a posting "means".
    #[serde(default)]
    summary_components: Vec<Component>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Component {
    /// "Salary", "EquityPercentage", "Bonus", … Only salary is pay; an equity
    /// component has no numbers on it and a bonus is not the wage.
    #[serde(default)]
    compensation_type: String,
    /// "1 YEAR", "1 HOUR", "NONE".
    #[serde(default)]
    interval: Option<String>,
    #[serde(default)]
    currency_code: Option<String>,
    #[serde(default)]
    min_value: Option<f64>,
    #[serde(default)]
    max_value: Option<f64>,
}

#[async_trait]
impl Scraper for AshbyBoards {
    fn source_name(&self) -> &'static str {
        "Ashby"
    }

    async fn fetch(&self, client: &reqwest::Client) -> Result<Vec<Item>> {
        Ok(fan_out(
            client,
            |b| matches!(b, Board::Ashby(_)),
            |e, c| Box::pin(fetch_board(e, c)),
        )
        .await)
    }
}

async fn fetch_board(employer: &'static Employer, client: reqwest::Client) -> Result<Vec<Item>> {
    let Board::Ashby(slug) = employer.board else {
        return Ok(Vec::new());
    };
    let url =
        format!("https://api.ashbyhq.com/posting-api/job-board/{slug}?includeCompensation=true");
    let board: BoardResponse = client
        .get(&url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    Ok(board
        .jobs
        .into_iter()
        // `isListed: false` is a posting the employer has taken off its own
        // board. Ranking it would be worse than not having it.
        .filter(|j| j.is_listed.unwrap_or(true))
        .filter(|j| !j.job_url.is_empty() && !j.title.trim().is_empty())
        .take(MAX_PER_EMPLOYER)
        .map(|job| to_item(employer.name, job))
        .collect())
}

fn to_item(company: &str, job: Job) -> Item {
    let timestamp = job
        .published_at
        .as_deref()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.with_timezone(&Utc))
        .unwrap_or_else(|| Utc::now() - chrono::Duration::days(30));

    let content = match job.location.as_deref() {
        Some(l) if !l.trim().is_empty() => format!("{company} · {l}"),
        _ => company.to_string(),
    };

    let mut item = Item::new(
        job.title.trim(),
        "Ashby",
        ItemType::Job,
        job.job_url.clone(),
    )
    .with_company(company)
    .with_location(job.location.clone())
    .with_timestamp(timestamp)
    .with_content(content);

    if let Some(p) = job.compensation.as_ref().and_then(salary) {
        item.salary_min = p.min;
        item.salary_max = p.max;
        item.salary_currency = p.currency.clone();
        item.salary_period = p.period.clone();
    }

    if let Some(html) = job.description_html.as_deref() {
        seed_detail(
            &mut item,
            details::detail_of(sections::split(html), Vec::new()),
        );
    }
    item
}

/// The salary component, if the posting published one. Equity and bonus rows
/// are skipped: what a posting *pays* is the wage, and mixing a bonus range
/// into it would make two postings incomparable.
fn salary(compensation: &Compensation) -> Option<pay::Pay> {
    let component = compensation
        .summary_components
        .iter()
        .find(|c| c.compensation_type.eq_ignore_ascii_case("salary"))?;
    pay::from_parts(
        component.min_value,
        component.max_value,
        component.currency_code.clone(),
        component.interval.clone(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn comp(kind: &str, interval: &str, min: f64, max: f64) -> Component {
        Component {
            compensation_type: kind.into(),
            interval: Some(interval.into()),
            currency_code: Some("USD".into()),
            min_value: Some(min),
            max_value: Some(max),
        }
    }

    #[test]
    fn reads_the_salary_component_and_ignores_equity() {
        let compensation = Compensation {
            summary_components: vec![
                Component {
                    compensation_type: "EquityPercentage".into(),
                    interval: Some("NONE".into()),
                    currency_code: None,
                    min_value: None,
                    max_value: None,
                },
                comp("Salary", "1 YEAR", 211_400.0, 290_600.0),
            ],
        };
        let p = salary(&compensation).unwrap();
        assert_eq!(p.min, Some(211_400.0));
        assert_eq!(p.max, Some(290_600.0));
        assert_eq!(p.period.as_deref(), Some("year"));
    }

    #[test]
    fn an_hourly_interval_survives_normalisation() {
        let compensation = Compensation {
            summary_components: vec![comp("Salary", "1 HOUR", 30.0, 45.0)],
        };
        let p = salary(&compensation).unwrap();
        assert_eq!(p.period.as_deref(), Some("hour"));
        assert_eq!(pay::annual_equivalent(&p), Some(37.5 * 2080.0));
    }

    #[test]
    fn a_posting_with_no_compensation_reports_none() {
        let compensation = Compensation {
            summary_components: Vec::new(),
        };
        assert!(salary(&compensation).is_none());
    }
}
