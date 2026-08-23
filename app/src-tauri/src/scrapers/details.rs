//! Fetching one posting's actual description from the employer's own site.
//!
//! Unlike everything else in this module, this is not a [`Scraper`]: it works
//! on URLs rather than sources, and it runs as a second phase after the scrape
//! rather than as part of it.
//!
//! It exists because every opportunity source reads a *list* endpoint, so
//! `content_text` arrives as 24-70 characters of office location. What the
//! posting requires, what the job actually involves and what it pays for are
//! all on the posting's own page, and nowhere else.
//!
//! # How a posting is served
//!
//! [`route`] returns an ordered chain rather than one handler, and [`fetch`]
//! walks it until something comes back with text. The order is the same
//! judgement every time: the employer's own words first, a mirror last.
//!
//! 1. **The applicant-tracking system's own API.** Six of them cover 47% of a
//!    real cache between them — Workday alone is 31% — and each returns JSON
//!    on a documented URL derived from the posting link. Three of the six
//!    (Lever, SmartRecruiters, Workable) have already labelled their sections,
//!    which is better than any heading guess.
//! 2. **The page's schema.org metadata** ([`super::jsonld`]), which catches
//!    the iCIMS family, Ashby, Microsoft and the long tail of career sites.
//! 3. **simplify.jobs**, for the postings the three GitHub repos gave us an id
//!    for. It is a mirror of the same posting, so it goes last — but it is a
//!    good one, and it is why coverage does not fall off a cliff on the
//!    employers nobody has written a handler for.
//!
//! A handler that returns only unsegmented prose is not the end of the walk:
//! if simplify.jobs is still in the chain it is tried too and merged in, since
//! its pre-split requirements are exactly what the prose was missing.
//!
//! Deliberately not attempted: a readability-style "grab the biggest block of
//! text" fallback. Garbage text feeds garbage into skill matching, and a
//! posting with no skills listed is a better answer than one credited with the
//! words in a site's navigation menu.

use scraper::{Html, Selector};
use serde::Deserialize;
use std::sync::LazyLock;

use super::sections::{self, Section, Sections};
use super::{collapse_ws, jsonld, truncate_words};
use crate::models::{DetailStatus, JobDetail};

/// Cap on any one stored field. Descriptions routinely run past 20 KB, most of
/// it company boilerplate that helps nobody and slows every skill scan.
const MAX_FIELD: usize = 8_000;

/// Cap on a page read for its metadata. The largest career page measured is
/// 692 KB of framework payload wrapped around a 10 KB posting; this only ever
/// bites on something pathological, and cutting an HTML document short costs
/// at worst the `<script>` tag we were looking for.
const MAX_PAGE: usize = 2 * 1024 * 1024;

/// One way of getting at a posting. Each variant carries what its URL needs,
/// parsed out of the posting link, so routing stays a pure function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Handler {
    Greenhouse { board: String, id: String },
    Lever { org: String, id: String },
    SmartRecruiters { company: String, id: String },
    /// Workday's `cxs` endpoint, which serves the JSON its own single-page app
    /// reads. The host varies per tenant, hence carrying it.
    Workday { host: String, tenant: String, site: String, path: String },
    Workable { account: String, shortcode: String },
    Eightfold { host: String, id: String },
    JobBank(String),
    /// Fetch the posting page and read its schema.org `JobPosting`.
    Page(String),
    /// Fetch `https://simplify.jobs/p/{id}`.
    Simplify(String),
}

impl Handler {
    /// Short label for logs and the coverage check.
    pub fn name(&self) -> &'static str {
        match self {
            Handler::Greenhouse { .. } => "greenhouse",
            Handler::Lever { .. } => "lever",
            Handler::SmartRecruiters { .. } => "smartrecruiters",
            Handler::Workday { .. } => "workday",
            Handler::Workable { .. } => "workable",
            Handler::Eightfold { .. } => "eightfold",
            Handler::JobBank(_) => "job bank",
            Handler::Page(_) => "schema.org",
            Handler::Simplify(_) => "simplify.jobs",
        }
    }
}

/// The chain to try for one posting, best source first. Empty means nothing
/// here can serve it, which the caller records so the URL is never retried.
pub fn route(url: &str, simplify_id: Option<&str>) -> Vec<Handler> {
    let mut chain = Vec::new();
    if let Some(handler) = ats_handler(url) {
        chain.push(handler);
    }
    if page_worth_reading(url) {
        chain.push(Handler::Page(url.to_string()));
    }
    if let Some(id) = simplify_id.filter(|id| !id.is_empty()) {
        chain.push(Handler::Simplify(id.to_string()));
    }
    chain
}

