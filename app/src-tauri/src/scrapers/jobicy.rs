//! Jobicy — remote roles, through a public key-less API that filters by the
//! country a role is open to. Read for Canada: these are the remote postings
//! a Canadian can actually be hired into, which is a different list from
//! "remote" on a US board, where most of them mean "remote within the US".
//!
//! Measured: Canada's software engineering list is a few dozen roles and its
//! data science list a handful, each well inside one page of fifty, with pay
//! published in CAD on most of them and the whole description in the payload.
//! The walk still follows `nextCursor`, so a list that outgrows a page is not
//! silently cut off.
//!
//! Jobicy asks that it be credited with a link and that applications go to the
//! job's own URL. The item's URL *is* that URL and the source name is shown on
//! the card.

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::collections::HashSet;

use super::sections::{self, Section, Sections};
use super::{details, Scraper};
use crate::models::{Item, ItemType};
use crate::rank::{classify_seniority, DEFAULT_MAJOR};

pub struct Jobicy;

pub const NAME: &str = "Jobicy";

const API: &str = "https://jobicy.com/api/v2/remote-jobs";
const GEO: &str = "canada";
/// (industry, the discipline it implies). Jobicy's "engineering" is software
/// engineering — its own label for it on every row is "Software Engineering".
const INDUSTRIES: &[(&str, Option<&str>)] =
    &[("engineering", Some(DEFAULT_MAJOR)), ("data-science", None)];
/// The API's largest page.
const COUNT: usize = 50;
/// Pages per industry. One is what the lists measure today.
const MAX_PAGES: usize = 4;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Response {
    #[serde(default)]
    jobs: Vec<Job>,
    #[serde(default)]
    has_more: bool,
    #[serde(default)]
    next_cursor: Option<String>,
}

/// Field types vary from row to row — the salaries arrive as numbers on one
/// posting and strings on the next, or null — so those are read loosely.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Job {
    #[serde(default)]
    url: String,
    #[serde(default)]
    job_title: String,
    #[serde(default)]
    company_name: String,
    /// "Canada", "Anywhere", or a list: "EMEA,  LATAM,  Canada,  USA".
    #[serde(default)]
    job_geo: String,
    /// "Senior", "Entry-Level, Junior", "Any", …
    #[serde(default)]
    job_level: String,
    /// A Python list printed as a string: "['Full-Time']".
    #[serde(default)]
    job_type: serde_json::Value,
    #[serde(default)]
    job_description: Option<String>,
    #[serde(default)]
    pub_date: Option<String>,
    #[serde(default)]
    salary_min: serde_json::Value,
    #[serde(default)]
    salary_max: serde_json::Value,
    #[serde(default)]
    salary_currency: Option<String>,
    #[serde(default)]
    salary_period: Option<String>,
}

#[async_trait]
impl Scraper for Jobicy {
    fn source_name(&self) -> &'static str {
        NAME
    }

    async fn fetch(&self, client: &reqwest::Client) -> Result<Vec<Item>> {
        let lists = INDUSTRIES
            .iter()
            .map(|(industry, discipline)| list(client, industry, *discipline));
        let mut failures = 0;
        let mut last_error = None;
        let mut seen = HashSet::new();
        let mut items = Vec::new();
        for result in futures::future::join_all(lists).await {
            match result {
                Ok(rows) => items.extend(rows.into_iter().filter(|i| seen.insert(i.url.clone()))),
                Err(e) => {
                    failures += 1;
                    last_error = Some(e);
                }
            }
        }
        if failures == INDUSTRIES.len() {
            return Err(last_error.unwrap_or_else(|| anyhow::anyhow!("no list answered")));
        }
        Ok(items)
    }
}

/// One industry's list, following the cursor until it says there is no more.
async fn list(
    client: &reqwest::Client,
    industry: &str,
    discipline: Option<&'static str>,
) -> Result<Vec<Item>> {
    let mut items = Vec::new();
    let mut cursor: Option<String> = None;
    for page in 0..MAX_PAGES {
        let count = COUNT.to_string();
        let mut params = vec![
            ("count", count.as_str()),
            ("geo", GEO),
            ("industry", industry),
        ];
        if let Some(c) = cursor.as_deref() {
            params.push(("cursor", c));
        }
        let url = reqwest::Url::parse_with_params(API, &params)?;
        let body: Response = match client
            .get(url)
            .send()
            .await
            .and_then(|r| r.error_for_status())
        {
            Ok(r) => r.json().await?,
            Err(e) if page > 0 => {
                log::warn!("{NAME} '{industry}' page {page}: {e}");
                break;
            }
            Err(e) => return Err(e.into()),
        };
        items.extend(body.jobs.into_iter().filter_map(|j| to_item(j, discipline)));
        match body.next_cursor.filter(|c| body.has_more && !c.is_empty()) {
            Some(next) => cursor = Some(next),
            None => break,
        }
    }
    Ok(items)
}

/// A number that may have arrived as a string.
fn number(value: &serde_json::Value) -> Option<f64> {
    match value {
        serde_json::Value::Number(n) => n.as_f64(),
        serde_json::Value::String(s) => s.trim().replace(',', "").parse().ok(),
        _ => None,
    }
}

/// Where a role is open to, readably. Canada is put first when it is on the
/// list — that is why the row is here, and the location tagger has to see it —
/// and a list of a dozen regions is cut to three.
fn location_of(geo: &str) -> String {
    let mut places: Vec<&str> = geo
        .split(',')
        .map(str::trim)
        .filter(|p| !p.is_empty() && !p.eq_ignore_ascii_case("anywhere"))
        .collect();
    if let Some(at) = places.iter().position(|p| p.eq_ignore_ascii_case("canada")) {
        let canada = places.remove(at);
        places.insert(0, canada);
    }
    places.truncate(3);
    if places.is_empty() {
        "Remote".to_string()
    } else {
        format!("{} | Remote", places.join(", "))
    }
}

