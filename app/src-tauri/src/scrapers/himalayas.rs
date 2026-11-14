//! Himalayas — a remote-jobs board with a public, key-less search API.
//!
//! Searched, not drained: the board holds well over a hundred thousand roles
//! and this app wants the early-career corner of it. So the source is a handful
//! of queries (intern, co-op, new grad, junior) crossed with the two countries
//! this reader works in, each narrowed to entry-level, a few pages deep.
//!
//! Himalayas asks for attribution and that `applicationLink` be kept as given,
//! so the item's URL *is* that link and the source name is shown on the card.
//! Without an API key the limit is 60 requests a minute; a run here is about
//! twenty.

use anyhow::Result;
use async_trait::async_trait;
use chrono::{TimeZone, Utc};
use futures::future::join_all;
use serde::Deserialize;
use std::collections::HashSet;

use super::sections::{self, Section, Sections};
use super::{details, Scraper};
use crate::models::{Item, ItemType};
use crate::pay;

pub struct Himalayas;

/// (query, country). `country` is an ISO code the API filters on.
const QUERIES: &[(&str, &str)] = &[
    ("intern", "CA"),
    ("co-op", "CA"),
    ("student", "CA"),
    ("junior developer", "CA"),
    ("software intern", "US"),
    ("new grad", "US"),
    ("junior developer", "US"),
];

/// A page is 20. Past three pages the same query is mostly roles that are not
/// early-career at all.
const MAX_PAGES: usize = 3;
const PAGE: usize = 20;

#[derive(Deserialize)]
struct Response {
    #[serde(default)]
    jobs: Vec<Job>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Job {
    #[serde(default)]
    title: String,
    #[serde(default)]
    company_name: String,
    #[serde(default)]
    employment_type: Option<String>,
    #[serde(default)]
    min_salary: Option<f64>,
    #[serde(default)]
    max_salary: Option<f64>,
    #[serde(default)]
    salary_period: Option<String>,
    #[serde(default)]
    currency: Option<String>,
    #[serde(default)]
    location_restrictions: Vec<String>,
    #[serde(default)]
    pub_date: Option<i64>,
    #[serde(default)]
    expiry_date: Option<i64>,
    #[serde(default)]
    application_link: String,
    #[serde(default)]
    guid: String,
    #[serde(default)]
    description: Option<String>,
}

#[async_trait]
impl Scraper for Himalayas {
    fn source_name(&self) -> &'static str {
        "Himalayas"
    }

    async fn fetch(&self, client: &reqwest::Client) -> Result<Vec<Item>> {
        let searches = QUERIES
            .iter()
            .map(|(query, country)| search(client, query, country));
        let results = join_all(searches).await;

        // One query failing is thin coverage, not a broken source; every one
        // failing is the network, and says so.
        let mut failures = 0;
        let mut seen = HashSet::new();
        let mut items = Vec::new();
        let mut last_error = None;
        for result in results {
            match result {
                Ok(jobs) => {
                    for job in jobs {
                        let key = if job.guid.is_empty() {
                            &job.application_link
                        } else {
                            &job.guid
                        };
                        if seen.insert(key.clone()) {
                            items.extend(to_item(job));
                        }
                    }
                }
                Err(e) => {
                    failures += 1;
                    last_error = Some(e);
                }
            }
        }
        if failures == QUERIES.len() {
            return Err(last_error.unwrap_or_else(|| anyhow::anyhow!("no query answered")));
        }
        Ok(items)
    }
}

/// One query, paged until a short page.
async fn search(client: &reqwest::Client, query: &str, country: &str) -> Result<Vec<Job>> {
    let mut all = Vec::new();
    for page in 1..=MAX_PAGES {
        let url = reqwest::Url::parse_with_params(
            "https://himalayas.app/jobs/api/search",
            [
                ("q", query),
                ("country", country),
                ("seniority", "Entry-level"),
                ("page", page.to_string().as_str()),
            ],
        )?;
        let response = client
            .get(url)
            .send()
            .await
            .and_then(|r| r.error_for_status());
        let body: Response = match response {
            Ok(r) => r.json().await?,
            // A failing page after the first ends the walk with what it has.
            Err(e) if page > 1 => {
                log::warn!("Himalayas '{query}' page {page}: {e}");
                break;
            }
            Err(e) => return Err(e.into()),
        };
        let count = body.jobs.len();
        all.extend(body.jobs);
        if count < PAGE {
            break;
        }
    }
    Ok(all)
}