/// The dedicated handler for the applicant-tracking system this link points
/// at, if we have one and the link is a posting rather than a search page.
fn ats_handler(url: &str) -> Option<Handler> {
    let (host, path) = split_url(url)?;
    let segments: Vec<&str> = path
        .split('?')
        .next()
        .unwrap_or_default()
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();

    if host.ends_with("greenhouse.io") {
        // .../{board}/jobs/{id}
        let at = segments.iter().position(|s| *s == "jobs")?;
        return Some(Handler::Greenhouse {
            board: segments.get(at.checked_sub(1)?)?.to_string(),
            id: segments.get(at + 1)?.to_string(),
        });
    }
    if host == "jobs.lever.co" {
        // /{org}/{id}[/apply]
        return Some(Handler::Lever {
            org: segments.first()?.to_string(),
            id: segments.get(1)?.to_string(),
        });
    }
    if host == "jobs.smartrecruiters.com" {
        // /{company}/{numeric id}
        let id = segments.get(1)?;
        return id
            .chars()
            .all(|c| c.is_ascii_digit())
            .then(|| Handler::SmartRecruiters {
                company: segments[0].to_string(),
                id: id.to_string(),
            });
    }
    if host.ends_with(".myworkdayjobs.com") || host.ends_with(".myworkdaysite.com") {
        return workday(host, &segments);
    }
    if host == "apply.workable.com" {
        // /{account}/j/{shortcode}[/apply]
        let at = segments.iter().position(|s| *s == "j")?;
        return Some(Handler::Workable {
            account: segments.first()?.to_string(),
            shortcode: segments.get(at + 1)?.to_string(),
        });
    }
    if host.ends_with(".eightfold.ai") {
        // /careers/job/{id}
        let at = segments.iter().position(|s| *s == "job")?;
        return Some(Handler::Eightfold {
            host: host.to_string(),
            id: segments.get(at + 1)?.to_string(),
        });
    }
    if host.ends_with("jobbank.gc.ca") && path.contains("/jobsearch/jobposting") {
        return Some(Handler::JobBank(url.to_string()));
    }
    None
}

/// Workday comes in two shapes, one per hosting arrangement:
///
/// - `{tenant}.wd5.myworkdayjobs.com/[{locale}/]{site}/job/{path}`
/// - `wd1.myworkdaysite.com/recruiting/{tenant}/{site}/job/{path}`
///
/// Both resolve to `https://{host}/wday/cxs/{tenant}/{site}/job/{path}`. The
/// optional locale segment has to go: asking the API for `fr-CA` as the site
/// is a 404, and leaving it in is how a Canadian posting came back in French.
fn workday(host: &str, segments: &[&str]) -> Option<Handler> {
    let at = segments.iter().position(|s| *s == "job")?;
    let path = segments[at + 1..].join("/");
    if path.is_empty() {
        return None;
    }

    let (tenant, site) = if segments.first() == Some(&"recruiting") {
        (segments.get(1)?.to_string(), segments.get(2)?.to_string())
    } else {
        let site = segments.get(at.checked_sub(1)?)?;
        (host.split('.').next()?.to_string(), site.to_string())
    };
    Some(Handler::Workday {
        host: host.to_string(),
        tenant,
        site,
        path,
    })
}

/// Hosts measured to carry no schema.org `JobPosting`, so fetching the page
/// for one would spend a request to learn nothing. Two kinds: sites we already
/// have an API handler for, where the page adds nothing the API did not give
/// us, and single-page apps that render the posting entirely client-side.
///
/// Anything not listed gets one attempt, once — the result is recorded either
/// way, so an employer we guessed wrong about costs a single request in the
/// lifetime of that posting.
const NO_METADATA: &[&str] = &[
    "greenhouse.io",
    "jobs.lever.co",
    "smartrecruiters.com",
    "apply.workable.com",
    "eightfold.ai",
    "jobbank.gc.ca",
    "lifeattiktok.com",
    "bytedance.com",
    "oraclecloud.com",
    "ats.rippling.com",
];

fn page_worth_reading(url: &str) -> bool {
    let Some((host, path)) = split_url(url) else {
        return false;
    };
    if NO_METADATA.iter().any(|h| host.ends_with(h)) {
        return false;
    }
    // A search URL is a list of postings, not one of them. A handful of rows
    // link to one, and they would otherwise each cost a page fetch.
    !path.split(['/', '?', '&']).any(|s| s == "search")
        && !url.contains("keyword=")
        && !url.contains("base_query=")
}

/// Host and path-with-query, for the routing above. `None` for anything that
/// is not an ordinary http(s) URL.
fn split_url(url: &str) -> Option<(&str, &str)> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let (host, path) = match rest.find('/') {
        Some(at) => (&rest[..at], &rest[at..]),
        None => (rest, ""),
    };
    (!host.is_empty()).then_some((host, path))
}

