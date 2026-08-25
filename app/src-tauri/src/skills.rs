//! The software-engineering skill catalog, and extraction of the skills a
//! posting is asking for.
//!
//! Interests (`rank::INTERESTS`) say what the user wants to *read about*; these
//! say what they can *build*. The difference matters on the internships hub,
//! where two postings that both classify as "Backend" can want entirely
//! different stacks.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use serde::Serialize;

use crate::models::Item;

/// Category → skills → keywords.
///
/// The category is what the picker groups by and what a job detail labels its
/// chip rows with; the keywords decide whether a posting is asking for that
/// skill.
///
/// Keyword rules, inherited from [`crate::rank::INTERESTS`]: matching is naive
/// substring containment over a space-padded lowercased haystack, so every
/// keyword is either long enough to be distinctive or written space-padded
/// (`" go "`, `" c "`, `" r "`). A bare short token like `"go"` or `"r"` fires
/// inside ordinary words — "algorithms", "great" — and is never acceptable
/// here. The tests at the bottom of this file guard exactly that.
/// One skill: its display name and the keywords that betray it in a posting.
pub type Skill = (&'static str, &'static [&'static str]);
/// One category: its display name and the skills under it.
pub type Category = (&'static str, &'static [Skill]);

pub const SKILLS: &[Category] = &[
    (
        "Languages",
        &[
            ("Python", &["python", "py3", "cpython"]),
            ("JavaScript", &["javascript", " js ", "es6", "ecmascript"]),
            ("TypeScript", &["typescript", " ts ", ".ts "]),
            ("Java", &[" java ", " java,", " java.", "jvm", "openjdk"]),
            ("C", &[" c ", " c,", "c99", "c11", " c/"]),
            ("C++", &["c++", "cpp", "cplusplus"]),
            ("C#", &["c#", "csharp", "c-sharp"]),
            ("Go", &["golang", " go ", " go,", " go/"]),
            ("Rust", &[" rust ", " rust,", " rust.", "rustlang"]),
            ("Swift", &["swift ", "swift,", "swiftui"]),
            ("Kotlin", &["kotlin"]),
            ("Ruby", &["ruby"]),
            ("PHP", &["php", "laravel"]),
            ("Scala", &["scala ", "scala,", "scala."]),
            // Padded so it does not fire inside "postgresql" or "mysql", which
            // are their own skills.
            (
                "SQL",
                &[" sql ", " sql,", " sql.", " sql/", "t-sql", "pl/sql"],
            ),
            ("R", &[" r ", " r,", " r/", "rstudio", "tidyverse"]),
            ("MATLAB", &["matlab", "simulink"]),
            (
                "Shell / Bash",
                &["bash", "shell script", "zsh", "powershell"],
            ),
        ],
    ),
    (
        "Frontend",
        &[
            ("React", &["react", "react.js", "reactjs"]),
            ("Next.js", &["next.js", "nextjs"]),
            ("Vue", &["vue.js", "vuejs", "vue 3", "nuxt"]),
            ("Angular", &["angular"]),
            ("Svelte", &["svelte", "sveltekit"]),
            ("HTML / CSS", &["html", "css", "scss", "sass"]),
            ("Tailwind CSS", &["tailwind"]),
            ("Redux", &["redux", "zustand", "state management"]),
            (
                "Web Accessibility",
                &["accessibility", "a11y", "wcag", "screen reader"],
            ),
            ("WebGL", &["webgl", "three.js", "canvas api"]),
            // " vite" rather than "vite", which fires inside "invite".
            (
                "Webpack / Vite",
                &["webpack", " vite ", " vite,", "vite.config", "esbuild"],
            ),
        ],
    ),
    (
        "Mobile",
        &[
            ("Android", &["android", "android sdk"]),
            // " ios " rather than "ios ", which fires inside "scenarios".
            ("iOS", &[" ios ", " ios,", "ios development", "xcode"]),
            ("SwiftUI", &["swiftui"]),
            ("Jetpack Compose", &["jetpack compose"]),
            ("React Native", &["react native"]),
            ("Flutter", &["flutter", "dart "]),
            (
                "App Store Release",
                &["app store", "play store", "testflight", "mobile release"],
            ),
        ],
    ),
    (
        "Backend & APIs",
        &[
            ("Node.js", &["node.js", "nodejs", "node js"]),
            // Not a bare "express", which fires on "express interest in".
            (
                "Express",
                &[
                    "express.js",
                    "expressjs",
                    "express framework",
                    "express server",
                ],
            ),
            ("Django", &["django"]),
            ("Flask", &["flask"]),
            ("FastAPI", &["fastapi"]),
            (
                "Spring Boot",
                &["spring boot", "spring framework", "springboot"],
            ),
            (".NET", &[".net", "dotnet", "asp.net"]),
            // Not a bare "rails ", which fires inside "guardrails".
            (
                "Ruby on Rails",
                &["ruby on rails", "rails app", "activerecord"],
            ),
            ("REST APIs", &["rest api", "restful", "http api"]),
            ("GraphQL", &["graphql", "apollo"]),
            ("gRPC", &["grpc", "protobuf", "protocol buffer"]),
            (
                "Microservices",
                &["microservice", "service-oriented", "distributed system"],
            ),
            ("WebSockets", &["websocket", "socket.io", "real-time api"]),
            (
                "Message Queues",
                &["rabbitmq", "message queue", "pub/sub", "sqs", "celery"],
            ),
        ],
    ),
    (
        "Data & Databases",
        &[
            ("PostgreSQL", &["postgres", "postgresql", "psql"]),
            ("MySQL", &["mysql", "mariadb"]),
            ("MongoDB", &["mongodb", "mongo ", "nosql"]),
            ("Redis", &["redis", "memcached"]),
            ("SQLite", &["sqlite"]),
            ("Elasticsearch", &["elasticsearch", "opensearch", "lucene"]),
            (
                "Apache Spark",
                &["apache spark", "pyspark", "spark sql", "spark job"],
            ),
            ("Kafka", &["kafka", "kinesis", "event streaming"]),
            (
                "ETL / Pipelines",
                &[
                    " etl ",
                    " elt ",
                    "etl pipeline",
                    "data pipeline",
                    "ingestion pipeline",
                ],
            ),
            ("Snowflake", &["snowflake"]),
            ("BigQuery", &["bigquery", "redshift", "data warehouse"]),
            ("dbt", &["dbt "]),
            ("Pandas", &["pandas", "numpy", "dataframe"]),
            ("Airflow", &["airflow", "dagster", "prefect", "luigi"]),
        ],
    ),
    (
        "AI / ML",
        &[
            ("PyTorch", &["pytorch", "torch "]),
            ("TensorFlow", &["tensorflow", "keras"]),
            ("scikit-learn", &["scikit-learn", "sklearn"]),
            (
                "Deep Learning",
                &["deep learning", "neural network", "transformer"],
            ),
            (
                "NLP",
                &[
                    "natural language processing",
                    " nlp ",
                    "text classification",
                ],
            ),
            (
                "Computer Vision",
                &[
                    "computer vision",
                    "image recognition",
                    "opencv",
                    "object detection",
                ],
            ),
            (
                "LLMs",
                &[
                    "large language model",
                    " llm ",
                    "llms",
                    "gpt-",
                    "prompt engineering",
                ],
            ),
            (
                "RAG",
                &[
                    "retrieval-augmented",
                    " rag ",
                    "vector database",
                    "embeddings",
                ],
            ),
            (
                "Hugging Face",
                &["hugging face", "huggingface", "transformers library"],
            ),
            (
                "MLOps",
                &[
                    "mlops",
                    "model serving",
                    "model deployment",
                    "feature store",
                ],
            ),
            (
                "Reinforcement Learning",
                &["reinforcement learning", " rlhf ", "policy gradient"],
            ),
        ],
    ),
    (
        "Cloud & DevOps",
        &[
            // " aws" rather than "aws", which fires inside "laws" and "flaws".
            (
                "AWS",
                &[
                    " aws ",
                    " aws,",
                    " aws.",
                    " aws/",
                    "amazon web services",
                    "ec2 ",
                    "lambda function",
                ],
            ),
            ("Azure", &["azure"]),
            ("GCP", &["gcp", "google cloud"]),
            ("Docker", &["docker", "containeriz"]),
            ("Kubernetes", &["kubernetes", "k8s", "helm chart"]),
            (
                "Terraform",
                &["terraform", "pulumi", "infrastructure as code"],
            ),
            (
                "CI/CD",
                &["ci/cd", "continuous integration", "continuous deployment"],
            ),
            ("GitHub Actions", &["github actions"]),
            ("Jenkins", &["jenkins", "circleci", "gitlab ci", "teamcity"]),
            ("Linux", &["linux", "unix", "ubuntu", "debian"]),
            (
                "Nginx",
                &["nginx", "apache http", "load balanc", "reverse proxy"],
            ),
            (
                "Serverless",
                &["serverless", "cloud function", "edge function"],
            ),
            (
                "Observability",
                &[
                    "observability",
                    "prometheus",
                    "grafana",
                    "datadog",
                    "opentelemetry",
                ],
            ),
            ("Ansible", &["ansible", "puppet", " chef "]),
        ],
    ),
    (
        "Systems & Low-level",
        &[
            (
                "Operating Systems",
                &["operating system", "kernel", "scheduler design"],
            ),
            (
                "Compilers",
                &["compiler", "llvm", "interpreter design", "parser generator"],
            ),
            (
                "Embedded / Firmware",
                &["embedded", "firmware", "microcontroller", "rtos"],
            ),
            (
                "Concurrency",
                &[
                    "concurrency",
                    "multithread",
                    "parallel programming",
                    "async runtime",
                ],
            ),
            (
                "Networking",
                &[
                    "tcp/ip",
                    "networking protocol",
                    "socket programming",
                    " dns ",
                ],
            ),
            (
                "Memory Management",
                &["memory management", "memory safety", "garbage collect"],
            ),
            (
                "Assembly",
                &["assembly language", "x86 assembly", "arm assembly"],
            ),
            (
                "FPGA / Verilog",
                &["fpga", "verilog", "vhdl", "hardware description"],
            ),
            (
                "Robotics / ROS",
                &["robotics", " ros ", "ros2", "motion planning"],
            ),
            (
                "Real-time Systems",
                &["real-time system", "low latency", "deterministic latency"],
            ),
        ],
    ),
    (
        "Security",
        &[
            (
                "Application Security",
                &[
                    "application security",
                    "appsec",
                    "owasp",
                    "secure development",
                ],
            ),
            (
                "Cryptography",
                &["cryptograph", "encryption", " tls ", "public key"],
            ),
            (
                "Penetration Testing",
                &["penetration test", "pentest", "red team", "ethical hacking"],
            ),
            (
                "Threat Modeling",
                &["threat model", "risk assessment", "attack surface"],
            ),
            (
                "OAuth / Identity",
                &["oauth", "openid", " saml ", "single sign-on", " iam "],
            ),
            (
                "Reverse Engineering",
                &["reverse engineer", "binary analysis", "disassembl"],
            ),
            (
                "Incident Response",
                &["incident response", " soc ", "siem", "threat detection"],
            ),
            (
                "Secure Code Review",
                &["secure code review", "static analysis", " sast ", " dast "],
            ),
        ],
    ),
    (
        "Tools & Practices",
        &[
            // Deliberately not "github"/"gitlab": three of our sources are
            // GitHub repos, and a listing living on GitHub is not the same as
            // a posting asking for Git.
            ("Git", &[" git ", " git,", "version control"]),
            ("Agile / Scrum", &["agile", "scrum", "sprint ", "kanban"]),
            (
                "Code Review",
                &["code review", "pull request", "peer review"],
            ),
            ("Unit Testing", &["unit test", "jest", "pytest", "junit"]),
            (
                "Integration Testing",
                &[
                    "integration test",
                    "end-to-end test",
                    "e2e test",
                    "cypress",
                    "playwright",
                ],
            ),
            (
                "TDD",
                &["test-driven", " tdd ", "behaviour-driven", " bdd "],
            ),
            (
                "Debugging",
                &["debugging", "troubleshoot", "root cause analysis"],
            ),
            (
                "System Design",
                &[
                    "system design",
                    "software architecture",
                    "design pattern",
                    "scalable system",
                ],
            ),
            (
                "Data Structures & Algorithms",
                &["data structures", "algorithms", "complexity analysis"],
            ),
            // Not a bare "documentation", which appears in almost every
            // posting and would put this chip on all of them.
            (
                "Technical Writing",
                &[
                    "technical writing",
                    "technical documentation",
                    "write documentation",
                ],
            ),
            ("Jira", &["jira", "confluence", "linear.app"]),
            ("Figma", &["figma", "design system", "wireframe"]),
        ],
    ),
];

/// One catalog category, as the picker consumes it.
#[derive(Serialize, Clone, Debug)]
pub struct SkillCategory {
    pub name: String,
    pub skills: Vec<String>,
}

/// The catalog, in declaration order, so the UI never keeps its own copy.
pub fn list_skills() -> Vec<SkillCategory> {
    SKILLS
        .iter()
        .map(|(name, skills)| SkillCategory {
            name: name.to_string(),
            skills: skills.iter().map(|(skill, _)| skill.to_string()).collect(),
        })
        .collect()
}

/// The catalog name matching an outside label, case- and spacing-insensitively.
///
/// Sources tag postings with their own vocabulary ("Node.JS", "postgresql"),
/// which lines up with the catalog often enough to be worth normalising for
/// and never often enough to assume.
pub fn canonical(label: &str) -> Option<&'static str> {
    let needle = normalize(label);
    if needle.is_empty() {
        return None;
    }
    SKILLS
        .iter()
        .flat_map(|(_, skills)| skills.iter())
        .find(|(name, _)| normalize(name) == needle)
        .map(|(name, _)| *name)
}

/// Lowercases and drops everything that is not alphanumeric, so "Node.js",
/// "node js" and "NodeJS" collapse to one key. Deliberately keeps digits —
/// "C++" and "C#" must not collapse into "C".
fn normalize(s: &str) -> String {
    let mut out: String = s
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '+' || *c == '#')
        .flat_map(|c| c.to_lowercase())
        .collect();
    // "nodejs" and "node" should not match, but "reactjs"/"react" should; the
    // trailing "js" is noise only when something precedes it.
    if out.len() > 2 && out.ends_with("js") {
        out.truncate(out.len() - 2);
    }
    out
}