fn period_of(raw: &str) -> Option<String> {
    Some(
        match raw.to_ascii_lowercase().as_str() {
            "annual" | "yearly" | "year" => "year",
            "monthly" | "month" => "month",
            "weekly" | "week" => "week",
            "daily" | "day" => "day",
            "hourly" | "hour" => "hour",
            _ => return None,
        }
        .to_string(),
    )
}

fn to_item(job: Job) -> Option<Item> {
    if job.title.trim().is_empty() || job.application_link.is_empty() {
        return None;
    }
    // A job open to forty countries is a list, not a place.
    let place: Vec<&str> = job
        .location_restrictions
        .iter()
        .map(String::as_str)
        .take(3)
        .collect();
    let location = if place.is_empty() {
        "Remote".to_string()
    } else {
        format!("{} | Remote", place.join(", "))
    };
    let timestamp = job
        .pub_date
        .and_then(|s| Utc.timestamp_opt(s, 0).single())
        .unwrap_or_else(|| Utc::now() - chrono::Duration::days(30));
    let intern = job
        .employment_type
        .as_deref()
        .is_some_and(|t| t.to_ascii_lowercase().contains("intern"));

    let mut item = Item::new(
        job.title.trim(),
        "Himalayas",
        if intern {
            ItemType::Internship
        } else {
            ItemType::Job
        },
        job.application_link.clone(),
    )
    .with_company(job.company_name.trim())
    .with_location(Some(location.clone()))
    .with_timestamp(timestamp)
    .with_content(format!("{} · {location}", job.company_name.trim()))
    .with_closes_at(
        job.expiry_date
            .and_then(|s| Utc.timestamp_opt(s, 0).single()),
    );

    if let Some(range) = pay::from_parts(
        job.min_salary,
        job.max_salary,
        job.currency.clone(),
        job.salary_period.as_deref().and_then(period_of),
    ) {
        item.salary_min = range.min;
        item.salary_max = range.max;
        item.salary_currency = range.currency.clone();
        item.salary_period = range.period.clone();
    }
    if let Some(html) = job.description.as_deref() {
        let mut parts = Sections::default();
        parts.merge(sections::split_into(html, Section::Overview));
        super::ats::seed_detail(&mut item, details::detail_of(parts, Vec::new()));
    }
    Some(item)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job() -> Job {
        serde_json::from_str(
            r#"{"title":".Net Software Developer (Internship)","companyName":"One Human Service Network",
                "employmentType":"Intern","minSalary":null,"maxSalary":null,"salaryPeriod":"annual",
                "locationRestrictions":["Canada"],"pubDate":1793537413,"expiryDate":1795379430,
                "applicationLink":"https://himalayas.app/companies/x/jobs/net-sof","guid":"g1",
                "description":"<h3>Key responsibilities</h3><p>Build software.</p>"}"#,
        )
        .unwrap()
    }

    #[test]
    fn an_intern_posting_keeps_its_link_its_deadline_and_its_text() {
        let item = to_item(job()).unwrap();
        assert_eq!(item.item_type, ItemType::Internship);
        assert_eq!(item.url, "https://himalayas.app/companies/x/jobs/net-sof");
        assert!(item.closes_at.is_some_and(|c| c > item.timestamp));
        assert_eq!(item.location.as_deref(), Some("Canada | Remote"));
        assert!(item.responsibilities.is_some() || item.description.is_some());
    }

    #[test]
    fn periods_are_the_ones_the_pay_module_understands() {
        assert_eq!(period_of("annual").as_deref(), Some("year"));
        assert_eq!(period_of("hourly").as_deref(), Some("hour"));
        assert_eq!(period_of("fortnightly"), None);
    }

    #[test]
    fn a_posting_open_to_many_countries_is_not_a_wall_of_them() {
        let mut j = job();
        j.location_restrictions = (0..40).map(|i| format!("Country {i}")).collect();
        let item = to_item(j).unwrap();
        assert_eq!(item.location.as_deref().unwrap().matches(',').count(), 2);
    }
}