/// The level from Jobicy's own label, for a title that does not say.
fn level_of(label: &str) -> Option<String> {
    let label = label.to_ascii_lowercase();
    let level = if label.contains("intern") {
        "Intern"
    } else if label.contains("entry") || label.contains("junior") {
        "Junior"
    } else if label.contains("director") || label.contains("executive") {
        "Lead"
    } else if label.contains("senior") {
        "Senior"
    } else {
        return None;
    };
    Some(level.to_string())
}

fn to_item(job: Job, discipline: Option<&'static str>) -> Option<Item> {
    let title = job.job_title.trim();
    if title.is_empty() || job.url.is_empty() {
        return None;
    }
    let location = location_of(&job.job_geo);
    let timestamp = job
        .pub_date
        .as_deref()
        .and_then(|d| DateTime::parse_from_rfc3339(d).ok())
        .map(|d| d.with_timezone(&Utc))
        .unwrap_or_else(|| Utc::now() - chrono::Duration::days(30));
    let seniority = classify_seniority(title).or_else(|| level_of(&job.job_level));
    let internship = seniority.as_deref() == Some("Intern")
        || job
            .job_type
            .to_string()
            .to_ascii_lowercase()
            .contains("intern");

    let company = job.company_name.trim();
    let mut item = Item::new(
        title,
        NAME,
        if internship {
            ItemType::Internship
        } else {
            ItemType::Job
        },
        job.url.clone(),
    )
    .with_company(company)
    .with_location(Some(location.clone()))
    .with_timestamp(timestamp)
    .with_content(format!("{company} · {location}"));
    item.discipline = discipline.map(str::to_string);
    item.seniority = seniority;

    if let Some(range) = crate::pay::from_parts(
        number(&job.salary_min),
        number(&job.salary_max),
        job.salary_currency.clone().filter(|c| !c.is_empty()),
        job.salary_period.clone(),
    ) {
        item.salary_min = range.min;
        item.salary_max = range.max;
        item.salary_currency = range.currency;
        item.salary_period = range.period;
    }
    if let Some(html) = job.job_description.as_deref() {
        let mut parts = Sections::default();
        parts.merge(sections::split_into(html, Section::Overview));
        super::ats::seed_detail(&mut item, details::detail_of(parts, Vec::new()));
    }
    Some(item)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(json: &str) -> Job {
        serde_json::from_str(json).unwrap()
    }

    const ROW: &str = r#"{"id":"162522","url":"https://jobicy.com/jobs/162522-senior-software-engineer",
        "jobTitle":"Senior Software Engineer, Services","companyName":"Rush Street Interactive",
        "jobIndustry":"['Software Engineering']","jobType":"['Full-Time']",
        "jobGeo":"EMEA,  LATAM,  Canada,  USA","jobLevel":"Senior",
        "jobDescription":"<p>About us.</p><h3>Requirements</h3><ul><li>5 years of Go</li></ul>",
        "pubDate":"2026-11-16T09:33:46+00:00","salaryMin":"124000","salaryMax":186000,
        "salaryCurrency":"CAD","salaryPeriod":"yearly"}"#;

    #[test]
    fn a_row_keeps_its_link_its_pay_and_its_text() {
        let item = to_item(job(ROW), Some(DEFAULT_MAJOR)).unwrap();
        assert_eq!(
            item.url,
            "https://jobicy.com/jobs/162522-senior-software-engineer"
        );
        assert_eq!(item.company.as_deref(), Some("Rush Street Interactive"));
        assert_eq!(
            item.salary_min,
            Some(124_000.0),
            "a salary sent as a string"
        );
        assert_eq!(item.salary_max, Some(186_000.0), "and one sent as a number");
        assert_eq!(item.salary_currency.as_deref(), Some("CAD"));
        assert_eq!(item.salary_period.as_deref(), Some("year"));
        assert_eq!(item.item_type, ItemType::Job);
        assert!(item.requirements.is_some() || item.description.is_some());
        assert_eq!(item.discipline.as_deref(), Some(DEFAULT_MAJOR));
    }

    #[test]
    fn canada_leads_the_location_so_it_is_tagged() {
        assert_eq!(
            location_of("APAC,  EMEA,  LATAM,  Canada,  USA"),
            "Canada, APAC, EMEA | Remote"
        );
        assert_eq!(location_of("Anywhere"), "Remote");
        assert_eq!(location_of("Canada"), "Canada | Remote");
        let tags = crate::location::normalize_location(Some(&location_of(
            "APAC, EMEA, LATAM, Canada, USA",
        )));
        assert!(tags.iter().any(|t| t == "Canada"), "{tags:?}");
    }

    #[test]
    fn missing_pay_is_none_rather_than_zero() {
        let row = ROW
            .replace(r#""salaryMin":"124000""#, r#""salaryMin":null"#)
            .replace(r#""salaryMax":186000"#, r#""salaryMax":null"#);
        let item = to_item(job(&row), None).unwrap();
        assert_eq!(item.salary_min, None);
        assert_eq!(item.salary_max, None);
    }

    #[test]
    fn the_level_label_fills_in_for_a_silent_title() {
        let row = ROW
            .replace("Senior Software Engineer, Services", "Software Engineer")
            .replace(
                r#""jobLevel":"Senior""#,
                r#""jobLevel":"Entry-Level, Junior""#,
            );
        let item = to_item(job(&row), None).unwrap();
        assert_eq!(item.seniority.as_deref(), Some("Junior"));
    }
}