/// The category a skill belongs to, for grouping the chips on a job detail.
pub fn category_of(skill: &str) -> Option<&'static str> {
    SKILLS.iter().find_map(|(category, skills)| {
        skills
            .iter()
            .any(|(name, _)| *name == skill)
            .then_some(*category)
    })
}

/// Which catalog skills this posting wants, in catalog order.
///
/// Read off the posting and nothing else: catalog keywords found in the
/// requirements, the duties and the description the enrichment pass fetched,
/// plus any skill the source tagged the posting with. A posting whose page we
/// could not read reports nothing, and the UI says so.
///
/// There used to be a `ROLES` table here that guessed a toolset from the job
/// title where the fetch came back empty. It is gone on purpose. A guess and a
/// reading look identical once they are chips in the same panel, so a match
/// score built on one is not something the reader can act on — "this posting
/// asks for React" has to mean the posting said React.
///
/// Only meaningful for opportunities — an article mentioning Kubernetes is not
/// asking you to know it — so callers gate on
/// [`ItemType::is_opportunity`](crate::models::ItemType::is_opportunity)
/// rather than paying for this on every row of the feed.
pub fn extract(item: &Item) -> Vec<String> {
    // Memoized because this is a pure function of the item's text, and the
    // caller runs it over the same unchanging cache again and again: one
    // `loadAll()` in the UI ranks the feed and the hub back to back, and every
    // settings change does it again.
    //
    // Measured over 1,535 real postings: 17.6ms cold, 0.26ms warm (release);
    // 316ms cold under `tauri dev`, which is the build you actually feel while
    // working on this. Not a crisis either way — the memo is here because it
    // is twenty lines for a 67x cut on a path that runs on every keystroke's
    // worth of settings churn, and phones are slower than this laptop.
    let key = &item.url;
    if let Some(hit) = memo().lock().unwrap().get(key) {
        return hit.clone();
    }

    let found = extract_uncached(item);
    let mut cache = memo().lock().unwrap();
    // The keys are URLs, so this is bounded by the item cache — but a long
    // session that refreshes for weeks would still creep. Dropping the whole
    // map is fine: it costs one recompute pass, not correctness.
    if cache.len() >= MEMO_CAPACITY {
        cache.clear();
    }
    cache.insert(key.clone(), found.clone());
    found
}