/// Walks a chain until a handler produces text. Never returns an error: a
/// failure is a [`DetailStatus`] to be recorded, because recording it is what
/// stops the URL being retried on every future refresh.
pub async fn fetch(client: &reqwest::Client, chain: &[Handler]) -> JobDetail {
    if chain.is_empty() {
        return JobDetail::with_status(DetailStatus::Unsupported);
    }

    let mut status = DetailStatus::Unsupported;
    for (index, handler) in chain.iter().enumerate() {
        let detail = fetch_one(client, handler).await;
        if !detail.has_text() {
            // A hard failure outranks "parsed fine, said nothing", which
            // outranks "we never had a handler".
            if detail.status == DetailStatus::Failed || status == DetailStatus::Unsupported {
                status = detail.status;
            }
            continue;
        }

        // Prose with no sections in it is the one case worth spending a second
        // request on: simplify.jobs hands back requirements already split, and
        // that is precisely what unsegmented prose is missing.
        let sectioned = detail.requirements.is_some() || detail.responsibilities.is_some();
        if !sectioned {
            if let Some(mirror) = chain[index + 1..]
                .iter()
                .find(|h| matches!(h, Handler::Simplify(_)))
            {
                let extra = fetch_one(client, mirror).await;
                if extra.has_text() {
                    return merge(detail, extra);
                }
            }
        }
        return detail;
    }
    JobDetail::with_status(status)
}

/// Fills the gaps in `detail` from `extra`, field by field. The first
/// handler's text always wins where both have it — it came from the employer.
fn merge(mut detail: JobDetail, extra: JobDetail) -> JobDetail {
    detail.description = detail.description.or(extra.description);
    detail.requirements = detail.requirements.or(extra.requirements);
    detail.responsibilities = detail.responsibilities.or(extra.responsibilities);
    detail.perks = detail.perks.or(extra.perks);
    if detail.tagged_skills.is_empty() {
        detail.tagged_skills = extra.tagged_skills;
    }
    detail.status = DetailStatus::Ok;
    detail
}

async fn fetch_one(client: &reqwest::Client, handler: &Handler) -> JobDetail {
    let result = match handler {
        Handler::Greenhouse { board, id } => fetch_greenhouse(client, board, id).await,
        Handler::Lever { org, id } => fetch_lever(client, org, id).await,
        Handler::SmartRecruiters { company, id } => {
            fetch_smart_recruiters(client, company, id).await
        }
        Handler::Workday {
            host,
            tenant,
            site,
            path,
        } => fetch_workday(client, host, tenant, site, path).await,
        Handler::Workable { account, shortcode } => {
            fetch_workable(client, account, shortcode).await
        }
        Handler::Eightfold { host, id } => fetch_eightfold(client, host, id).await,
        Handler::JobBank(url) => fetch_job_bank(client, url).await,
        Handler::Page(url) => fetch_page(client, url).await,
        Handler::Simplify(id) => fetch_simplify(client, id).await,
    };
    match result {
        Ok(detail) => detail,
        Err(e) => {
            log::debug!("{}: {e}", handler.name());
            JobDetail::with_status(DetailStatus::Failed)
        }
    }
}

// --- shared plumbing ---

/// A GET that asks for English. Without it Workday serves a Canadian posting
/// in French, which is a worse read and matches none of the skill keywords.
fn get(client: &reqwest::Client, url: String) -> reqwest::RequestBuilder {
    client.get(url).header("Accept-Language", "en-US,en;q=0.9")
}

/// A JSON GET. `None` means the posting is gone — a 404 here is an expired
/// listing, not a fault, since postings outlive their pages in the cache by
/// design.
async fn json<T: serde::de::DeserializeOwned>(
    client: &reqwest::Client,
    url: String,
) -> anyhow::Result<Option<T>> {
    let response = get(client, url).send().await?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    Ok(Some(response.error_for_status()?.json::<T>().await?))
}

/// Reads a page body, giving up at [`MAX_PAGE`] rather than buffering however
/// much a career site decided to ship.
async fn page_text(client: &reqwest::Client, url: &str) -> anyhow::Result<Option<String>> {
    let mut response = get(client, url.to_string()).send().await?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    response = response.error_for_status()?;

    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        body.extend_from_slice(&chunk);
        if body.len() >= MAX_PAGE {
            break;
        }
    }
    Ok(Some(String::from_utf8_lossy(&body).into_owned()))
}

/// Turns a split description into the row we store. `Empty` rather than `Ok`
/// when nothing came back, so the coverage check can tell a handler that works
/// from one that merely does not crash.
fn detail_of(sections: Sections, tagged_skills: Vec<String>) -> JobDetail {
    let mut detail = JobDetail {
        description: field(&sections.overview),
        requirements: field(&sections.requirements),
        responsibilities: field(&sections.responsibilities),
        perks: field(&sections.perks),
        tagged_skills,
        status: DetailStatus::Ok,
    };
    if !detail.has_text() {
        detail.status = DetailStatus::Empty;
    }
    detail
}

