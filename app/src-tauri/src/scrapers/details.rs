//! Fetching one posting's actual description.
//!
//! Unlike everything else in this module, this is not a [`Scraper`]: it works
//! on URLs rather than sources, and it runs as a second phase after the scrape
//! rather than as part of it.
//!
//! It exists because every opportunity source reads a *list* endpoint, so
//! `content_text` arrives as 24-70 characters of office location. Measured
//! over a full cache, literal skill extraction fired on 4% of postings. With
//! the real requirements text it fires on most of them.
//!
//! Two handlers, chosen by measurement rather than ambition:
//!
//! - **simplify.jobs** — the three GitHub repos hand us a posting id, and
//!   simplify.jobs has already split each posting into `requirements`,
//!   `responsibilities`, tagged `skills` and company `benefits` *as arrays*.
//!   Nobody else's parsing to redo, and it covers ~91-99% of those repos.
//! - **Job Bank Canada** — its own pages, whose sections are already labelled
//!   ("Responsibilities", "Experience and specialization", "Benefits").
//!
//! Deliberately not attempted: Workday (per-tenant POST API, and its pages are
//! JS shells), TikTok/ByteDance (136 KB and 2.2s each), and a generic
//! readability fallback (garbage text would feed garbage into skill matching).
//! Those keep the role-inference behaviour in [`crate::skills::ROLES`].

use scraper::{Html, Selector};
use serde::Deserialize;
use std::sync::LazyLock;

use super::{collapse_ws, strip_html, truncate_words};
use crate::models::{DetailStatus, JobDetail};

/// Cap on any one stored field. Descriptions routinely run past 20 KB, most of
/// it company boilerplate that helps nobody and slows every skill scan.
const MAX_FIELD: usize = 8_000;

/// Which handler, if any, can serve a posting. A pure function of the inputs,
/// so the routing table is testable without touching the network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    /// Fetch `https://simplify.jobs/p/{id}`.
    Simplify(String),
    /// Fetch the Job Bank posting page at this URL.
    JobBank(String),
    /// Nothing here can serve it.
    None,
}

pub fn route(url: &str, simplify_id: Option<&str>) -> Route {
    // The posting id is worth more than the apply link: it resolves to text
    // somebody has already segmented, whatever ATS the apply link points at.
    if let Some(id) = simplify_id.filter(|id| !id.is_empty()) {
        return Route::Simplify(id.to_string());
    }
    if url.contains("jobbank.gc.ca/jobsearch/jobposting") {
        return Route::JobBank(url.to_string());
    }
    Route::None
}

/// Fetches and parses one posting. Never returns an error: a failure is a
/// [`DetailStatus`] to be recorded, because recording it is what stops the URL
/// being retried on every future refresh.
pub async fn fetch_one(client: &reqwest::Client, route: &Route) -> JobDetail {
    match route {
        Route::None => JobDetail::with_status(DetailStatus::Unsupported),
        Route::Simplify(id) => match fetch_simplify(client, id).await {
            Ok(detail) => detail,
            Err(e) => {
                log::debug!("simplify.jobs/{id}: {e}");
                JobDetail::with_status(DetailStatus::Failed)
            }
        },
        Route::JobBank(url) => match fetch_job_bank(client, url).await {
            Ok(detail) => detail,
            Err(e) => {
                log::debug!("job bank {url}: {e}");
                JobDetail::with_status(DetailStatus::Failed)
            }
        },
    }
}

// --- simplify.jobs ---

/// The slice of `__NEXT_DATA__` we care about. Everything is optional: the
/// page is a Next.js dump we do not control, and a missing field must degrade
/// to "no text" rather than failing the parse.
#[derive(Deserialize)]
struct NextData {
    props: Props,
}
#[derive(Deserialize)]
struct Props {
    #[serde(rename = "pageProps")]
    page_props: PageProps,
}
#[derive(Deserialize)]
struct PageProps {
    #[serde(rename = "jobPosting")]
    job_posting: Option<JobPosting>,
}
#[derive(Deserialize)]
struct JobPosting {
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    requirements: Vec<String>,
    #[serde(default)]
    responsibilities: Vec<String>,
    #[serde(default)]
    skills: Vec<SkillTag>,
    #[serde(default)]
    job: Option<Job>,
}
#[derive(Deserialize)]
struct SkillTag {
    #[serde(default)]
    name: Option<String>,
}
#[derive(Deserialize)]
struct Job {
    #[serde(default)]
    company: Option<Company>,
}
#[derive(Deserialize)]
struct Company {
    #[serde(default)]
    benefits: Vec<String>,
}