/// Comfortably larger than any cache the retention window can hold, so
/// ordinary use never trips the reset.
///
/// This was 8,000 — "roughly twice a full cache" when a full cache was ~1,500
/// postings. The employer boards took the cache past 18,000, at which point
/// the memo filled, cleared, and filled again *within a single ranking pass*
/// and never served a hit: `personalize` measured 2,264 ms cold and 2,250 ms
/// warm over the real database. A memo that never hits is worse than no memo,
/// so the number has to stay ahead of `RETENTION_DAYS` worth of postings.
const MEMO_CAPACITY: usize = 100_000;

fn memo() -> &'static Mutex<HashMap<String, Vec<String>>> {
    static MEMO: OnceLock<Mutex<HashMap<String, Vec<String>>>> = OnceLock::new();
    MEMO.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Drops the memo. Called after a scrape, when a URL's text may have changed
/// under a key we have already answered for.
pub fn clear_memo() {
    memo().lock().unwrap().clear();
}

/// Skills whose ordinary keywords are also ordinary English words, and the
/// only keywords they may match on in long text.
///
/// The catalog was tuned against a 24-70 character haystack of location text,
/// where `" go "` and `" c "` were safe. Against a real 8 KB description they
/// are not: "go to market", "ready to go", "R&D", and any bullet list with a
/// stray letter all fire.
///
/// Stated as an explicit allow-list rather than derived from the keyword's
/// shape, because no rule of thumb survives the real cases — `" c/"` looks
/// loose but only ever matches "C/C++", while `" rag "` looks specific and is
/// a common noun. Four entries is cheap to read and impossible to be subtly
/// wrong about.
const AMBIGUOUS: &[(&str, &[&str])] = &[
    ("Go", &["golang", " go/", "go programming"]),
    ("C", &[" c/", "c99", "c11", "c programming"]),
    ("R", &["rstudio", "tidyverse", " r/", "r programming"]),
    (
        "RAG",
        &["retrieval-augmented", "vector database", "embeddings"],
    ),
];