/// Joins lines with newlines, which is how the UI reads them back apart.
fn field(lines: &[String]) -> Option<String> {
    let joined = lines
        .iter()
        .map(|l| collapse_ws(l))
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    (!joined.trim().is_empty()).then(|| truncate_words(&joined, MAX_FIELD))
}

/// Greenhouse returns its description HTML-escaped inside JSON, so the tags
/// arrive as `&lt;div&gt;`. One text-extraction pass turns the entities back
/// into markup; without it the whole posting parses as a single run of text.
fn unescape_if_escaped(content: &str) -> String {
    if !content.contains("&lt;") {
        return content.to_string();
    }
    Html::parse_fragment(content)
        .root_element()
        .text()
        .collect::<String>()
}

// --- Greenhouse ---

#[derive(Deserialize)]
struct GreenhouseJob {
    #[serde(default)]
    content: Option<String>,
}

async fn fetch_greenhouse(
    client: &reqwest::Client,
    board: &str,
    id: &str,
) -> anyhow::Result<JobDetail> {
    let url = format!("https://boards-api.greenhouse.io/v1/boards/{board}/jobs/{id}");
    let Some(job) = json::<GreenhouseJob>(client, url).await? else {
        return Ok(JobDetail::with_status(DetailStatus::Empty));
    };
    Ok(parse_greenhouse(job.content.as_deref().unwrap_or_default()))
}

fn parse_greenhouse(content: &str) -> JobDetail {
    detail_of(sections::split(&unescape_if_escaped(content)), Vec::new())
}

// --- Lever ---

/// Lever splits a posting into named lists itself, which is the best shape any
/// of these APIs hands us: the heading is a field, not a guess.
#[derive(Deserialize)]
struct LeverPosting {
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    lists: Vec<LeverList>,
    #[serde(default)]
    additional: Option<String>,
}
#[derive(Deserialize)]
struct LeverList {
    #[serde(default)]
    text: String,
    #[serde(default)]
    content: String,
}

async fn fetch_lever(client: &reqwest::Client, org: &str, id: &str) -> anyhow::Result<JobDetail> {
    let url = format!("https://api.lever.co/v0/postings/{org}/{id}");
    let Some(posting) = json::<LeverPosting>(client, url).await? else {
        return Ok(JobDetail::with_status(DetailStatus::Empty));
    };
    Ok(parse_lever(&posting))
}

fn parse_lever(posting: &LeverPosting) -> JobDetail {
    let mut out = sections::split(posting.description.as_deref().unwrap_or_default());
    for list in &posting.lists {
        // `text` is the employer's own heading for the list, so it is
        // classified once and every line under it follows.
        out.merge(sections::split_into(
            &list.content,
            sections::section_of(&list.text),
        ));
    }
    out.merge(sections::split(
        posting.additional.as_deref().unwrap_or_default(),
    ));
    detail_of(out, Vec::new())
}

// --- SmartRecruiters ---

#[derive(Deserialize)]
struct SmartPosting {
    #[serde(rename = "jobAd", default)]
    job_ad: Option<SmartJobAd>,
}
#[derive(Deserialize)]
struct SmartJobAd {
    #[serde(default)]
    sections: SmartSections,
}
#[derive(Deserialize, Default)]
struct SmartSections {
    #[serde(rename = "companyDescription", default)]
    company: Option<SmartSection>,
    #[serde(rename = "jobDescription", default)]
    job: Option<SmartSection>,
    #[serde(default)]
    qualifications: Option<SmartSection>,
    #[serde(rename = "additionalInformation", default)]
    additional: Option<SmartSection>,
}
#[derive(Deserialize)]
struct SmartSection {
    #[serde(default)]
    text: String,
}

async fn fetch_smart_recruiters(
    client: &reqwest::Client,
    company: &str,
    id: &str,
) -> anyhow::Result<JobDetail> {
    let url = format!("https://api.smartrecruiters.com/v1/companies/{company}/postings/{id}");
    let Some(posting) = json::<SmartPosting>(client, url).await? else {
        return Ok(JobDetail::with_status(DetailStatus::Empty));
    };
    Ok(parse_smart_recruiters(&posting))
}

fn parse_smart_recruiters(posting: &SmartPosting) -> JobDetail {
    let Some(ad) = &posting.job_ad else {
        return JobDetail::with_status(DetailStatus::Empty);
    };
    let mut out = Sections::default();
    for (section, default) in [
        (&ad.sections.job, Section::Responsibilities),
        (&ad.sections.qualifications, Section::Requirements),
        (&ad.sections.additional, Section::Perks),
        (&ad.sections.company, Section::Overview),
    ] {
        if let Some(text) = section.as_ref().map(|s| s.text.as_str()) {
            out.merge(sections::split_into(text, default));
        }
    }
    detail_of(out, Vec::new())
}