static NEXT_DATA: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("script#__NEXT_DATA__").unwrap());

async fn fetch_simplify(client: &reqwest::Client, id: &str) -> anyhow::Result<JobDetail> {
    let response = client
        .get(format!("https://simplify.jobs/p/{id}"))
        .send()
        .await?;

    // A 404 is a community-contributed row, not a fault: those ids only exist
    // in the repo. Recording it as unsupported keeps it out of the retry queue
    // and lets role inference handle the posting.
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(JobDetail::with_status(DetailStatus::Unsupported));
    }
    let body = response.error_for_status()?.text().await?;

    let payload = {
        let doc = Html::parse_document(&body);
        let Some(script) = doc.select(&NEXT_DATA).next() else {
            return Ok(JobDetail::with_status(DetailStatus::Empty));
        };
        script.text().collect::<String>()
    };

    Ok(parse_simplify(&payload))
}

/// Split from the fetch so the shape of the payload is testable against a
/// saved fixture rather than the live site.
fn parse_simplify(payload: &str) -> JobDetail {
    let Ok(data) = serde_json::from_str::<NextData>(payload) else {
        return JobDetail::with_status(DetailStatus::Failed);
    };
    let Some(posting) = data.props.page_props.job_posting else {
        return JobDetail::with_status(DetailStatus::Empty);
    };

    let benefits = posting
        .job
        .and_then(|j| j.company)
        .map(|c| c.benefits)
        .unwrap_or_default();

    let mut detail = JobDetail {
        // Already prose; the HTML wrapper is all it needs stripping of.
        description: posting
            .description
            .as_deref()
            .map(strip_html)
            .and_then(non_empty),
        requirements: bullets(&posting.requirements),
        responsibilities: bullets(&posting.responsibilities),
        perks: bullets(&benefits),
        tagged_skills: posting
            .skills
            .into_iter()
            .filter_map(|s| s.name)
            .map(|n| collapse_ws(&n))
            .filter(|n| !n.is_empty())
            .collect(),
        status: DetailStatus::Ok,
    };

    if !detail.has_text() {
        detail.status = DetailStatus::Empty;
    }
    detail
}

/// Joins already-split bullets with newlines, which is how the UI reads them
/// back apart.
fn bullets(lines: &[String]) -> Option<String> {
    let joined = lines
        .iter()
        .map(|l| collapse_ws(l))
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    non_empty(joined).map(|s| truncate_words(&s, MAX_FIELD))
}

fn non_empty(s: String) -> Option<String> {
    (!s.trim().is_empty()).then(|| truncate_words(&s, MAX_FIELD))
}

// --- Job Bank Canada ---

static SECTION: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("div.job-posting-detail-requirements, section").unwrap());
static HEADING: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("h2, h3, h4").unwrap());
static LIST_ITEM: LazyLock<Selector> = LazyLock::new(|| Selector::parse("li, p").unwrap());

async fn fetch_job_bank(client: &reqwest::Client, url: &str) -> anyhow::Result<JobDetail> {
    let body = client.get(url).send().await?.error_for_status()?.text().await?;
    Ok(parse_job_bank(&body))
}