/// The keywords a skill may match on, given how much text there is to search.
fn keywords_for<'a>(name: &str, keywords: &'a [&'a str], long: bool) -> &'a [&'a str] {
    if !long {
        return keywords;
    }
    match AMBIGUOUS.iter().find(|(skill, _)| *skill == name) {
        Some((_, tightened)) => tightened,
        None => keywords,
    }
}

/// Every catalog keyword in one automaton, so a posting is read once.
///
/// The scan used to be `keyword.iter().any(|kw| text.contains(kw))` per skill:
/// about 600 independent passes over a haystack that averages 1.6 KB across
/// the real cache, for each of 17,739 opportunities. Aho-Corasick answers the
/// same question — which of these patterns occur — in a single pass, and it is
/// already in the dependency tree underneath `regex`.
///
/// Two automatons because [`AMBIGUOUS`] changes the pattern set: a short line
/// of scraped metadata keeps the loose keywords, a real description does not.
/// `find_overlapping_iter` rather than `find_iter`, because non-overlapping
/// leftmost matching would consume "react" and never see the "react native"
/// that contains it.
struct Matcher {
    automaton: aho_corasick::AhoCorasick,
    /// Pattern index → index into the flattened catalog.
    owner: Vec<usize>,
}

impl Matcher {
    fn build(long: bool) -> Self {
        let mut patterns: Vec<&str> = Vec::new();
        let mut owner: Vec<usize> = Vec::new();
        for (skill, (name, keywords)) in catalog().iter().enumerate() {
            for keyword in keywords_for(name, keywords, long) {
                patterns.push(keyword);
                owner.push(skill);
            }
        }
        Matcher {
            automaton: aho_corasick::AhoCorasick::new(&patterns)
                .expect("the catalog's keywords build an automaton"),
            owner,
        }
    }

    /// Which catalog entries this text names, as a flag per skill.
    fn hits(&self, text: &str) -> Vec<bool> {
        let mut found = vec![false; catalog().len()];
        for m in self.automaton.find_overlapping_iter(text) {
            found[self.owner[m.pattern().as_usize()]] = true;
        }
        found
    }
}

/// The catalog flattened once, so a skill has a stable index.
fn catalog() -> &'static [Skill] {
    static FLAT: OnceLock<Vec<Skill>> = OnceLock::new();
    FLAT.get_or_init(|| {
        SKILLS
            .iter()
            .flat_map(|(_, skills)| skills.iter().copied())
            .collect()
    })
}