// --- Workday ---

#[derive(Deserialize)]
struct WorkdayResponse {
    #[serde(rename = "jobPostingInfo", default)]
    job_posting_info: Option<WorkdayPosting>,
}
#[derive(Deserialize)]
struct WorkdayPosting {
    #[serde(rename = "jobDescription", default)]
    job_description: Option<String>,
}

async fn fetch_workday(
    client: &reqwest::Client,
    host: &str,
    tenant: &str,
    site: &str,
    path: &str,
) -> anyhow::Result<JobDetail> {
    let url = format!("https://{host}/wday/cxs/{tenant}/{site}/job/{path}");
    let Some(response) = json::<WorkdayResponse>(client, url).await? else {
        return Ok(JobDetail::with_status(DetailStatus::Empty));
    };
    let html = response
        .job_posting_info
        .and_then(|p| p.job_description)
        .unwrap_or_default();
    Ok(detail_of(sections::split(&html), Vec::new()))
}

// --- Workable ---

#[derive(Deserialize)]
struct WorkableJob {
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    requirements: Option<String>,
    #[serde(default)]
    benefits: Option<String>,
}

async fn fetch_workable(
    client: &reqwest::Client,
    account: &str,
    shortcode: &str,
) -> anyhow::Result<JobDetail> {
    let url = format!("https://apply.workable.com/api/v1/accounts/{account}/jobs/{shortcode}");
    let Some(job) = json::<WorkableJob>(client, url).await? else {
        return Ok(JobDetail::with_status(DetailStatus::Empty));
    };
    Ok(parse_workable(&job))
}

fn parse_workable(job: &WorkableJob) -> JobDetail {
    let mut out = Sections::default();
    for (html, default) in [
        (&job.description, Section::Overview),
        (&job.requirements, Section::Requirements),
        (&job.benefits, Section::Perks),
    ] {
        if let Some(text) = html.as_deref() {
            out.merge(sections::split_into(text, default));
        }
    }
    detail_of(out, Vec::new())
}

// --- Eightfold ---

#[derive(Deserialize)]
struct EightfoldJob {
    #[serde(default)]
    job_description: Option<String>,
}

async fn fetch_eightfold(client: &reqwest::Client, host: &str, id: &str) -> anyhow::Result<JobDetail> {
    let url = format!("https://{host}/api/apply/v2/jobs/{id}");
    let Some(job) = json::<EightfoldJob>(client, url).await? else {
        return Ok(JobDetail::with_status(DetailStatus::Empty));
    };
    let html = job.job_description.unwrap_or_default();
    Ok(detail_of(sections::split(&html), Vec::new()))
}

// --- schema.org metadata on the posting's own page ---

async fn fetch_page(client: &reqwest::Client, url: &str) -> anyhow::Result<JobDetail> {
    let Some(body) = page_text(client, url).await? else {
        return Ok(JobDetail::with_status(DetailStatus::Empty));
    };
    Ok(parse_page(&body))
}

fn parse_page(html: &str) -> JobDetail {
    let Some(posting) = jsonld::find(html) else {
        return JobDetail::with_status(DetailStatus::Empty);
    };

    // The typed fields are worth more than the blob, so they are merged first
    // and win any line the blob repeats.
    let mut out = Sections::default();
    for (text, default) in [
        (&posting.responsibilities, Section::Responsibilities),
        (&posting.qualifications, Section::Requirements),
        (&posting.benefits, Section::Perks),
    ] {
        if let Some(text) = text.as_deref() {
            out.merge(sections::split_into(text, default));
        }
    }
    if let Some(description) = &posting.description {
        out.merge(sections::split(description));
    }
    detail_of(out, posting.skills)
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
    /// The employer's own posting, as HTML. Simplify mirrors it verbatim, so
    /// this — not the curated lists below — is the fullest text available.
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    requirements: Vec<String>,
    /// Simplify's word for the nice-to-haves, which read as requirements.
    #[serde(default)]
    desirable: Vec<String>,
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
    let Some(body) = page_text(client, &format!("https://simplify.jobs/p/{id}")).await? else {
        // Community-contributed rows carry ids that only exist in the repo.
        return Ok(JobDetail::with_status(DetailStatus::Empty));
    };

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

    // `description` is the employer's posting as HTML — headings, paragraphs
    // and bullets intact — so it goes through the same split as every other
    // handler's. Stripping it to one line instead, as this used to, threw away
    // both the structure the UI renders and the section headings that tell
    // requirements from company boilerplate.
    let mut sections = posting
        .description
        .as_deref()
        .map(sections::split)
        .unwrap_or_default();

    // Simplify's own `requirements`/`responsibilities` are a rewrite of the
    // same posting: shorter, and paraphrased away from the employer's wording.
    // So they stand in for a section the split could not find rather than
    // merging into one, which would print each bullet twice in two voices.
    if sections.requirements.is_empty() {
        sections.requirements = posting.requirements;
        sections.requirements.extend(posting.desirable);
    }
    if sections.responsibilities.is_empty() {
        sections.responsibilities = posting.responsibilities;
    }
    if sections.perks.is_empty() {
        sections.perks = benefits;
    }

    detail_of(
        sections,
        posting
            .skills
            .into_iter()
            .filter_map(|s| s.name)
            .map(|n| collapse_ws(&n))
            .filter(|n| !n.is_empty())
            .collect(),
    )
}