/// Job Bank labels its own sections, so this is heading matching rather than
/// the guesswork a generic page would need.
fn parse_job_bank(html: &str) -> JobDetail {
    let doc = Html::parse_document(html);

    let mut requirements: Vec<String> = Vec::new();
    let mut responsibilities: Vec<String> = Vec::new();
    let mut perks: Vec<String> = Vec::new();

    for section in doc.select(&SECTION) {
        let Some(heading) = section.select(&HEADING).next() else {
            continue;
        };
        let label = collapse_ws(&heading.text().collect::<String>()).to_lowercase();

        let bucket = if label.contains("responsibilit") || label.contains("task") {
            &mut responsibilities
        } else if label.contains("requirement")
            || label.contains("experience and specialization")
            || label.contains("qualification")
            || label.contains("education")
            || label.contains("skill")
        {
            &mut requirements
        } else if label.contains("benefit") || label.contains("perk") {
            &mut perks
        } else {
            continue;
        };

        for node in section.select(&LIST_ITEM) {
            let text = collapse_ws(&node.text().collect::<String>());
            if !text.is_empty() && !bucket.contains(&text) {
                bucket.push(text);
            }
        }
    }

    let mut detail = JobDetail {
        description: None,
        requirements: bullets(&requirements),
        responsibilities: bullets(&responsibilities),
        perks: bullets(&perks),
        tagged_skills: Vec::new(),
        status: DetailStatus::Ok,
    };
    if !detail.has_text() {
        detail.status = DetailStatus::Empty;
    }
    detail
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_posting_id_wins_over_the_apply_link() {
        assert_eq!(
            route("https://job-boards.greenhouse.io/x/jobs/1", Some("abc")),
            Route::Simplify("abc".into())
        );
    }

    #[test]
    fn job_bank_is_routed_by_its_url() {
        assert_eq!(
            route("https://www.jobbank.gc.ca/jobsearch/jobposting/49936682", None),
            Route::JobBank("https://www.jobbank.gc.ca/jobsearch/jobposting/49936682".into())
        );
        // The TFW variant shares the prefix.
        assert!(matches!(
            route("https://www.jobbank.gc.ca/jobsearch/jobpostingtfw/123", None),
            Route::JobBank(_)
        ));
    }

    #[test]
    fn unhandled_hosts_route_nowhere() {
        assert_eq!(route("https://lifeattiktok.com/search/7676", None), Route::None);
        assert_eq!(route("https://x.wd5.myworkdayjobs.com/job/1", None), Route::None);
        // An empty id is not an id.
        assert_eq!(route("https://example.com/j", Some("")), Route::None);
    }

    #[test]
    fn parses_the_simplify_payload() {
        let payload = r#"{"props":{"pageProps":{"jobPosting":{
            "description":"<p>About us</p><div>We build things</div>",
            "requirements":["You are proficient in Python, R, or MATLAB.","Comfortable with statistics."],
            "responsibilities":["Build scalable models."],
            "skills":[{"name":"Python","id":"1"},{"name":"Data Visualization","id":"2"}],
            "job":{"company":{"benefits":["Health Insurance","401(k) Company Match"]}}
        }}}}"#;
        let d = parse_simplify(payload);

        assert_eq!(d.status, DetailStatus::Ok);
        assert_eq!(d.description.as_deref(), Some("About us We build things"));
        assert_eq!(
            d.requirements.as_deref(),
            Some("You are proficient in Python, R, or MATLAB.\nComfortable with statistics.")
        );
        assert_eq!(d.responsibilities.as_deref(), Some("Build scalable models."));
        assert_eq!(d.perks.as_deref(), Some("Health Insurance\n401(k) Company Match"));
        assert_eq!(d.tagged_skills, vec!["Python", "Data Visualization"]);
    }

    #[test]
    fn a_posting_with_no_text_is_empty_not_ok() {
        let payload = r#"{"props":{"pageProps":{"jobPosting":{
            "description":"","requirements":[],"responsibilities":[],"skills":[],"job":null}}}}"#;
        assert_eq!(parse_simplify(payload).status, DetailStatus::Empty);
    }

    #[test]
    fn a_missing_posting_is_empty_and_garbage_is_failed() {
        assert_eq!(
            parse_simplify(r#"{"props":{"pageProps":{"jobPosting":null}}}"#).status,
            DetailStatus::Empty
        );
        assert_eq!(parse_simplify("not json at all").status, DetailStatus::Failed);
    }

    #[test]
    fn parses_job_bank_sections_by_their_headings() {
        let html = r#"
            <section><h2>Responsibilities</h2><h3>Tasks</h3>
              <ul><li>Write, modify and test software code</li>
                  <li>Maintain existing programs</li></ul></section>
            <section><h2>Experience and specialization</h2>
              <ul><li>C++ Java JavaScript Python</li></ul></section>
            <section><h2>Benefits</h2><ul><li>Free parking available</li></ul></section>
            <section><h2>How to apply</h2><ul><li>By email</li></ul></section>
        "#;
        let d = parse_job_bank(html);

        assert_eq!(d.status, DetailStatus::Ok);
        assert!(d.responsibilities.as_deref().unwrap().contains("Write, modify and test"));
        assert!(d.requirements.as_deref().unwrap().contains("C++ Java JavaScript Python"));
        assert_eq!(d.perks.as_deref(), Some("Free parking available"));
        // An unrecognised section contributes nothing.
        assert!(!d.responsibilities.as_deref().unwrap().contains("By email"));
    }

    #[test]
    fn a_job_bank_page_with_no_known_sections_is_empty() {
        assert_eq!(
            parse_job_bank("<section><h2>How to apply</h2><p>By email</p></section>").status,
            DetailStatus::Empty
        );
    }

    #[test]
    fn fields_are_truncated_on_a_word_boundary() {
        let long = "word ".repeat(4_000);
        let out = bullets(&[long]).unwrap();
        assert!(out.len() <= MAX_FIELD);
        assert!(out.ends_with("word"), "cut mid-word: {:?}", &out[out.len() - 20..]);
    }
}