fn matcher(long: bool) -> &'static Matcher {
    static SHORT: OnceLock<Matcher> = OnceLock::new();
    static LONG: OnceLock<Matcher> = OnceLock::new();
    if long {
        LONG.get_or_init(|| Matcher::build(true))
    } else {
        SHORT.get_or_init(|| Matcher::build(false))
    }
}

/// Above this many characters, the haystack is a real description rather than
/// a line of scraped metadata, and [`AMBIGUOUS`] skills tighten up.
const LONG_TEXT: usize = 400;

/// Whether the enrichment pass reached this posting, so the UI can tell "asks
/// for nothing we recognise" apart from "we never got to read it". Those look
/// identical in an empty panel and mean opposite things to someone deciding
/// whether to apply.
///
/// `hasScrapedPosting` in `lib/item.ts` asks the same question of the same
/// fields, and has to keep doing so — it is what picks the wording.
pub fn from_posting(item: &Item) -> bool {
    [
        &item.requirements,
        &item.responsibilities,
        &item.description,
    ]
    .into_iter()
    .flatten()
    .any(|text| !text.trim().is_empty())
        || !item.tagged_skills.is_empty()
}

/// The catalog skills a piece of free text names.
///
/// The posting side reaches this through [`extract`]; a résumé bullet reaches
/// it directly, which is the whole reason "Built a Rust service backed by
/// SQLite" and a posting asking for Rust and SQLite meet on one vocabulary
/// instead of two that drift.
///
/// `strict` forces the [`AMBIGUOUS`] tightening on regardless of length.
/// Postings pass `false` and let the length rule decide, because a short line
/// of scraped metadata naming "Go" almost certainly means the language. A
/// résumé bullet passes `true`: it is English prose, where " go " is far more
/// often the verb, and "went from 3 to 30 engineers" should not put Go on
/// somebody's CV.
pub fn from_text(text: &str, strict: bool) -> Vec<String> {
    if text.trim().is_empty() {
        return Vec::new();
    }
    // Padded and lowercased for the same reasons the posting path does it:
    // keywords are written with leading and trailing spaces so they can match
    // at either end, and every catalog keyword is ASCII.
    let mut buf = String::with_capacity(text.len() + 2);
    buf.push(' ');
    buf.push_str(text);
    buf.push(' ');
    buf.make_ascii_lowercase();

    let long = strict || buf.len() > LONG_TEXT;
    hits_to_names(matcher(long).hits(&buf))
}

/// Catalog-ordered names for a hit flag per skill. Shared so the two callers
/// cannot disagree about ordering, which is what makes the result stable.
fn hits_to_names(found: Vec<bool>) -> Vec<String> {
    catalog()
        .iter()
        .zip(&found)
        .filter(|(_, hit)| **hit)
        .map(|((name, _), _)| name.to_string())
        .collect()
}

fn extract_uncached(item: &Item) -> Vec<String> {
    // Everything the employer wrote, read together. The requirements are the
    // densest part and the description the loosest, but a posting that names
    // its stack in the opening paragraph and never repeats it in a bullet is
    // ordinary, and reading only the bullets left those skills off the panel.
    //
    // `perks` is deliberately not here: a benefits list is what the job pays,
    // not what it asks for, and it is where "compensation" language lives.
    //
    // `content_text` is the fallback for the sources the enrichment pass could
    // not reach — one scraped line, usually the office location, which is what
    // [`LONG_TEXT`] distinguishes from a real description below.
    //
    // Built into one buffer and lowercased in place. The description of a real
    // posting averages 3.5 KB, and the `format!(…).to_lowercase()` this
    // replaces walked that text three more times than it had to — allocating a
    // fresh copy each time — over every one of 17,739 opportunities.
    let fields = [
        &item.requirements,
        &item.responsibilities,
        &item.description,
    ];
    let body_len: usize = fields
        .iter()
        .flat_map(|f| f.iter())
        .map(|t| t.len() + 1)
        .sum();

    // Padded so keywords written with leading/trailing spaces (" go ") can
    // match at the very start or end of the text.
    let mut text = String::with_capacity(item.title.len() + body_len + item.content_text.len() + 3);
    text.push(' ');
    text.push_str(&item.title);
    let title_end = text.len();
    for field in fields.into_iter().flatten() {
        text.push(' ');
        text.push_str(field);
    }
    if text[title_end..].trim().is_empty() {
        text.push(' ');
        text.push_str(&item.content_text);
    }
    text.push(' ');
    // Every catalog keyword is ASCII, so ASCII casefolding is exactly as
    // precise here as the Unicode one and does not have to reallocate.
    text.make_ascii_lowercase();
    let long = text.len() > LONG_TEXT;

    let mut found = matcher(long).hits(&text);

    // Skills the source tagged the posting with, where they name something the
    // catalog knows. Trusted outright — a tag is somebody's judgement about
    // the posting, not a substring that happened to appear in it.
    for tag in &item.tagged_skills {
        if let Some(name) = canonical(tag) {
            if let Some(at) = catalog().iter().position(|(n, _)| *n == name) {
                found[at] = true;
            }
        }
    }

    // The flags are already in catalog order, so the result is too — and
    // therefore stable between calls.
    hits_to_names(found)
}