// --- Job Bank Canada ---

async fn fetch_job_bank(client: &reqwest::Client, url: &str) -> anyhow::Result<JobDetail> {
    let Some(body) = page_text(client, url).await? else {
        return Ok(JobDetail::with_status(DetailStatus::Empty));
    };
    Ok(parse_job_bank(&body))
}

static JOB_BANK_BODY: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("div.job-posting-detail-requirements, section").unwrap());
static JOB_BANK_HEADING: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("h2, h3, h4").unwrap());

/// The sections of a Job Bank posting that describe the application process
/// rather than the job. They parse perfectly well, which is the problem: left
/// in, every posting's overview ends with an email address.
const JOB_BANK_SKIP: &[&str] = &["how to apply", "who can apply", "advertised until", "contact"];

/// Job Bank labels its own sections, but as `<h2>`/`<h3>` inside `<section>`
/// wrappers rather than as one description blob, so the generic split runs per
/// section with the wrapper's heading already inside it.
fn parse_job_bank(html: &str) -> JobDetail {
    let doc = Html::parse_document(html);
    let mut out = Sections::default();
    for section in doc.select(&JOB_BANK_BODY) {
        let heading = section
            .select(&JOB_BANK_HEADING)
            .next()
            .map(|h| collapse_ws(&h.text().collect::<String>()).to_lowercase())
            .unwrap_or_default();
        if JOB_BANK_SKIP.iter().any(|skip| heading.starts_with(skip)) {
            continue;
        }
        out.merge(sections::split(&section.inner_html()));
    }
    detail_of(out, Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(url: &str, simplify_id: Option<&str>) -> Vec<&'static str> {
        route(url, simplify_id).iter().map(Handler::name).collect()
    }

    #[test]
    fn the_employers_own_api_comes_before_the_mirror() {
        assert_eq!(
            names("https://job-boards.greenhouse.io/eudia/jobs/4379570009", Some("abc")),
            ["greenhouse", "simplify.jobs"]
        );
        assert_eq!(
            route("https://job-boards.greenhouse.io/eudia/jobs/4379570009", None)[0],
            Handler::Greenhouse { board: "eudia".into(), id: "4379570009".into() }
        );
    }

    #[test]
    fn workday_is_routed_to_its_cxs_endpoint() {
        assert_eq!(
            route(
                "https://analogdevices.wd1.myworkdayjobs.com/External/job/US-NC-Durham/Intern_R265305",
                None
            )[0],
            Handler::Workday {
                host: "analogdevices.wd1.myworkdayjobs.com".into(),
                tenant: "analogdevices".into(),
                site: "External".into(),
                path: "US-NC-Durham/Intern_R265305".into(),
            }
        );
    }

    #[test]
    fn a_workday_locale_segment_is_not_the_site() {
        // Leaving `en-US` in asks the API for a site that does not exist.
        assert_eq!(
            route("https://baxter.wd1.myworkdayjobs.com/en-US/baxter/job/KC/Tech_JR-199054", None)[0],
            Handler::Workday {
                host: "baxter.wd1.myworkdayjobs.com".into(),
                tenant: "baxter".into(),
                site: "baxter".into(),
                path: "KC/Tech_JR-199054".into(),
            }
        );
    }

    #[test]
    fn the_shared_workday_host_names_its_tenant_in_the_path() {
        assert_eq!(
            route("https://wd1.myworkdaysite.com/recruiting/snapchat/snap/job/Bellevue/Intern_R1", None)[0],
            Handler::Workday {
                host: "wd1.myworkdaysite.com".into(),
                tenant: "snapchat".into(),
                site: "snap".into(),
                path: "Bellevue/Intern_R1".into(),
            }
        );
    }

    #[test]
    fn the_other_applicant_tracking_systems_parse_their_links() {
        assert_eq!(
            route("https://jobs.lever.co/zoox/51838a63-2dde/apply", None)[0],
            Handler::Lever { org: "zoox".into(), id: "51838a63-2dde".into() }
        );
        assert_eq!(
            route("https://jobs.smartrecruiters.com/LLNL/3743990014731696", None)[0],
            Handler::SmartRecruiters { company: "LLNL".into(), id: "3743990014731696".into() }
        );
        assert_eq!(
            route("https://apply.workable.com/altom-transport/j/6C00FDED5C/", None)[0],
            Handler::Workable { account: "altom-transport".into(), shortcode: "6C00FDED5C".into() }
        );
        assert_eq!(
            route("https://eaton.eightfold.ai/careers/job/687238323389", None)[0],
            Handler::Eightfold { host: "eaton.eightfold.ai".into(), id: "687238323389".into() }
        );
        assert!(matches!(
            route("https://www.jobbank.gc.ca/jobsearch/jobpostingtfw/123", None)[0],
            Handler::JobBank(_)
        ));
    }

    #[test]
    fn a_host_with_no_handler_falls_back_to_the_page_then_the_mirror() {
        assert_eq!(
            names("https://careers-sig.icims.com/jobs/9210/job", Some("abc")),
            ["schema.org", "simplify.jobs"]
        );
        assert_eq!(names("https://jobs.ashbyhq.com/mercor/a0a9/application", None), ["schema.org"]);
    }

    #[test]
    fn a_site_we_measured_as_bare_is_not_fetched_for_metadata() {
        // Its own API already answered, or its pages carry no metadata at all.
        assert_eq!(names("https://lifeattiktok.com/position/7676276048527214901", Some("x")), ["simplify.jobs"]);
        assert_eq!(names("https://jobs.smartrecruiters.com/LLNL/374399", None), ["smartrecruiters"]);
    }

    #[test]
    fn a_search_link_is_not_a_posting() {
        assert!(names("https://www.amazon.jobs/en/search?base_query=intern", None).is_empty());
        assert!(names("https://wd1.myworkdaysite.com/recruiting/abinbev/USA/?keyword=intern", None).is_empty());
    }

    #[test]
    fn a_posting_with_nothing_to_go_on_routes_nowhere() {
        assert!(route("", None).is_empty());
        assert!(route("mailto:jobs@example.com", None).is_empty());
        // An empty id is not an id.
        assert_eq!(names("https://jobs.smartrecruiters.com/LLNL/374399", Some("")), ["smartrecruiters"]);
    }

    #[test]
    fn parses_the_simplify_payload() {
        // No headings in the description, so Simplify's own curated lists are
        // what fill the sections, and the prose stays whole as the overview.
        let payload = r#"{"props":{"pageProps":{"jobPosting":{
            "description":"<p>About us</p><div>We build things</div>",
            "requirements":["You are proficient in Python, R, or MATLAB."],
            "desirable":["Comfortable with statistics."],
            "responsibilities":["Build scalable models."],
            "skills":[{"name":"Python","id":"1"},{"name":"Data Visualization","id":"2"}],
            "job":{"company":{"benefits":["Health Insurance","401(k) Company Match"]}}
        }}}}"#;
        let d = parse_simplify(payload);

        assert_eq!(d.status, DetailStatus::Ok);
        // Two blocks, two lines — not one run of text, which is what the UI
        // used to have to render as an unbroken wall.
        assert_eq!(d.description.as_deref(), Some("About us\nWe build things"));
        assert_eq!(
            d.requirements.as_deref(),
            Some("You are proficient in Python, R, or MATLAB.\nComfortable with statistics.")
        );
        assert_eq!(d.responsibilities.as_deref(), Some("Build scalable models."));
        assert_eq!(d.perks.as_deref(), Some("Health Insurance\n401(k) Company Match"));
        assert_eq!(d.tagged_skills, vec!["Python", "Data Visualization"]);
    }

    #[test]
    fn the_employers_own_headings_beat_simplifys_rewrite() {
        // Simplify mirrors the posting's HTML in `description` and *also*
        // paraphrases it into `requirements`. Where the mirror carries the
        // employer's own headings, those win: same facts, the employer's
        // words, and no bullet printed twice in two voices.
        let payload = r#"{"props":{"pageProps":{"jobPosting":{
            "description":"<p>Join us.</p><h3>Requirements</h3><ul><li>Three years of Rust</li></ul><h3>Benefits</h3><ul><li>Free lunch</li></ul>",
            "requirements":["Some experience with a systems language."],
            "responsibilities":["Build scalable models."],
            "skills":[],
            "job":{"company":{"benefits":["Health Insurance"]}}
        }}}}"#;
        let d = parse_simplify(payload);

        assert_eq!(d.description.as_deref(), Some("Join us."));
        assert_eq!(d.requirements.as_deref(), Some("Three years of Rust"));
        assert_eq!(d.perks.as_deref(), Some("Free lunch"));
        // Nothing in the description spoke to duties, so the rewrite stands in.
        assert_eq!(d.responsibilities.as_deref(), Some("Build scalable models."));
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
    fn greenhouse_content_arrives_escaped() {
        let content =
            "&lt;h3&gt;Requirements&lt;/h3&gt;&lt;ul&gt;&lt;li&gt;Rust &amp;amp; C&lt;/li&gt;&lt;/ul&gt;";
        let d = parse_greenhouse(content);
        assert_eq!(d.requirements.as_deref(), Some("Rust & C"));
    }

    #[test]
    fn lever_lists_are_bucketed_by_their_own_headings() {
        let posting = LeverPosting {
            description: Some("<div>About Zoox</div>".into()),
            lists: vec![
                LeverList {
                    text: "Responsibilities".into(),
                    content: "<li>Build forecasting models</li>".into(),
                },
                LeverList {
                    text: "Required Qualifications".into(),
                    content: "<li>Python</li><li>SQL</li>".into(),
                },
            ],
            additional: None,
        };
        let d = parse_lever(&posting);
        assert_eq!(d.description.as_deref(), Some("About Zoox"));
        assert_eq!(d.responsibilities.as_deref(), Some("Build forecasting models"));
        assert_eq!(d.requirements.as_deref(), Some("Python\nSQL"));
    }

    #[test]
    fn smartrecruiters_sections_land_where_their_names_say() {
        let posting = SmartPosting {
            job_ad: Some(SmartJobAd {
                sections: SmartSections {
                    company: Some(SmartSection { text: "<p>About LLNL</p>".into() }),
                    job: Some(SmartSection { text: "<p>Model energy systems</p>".into() }),
                    qualifications: Some(SmartSection { text: "<ul><li>A degree</li></ul>".into() }),
                    additional: Some(SmartSection { text: "<p>Full benefits</p>".into() }),
                },
            }),
        };
        let d = parse_smart_recruiters(&posting);
        assert_eq!(d.responsibilities.as_deref(), Some("Model energy systems"));
        assert_eq!(d.requirements.as_deref(), Some("A degree"));
        assert_eq!(d.perks.as_deref(), Some("Full benefits"));
        assert_eq!(d.description.as_deref(), Some("About LLNL"));
    }

    #[test]
    fn workable_fields_are_already_labelled() {
        let d = parse_workable(&WorkableJob {
            description: Some("<p>Join us</p>".into()),
            requirements: Some("<ul><li>Electrical engineering</li></ul>".into()),
            benefits: Some(String::new()),
        });
        assert_eq!(d.requirements.as_deref(), Some("Electrical engineering"));
        assert_eq!(d.perks, None);
    }

    #[test]
    fn schema_metadata_prefers_its_typed_fields_over_the_blob() {
        let page = r#"<html><head><script type="application/ld+json">
            {"@type":"JobPosting","description":"<h3>Overview</h3><p>We build things</p>",
             "qualifications":"<ul><li>Three years of Rust</li></ul>",
             "skills":"Rust, Kubernetes"}
            </script></head><body></body></html>"#;
        let d = parse_page(page);
        assert_eq!(d.requirements.as_deref(), Some("Three years of Rust"));
        assert!(d.description.as_deref().unwrap().contains("We build things"));
        assert_eq!(d.tagged_skills, ["Rust", "Kubernetes"]);
    }

    #[test]
    fn a_page_without_metadata_is_empty() {
        assert_eq!(parse_page("<html><body>A shell</body></html>").status, DetailStatus::Empty);
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
        assert!(d.perks.as_deref().unwrap().contains("Free parking available"));
        assert!(!d.responsibilities.as_deref().unwrap().contains("By email"));
    }

    #[test]
    fn fields_are_truncated_on_a_word_boundary() {
        let long = "word ".repeat(4_000);
        let out = field(&[long]).unwrap();
        assert!(out.len() <= MAX_FIELD);
        assert!(out.ends_with("word"), "cut mid-word: {:?}", &out[out.len() - 20..]);
    }

    #[test]
    fn merging_keeps_the_employers_own_text() {
        let employer = JobDetail {
            description: Some("From the employer".into()),
            status: DetailStatus::Ok,
            ..Default::default()
        };
        let mirror = JobDetail {
            description: Some("From the mirror".into()),
            requirements: Some("Rust".into()),
            tagged_skills: vec!["Rust".into()],
            status: DetailStatus::Ok,
            ..Default::default()
        };
        let merged = merge(employer, mirror);
        assert_eq!(merged.description.as_deref(), Some("From the employer"));
        assert_eq!(merged.requirements.as_deref(), Some("Rust"));
        assert_eq!(merged.tagged_skills, ["Rust"]);
    }
}