#[cfg(test)]
mod from_text_tests {
    use super::*;

    #[test]
    fn reads_a_resume_bullet() {
        let got = from_text(
            "Built a Rust service backed by PostgreSQL and deployed it on AWS",
            true,
        );
        assert!(got.contains(&"Rust".to_string()), "{got:?}");
        assert!(got.contains(&"PostgreSQL".to_string()), "{got:?}");
        assert!(got.contains(&"AWS".to_string()), "{got:?}");
    }

    /// The reason `strict` exists. Loose matching would put Go on a CV for a
    /// bullet about team growth, and the user would have to notice and remove
    /// a skill they never claimed.
    #[test]
    fn strict_mode_ignores_english_prose() {
        let prose = "Helped the team go from 3 to 30 engineers and kept velocity";
        assert!(!from_text(prose, true).contains(&"Go".to_string()));
    }

    #[test]
    fn strict_mode_still_finds_the_real_language() {
        assert!(from_text("Wrote the ingest pipeline in Golang", true).contains(&"Go".to_string()));
    }

    #[test]
    fn empty_text_names_nothing() {
        assert!(from_text("   ", true).is_empty());
    }

    #[test]
    fn results_are_in_catalog_order_and_stable() {
        let text = "React and TypeScript on the front end, Python on the back";
        assert_eq!(from_text(text, true), from_text(text, true));
    }

    /// The posting path and the résumé path must agree, or a bullet could
    /// claim a skill the matcher will never find in a job description.
    #[test]
    fn agrees_with_the_posting_extractor() {
        use crate::models::{Item, ItemType};
        let text = "Docker, Kubernetes and Terraform for the deploy path";
        let mut item = Item::new("Infra", "test", ItemType::Job, "https://example.com/agree");
        item.requirements = Some(text.to_string());
        let from_posting: Vec<String> = extract_uncached(&item)
            .into_iter()
            .filter(|s| s != "Infrastructure as Code")
            .collect();
        for skill in from_text(text, true) {
            assert!(
                from_posting.contains(&skill),
                "{skill} missing from {from_posting:?}"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ItemType;

    /// A distinct URL per fixture. `extract` memoizes on the URL — correctly,
    /// since it is the primary key in production — so two fixtures sharing one
    /// would have the second read back the first one's answer. A counter
    /// rather than a hash of the text, because tests legitimately build
    /// several items from the same title and body and then differ in the
    /// enriched fields, which the URL must still distinguish.
    fn posting(title: &str, body: &str) -> Item {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let url = format!(
            "https://example.com/{}",
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        Item::new(title, "Test", ItemType::Internship, url).with_content(body)
    }

    /// What the automaton replaced: one `contains` pass per keyword.
    fn naive_hits(text: &str, long: bool) -> Vec<bool> {
        catalog()
            .iter()
            .map(|(name, keywords)| {
                keywords_for(name, keywords, long)
                    .iter()
                    .any(|kw| text.contains(kw))
            })
            .collect()
    }

    #[test]
    fn the_automaton_agrees_with_the_scan_it_replaced() {
        // Extraction is a single Aho-Corasick pass now rather than ~600
        // independent substring searches. That is only worth doing if it is
        // the same answer, so this asserts it against the old implementation —
        // including on the cases where overlapping matters ("react" inside
        // "react native") and where the long-text tightening applies.
        let filler = "You will collaborate across teams and ship production services. ".repeat(12);
        let cases = [
            " backend engineering intern build rest apis in python with django and postgresql, \
              deploy on aws with docker and kubernetes, write unit tests with pytest. ",
            " react native and react and next.js and node.js and nodejs throughout. ",
            " we invite candidates who build trust, respect employment laws, design scalable \
              guardrails, and reason about scenarios end to end. ",
            " stack: go, python, c/c++, rstudio, sql, postgresql, mysql. ",
            " poetry reading night — bring a friend and a favourite verse. ",
        ];

        for case in cases {
            for long in [false, true] {
                assert_eq!(
                    matcher(long).hits(case),
                    naive_hits(case, long),
                    "long={long} disagreed on {case:?}"
                );
            }
            let padded = format!(" {filler} {case} ");
            assert_eq!(matcher(true).hits(&padded), naive_hits(&padded, true));
        }
    }

    #[test]
    fn every_keyword_still_finds_its_own_skill() {
        // A pattern that lost its owner in the flattening would silently stop
        // matching, and no fixture above would notice.
        for (skill, (name, keywords)) in catalog().iter().enumerate() {
            for keyword in keywords_for(name, keywords, false) {
                let text = format!("  {keyword}  ");
                assert!(
                    matcher(false).hits(&text)[skill],
                    "{name} did not match its own keyword {keyword:?}"
                );
            }
        }
    }

    #[test]
    fn every_skill_has_a_category() {
        for (_, skills) in SKILLS {
            for (name, _) in *skills {
                assert!(category_of(name).is_some(), "{name} has no category");
            }
        }
    }

    #[test]
    fn skill_names_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for (_, skills) in SKILLS {
            for (name, _) in *skills {
                assert!(seen.insert(*name), "duplicate skill name: {name}");
            }
        }
    }

    #[test]
    fn extracts_a_real_backend_posting() {
        let found = extract(&posting(
            "Backend Engineering Intern",
            "You will build REST APIs in Python with Django and PostgreSQL, deploy on AWS \
             with Docker and Kubernetes, and write unit tests with pytest.",
        ));

        for expected in [
            "Python",
            "Django",
            "PostgreSQL",
            "REST APIs",
            "AWS",
            "Docker",
            "Kubernetes",
            "Unit Testing",
        ] {
            assert!(
                found.iter().any(|s| s == expected),
                "missing {expected} in {found:?}"
            );
        }
    }

    #[test]
    fn extracts_nothing_from_an_unrelated_event() {
        let event = Item::new(
            "Poetry reading night",
            "Test",
            ItemType::Event,
            "https://example.com/poetry",
        )
        .with_content("Bring a friend and a favourite verse.");
        assert!(extract(&event).is_empty());
    }

    #[test]
    fn short_names_do_not_fire_on_ordinary_prose() {
        // The failure this guards: bare "go", "r" and "c" keywords matching
        // inside "great", "our" and "practice", handing every posting in the
        // cache three languages it never mentioned.
        let found = extract(&posting(
            "Marketing Intern",
            "A great opportunity to grow your career and practice our craft in a \
             collaborative and rigorous environment.",
        ));
        for never in ["Go", "R", "C", "C++", "Rust"] {
            assert!(
                !found.iter().any(|s| s == never),
                "false positive {never} in {found:?}"
            );
        }
    }

    #[test]
    fn keywords_do_not_fire_inside_longer_words() {
        // Every one of these was a real false positive during authoring:
        // "sql" inside "postgresql", "aws" inside "laws", "rust" inside
        // "trust", "scala" inside "scalable", "vite" inside "invite",
        // "ios" inside "scenarios", "rails" inside "guardrails".
        let found = extract(&posting(
            "Program Manager",
            "We invite candidates who build trust, respect employment laws, design scalable \
             guardrails, and reason about scenarios end to end.",
        ));
        for never in [
            "SQL",
            "AWS",
            "Rust",
            "Scala",
            "Webpack / Vite",
            "iOS",
            "Ruby on Rails",
        ] {
            assert!(
                !found.iter().any(|s| s == never),
                "false positive {never} in {found:?}"
            );
        }
    }

    #[test]
    fn a_database_name_is_not_also_the_language() {
        // PostgreSQL is its own skill; it must not also claim SQL.
        let found = extract(&posting(
            "Data Intern",
            "You will use PostgreSQL and MySQL daily.",
        ));
        assert!(found.iter().any(|s| s == "PostgreSQL"));
        assert!(found.iter().any(|s| s == "MySQL"));
        assert!(!found.iter().any(|s| s == "SQL"), "{found:?}");

        // Written on its own it still matches.
        let plain = extract(&posting(
            "Data Intern",
            "Strong SQL, and comfort with dashboards.",
        ));
        assert!(plain.iter().any(|s| s == "SQL"), "{plain:?}");
    }

    #[test]
    fn a_title_alone_claims_nothing() {
        // No description was fetched, so there is nothing to report. The job
        // title is not evidence: a "Frontend Engineer Intern" that never says
        // React must not be credited with it, because the reader cannot tell
        // a credited guess from a credited reading.
        assert!(extract(&posting(
            "Frontend Engineer Intern",
            "Location: Toronto, ON"
        ))
        .is_empty());
        assert!(extract(&posting("Internship at Skyscanner", "Location: London")).is_empty());
    }

    #[test]
    fn the_scraped_line_is_still_read() {
        // The unenriched sources give us one line. Whatever it names is a
        // reading like any other, so it counts.
        let found = extract(&posting(
            "Backend Engineer Intern",
            "You'll work in Rust on our Kafka pipeline.",
        ));
        assert!(found.iter().any(|s| s == "Rust"), "{found:?}");
        assert!(found.iter().any(|s| s == "Kafka"), "{found:?}");
        // ...and nothing the line did not name.
        assert!(!found.iter().any(|s| s == "REST APIs"), "{found:?}");
    }

    /// A posting shaped like a real enriched one: a role title and a long
    /// requirements block, rather than the line of location text the catalog
    /// was originally tuned against.
    fn enriched(title: &str, requirements: &str) -> Item {
        let mut item = posting(title, "Location: Toronto, ON");
        item.requirements = Some(requirements.to_string());
        item
    }

    #[test]
    fn a_fetched_posting_is_credited_with_nothing_it_did_not_say() {
        let found = extract(&enriched(
            "Frontend Engineer Intern",
            "You write Svelte and you have shipped something with Tailwind.",
        ));
        assert!(found.iter().any(|s| s == "Svelte"), "{found:?}");
        for unsaid in ["React", "Web Accessibility"] {
            assert!(
                !found.iter().any(|s| s == unsaid),
                "invented {unsaid} in {found:?}"
            );
        }
    }

    #[test]
    fn any_fetched_field_marks_the_posting_as_read() {
        // A page that gave us prose but no bullet lists still counts as read.
        // `from_posting` and the UI's wording must agree on that, or a posting
        // we did read would carry the "could not read this" note.
        for build in [
            |item: &mut Item| item.description = Some("We build data pipelines.".into()),
            |item: &mut Item| item.responsibilities = Some("Own a service end to end.".into()),
            |item: &mut Item| item.tagged_skills = vec!["Python".into()],
        ] {
            let mut item = posting("Frontend Engineer Intern", "Location: Toronto, ON");
            build(&mut item);
            assert!(from_posting(&item));
        }
    }

    #[test]
    fn requirements_are_read_alongside_the_description() {
        // Both fields are evidence. A posting that names its stack once, in
        // the opening paragraph, and never repeats it in a bullet is ordinary
        // — reading only the bullets used to drop those skills.
        let mut item = enriched(
            "Intern at Foo",
            "Experience with Kubernetes and Terraform. Familiarity with Kafka.",
        );
        item.description = Some("Our platform runs on Rust and Postgres.".into());

        let found = extract(&item);
        for expected in ["Kubernetes", "Terraform", "Kafka", "Rust", "PostgreSQL"] {
            assert!(
                found.iter().any(|s| s == expected),
                "missing {expected} in {found:?}"
            );
        }
    }

    #[test]
    fn perks_are_not_read_as_requirements() {
        // A benefits list says what the job pays, not what it asks for.
        let mut item = posting("Intern at Foo", "Location: Toronto, ON");
        item.perks = Some("Free Kubernetes training and a Python conference budget.".into());
        assert!(extract(&item).is_empty(), "{:?}", extract(&item));
    }

    #[test]
    fn ambiguous_names_do_not_fire_on_long_prose() {
        // The failure this guards, and the reason `AMBIGUOUS` exists: against
        // 8 KB of description, " go " matches "go to market" and " c " matches
        // any bullet list with a stray letter. Neither means the language.
        let prose = "We are looking for someone ready to go above and beyond as we go to \
             market with our platform. You will go deep on customer problems, and \
             go on to own a surface area of your own. Our R&D organisation values \
             engineers who can go from idea to launch. This is a chance to go \
             further than you have before, and to go where the work takes you. \
             We move fast and we go together, so bring a bias to go and ship."
            .repeat(2);
        let found = extract(&enriched("Program Manager Intern", &prose));

        for never in ["Go", "C", "R"] {
            assert!(
                !found.iter().any(|s| s == never),
                "false positive {never} in {found:?}"
            );
        }
    }

    #[test]
    fn ambiguous_names_still_fire_on_a_distinctive_mention() {
        // Padding out to long-text length so the tightened path is the one
        // under test, then naming the languages the way a posting really does.
        let filler = "You will collaborate across teams and ship production services. ".repeat(8);
        let found = extract(&enriched(
            "Intern at Foo",
            &format!("{filler} Requirements: strong Golang, C/C++ and RStudio experience."),
        ));

        for expected in ["Go", "C", "C++", "R"] {
            assert!(
                found.iter().any(|s| s == expected),
                "missing {expected} in {found:?}"
            );
        }
    }

    #[test]
    fn short_text_keeps_the_loose_keywords() {
        // The unenriched majority still relies on the padded forms.
        let found = extract(&posting("Intern at Foo", "Stack: Go, Python."));
        assert!(found.iter().any(|s| s == "Go"), "{found:?}");
    }

    #[test]
    fn tagged_skills_are_trusted_outright() {
        // "Data Visualization" is not in the catalog and is dropped; the rest
        // are taken without needing to appear in the text at all.
        let mut item = posting("Intern at Foo", "Location: Toronto, ON");
        item.tagged_skills = vec![
            "Python".into(),
            "Node.JS".into(),
            "postgresql".into(),
            "Data Visualization".into(),
        ];
        let found = extract(&item);

        for expected in ["Python", "Node.js", "PostgreSQL"] {
            assert!(
                found.iter().any(|s| s == expected),
                "missing {expected} in {found:?}"
            );
        }
        assert!(!found.iter().any(|s| s == "Data Visualization"));
    }

    #[test]
    fn canonical_normalizes_outside_vocabulary() {
        assert_eq!(canonical("Node.JS"), Some("Node.js"));
        assert_eq!(canonical("postgresql"), Some("PostgreSQL"));
        assert_eq!(canonical("  react  "), Some("React"));
        assert_eq!(canonical("C++"), Some("C++"));
        assert_eq!(canonical("C#"), Some("C#"));
        // Distinct skills must not collapse into each other.
        assert_eq!(canonical("C"), Some("C"));
        assert_eq!(canonical("Totally Made Up"), None);
        assert_eq!(canonical(""), None);
    }

    #[test]
    fn results_follow_catalog_order() {
        // Languages precede Cloud & DevOps in the table, so Python must come
        // out first however the posting orders them.
        let found = extract(&posting("Intern", "We use Kubernetes, and Python too."));
        let python = found.iter().position(|s| s == "Python").expect("python");
        let k8s = found
            .iter()
            .position(|s| s == "Kubernetes")
            .expect("kubernetes");
        assert!(python < k8s);
    }
}
