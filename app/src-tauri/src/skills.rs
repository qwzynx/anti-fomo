//! The software-engineering skill catalog, and extraction of the skills a
//! posting is asking for.
//!
//! Interests (`rank::INTERESTS`) say what the user wants to *read about*; these
//! say what they can *build*. The difference matters on the internships hub,
//! where two postings that both classify as "Backend" can want entirely
//! different stacks.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};

use crate::models::Item;

/// Category → skills → keywords.
///
/// The category is what the picker groups by and what a job detail labels its
/// chip rows with; the keywords decide whether a posting is asking for that
/// skill.
///
/// Keyword rules: matching is substring containment over a lowercased
/// haystack, so every keyword is either long enough to be distinctive or
/// written with a leading and/or trailing space (`" go "`, `" c "`, `" r "`).
/// A bare short token like `"go"` or `"r"` fires inside ordinary words —
/// "algorithms", "great" — and is never acceptable here.
///
/// Those outer spaces mean *word boundary*, not *literal space* — see
/// [`Pattern`]. `" rust "` therefore matches "(Rust)", "Rust/Go" and
/// "Rust-based" as well as "in Rust ", which is how postings actually write
/// it, while still refusing the "rust" inside "trust". A keyword with no
/// padding still matches anywhere, so "Dockerized" counts as Docker. The
/// tests at the bottom of this file guard both halves.
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

/// How hard a posting is asking for a skill.
///
/// "Preferred: Kubernetes" and "5+ years of production Kubernetes" are not the
/// same ask, and a panel that draws both as the same chip has thrown away the
/// distinction the reader is actually deciding on. So extraction reads the
/// posting a line at a time and keeps the context each mention arrived in.
///
/// Ordered strongest-first, and the ordering is load-bearing: a skill named in
/// several places keeps the strongest emphasis any one of them gave it.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Emphasis {
    /// In the requirements, in the title, or carrying a "must have" /
    /// "proficiency in" / "3+ years of" cue wherever it appeared.
    Must,
    /// Under a "nice to have" / "preferred" / "bonus" heading, or with one of
    /// those cues on its own line.
    Nice,
    /// In the duties or the blurb, with nothing marking it as an ask.
    Mentioned,
}

/// One skill a posting asks for, and how hard it asks.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Demand {
    pub skill: String,
    /// How much of the posting is about this skill. Only ever meaningful
    /// against the other demands of the *same* posting — a terse Greenhouse
    /// blurb and a 9 KB Workday page do not score on one scale — which is how
    /// every consumer uses it.
    pub weight: f32,
    pub emphasis: Emphasis,
}

/// What a line of a posting is doing with the skills it names.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tone {
    /// "Must have", "proficiency in", "4+ years of".
    Must,
    /// A requirements bullet with no cue either way.
    Plain,
    /// "Nice to have", "preferred", "a plus", "familiarity with".
    Nice,
    /// "You will learn", "no prior experience needed". The posting names the
    /// skill in order to say it is *not* asking for it.
    Learn,
}

/// Cues that mean the posting is offering the skill rather than asking for it.
///
/// Tried first, and tightly worded, because this is the only tone that can
/// remove a skill from the panel outright. "Willingness to learn Kubernetes"
/// is not a Kubernetes requirement; "exposure to Kubernetes" is a weak one,
/// and lives in [`NICE_CUES`] instead.
const LEARN_CUES: &[&str] = &[
    "you will learn",
    "you'll learn",
    "you’ll learn",
    "opportunity to learn",
    "chance to learn",
    "will be trained",
    "we will train",
    "we'll train",
    "we will teach",
    "we'll teach",
    "training provided",
    "training will be provided",
    "no prior experience",
    "no previous experience",
    "no experience with",
    "no experience in",
    "not required to have",
    "willingness to learn",
    "eagerness to learn",
    "eager to learn",
    "interest in learning",
    "excited to learn",
    "introduction to",
    "get exposure to",
    "you will be introduced",
];

/// Cues that mean the skill is wanted but not required.
const NICE_CUES: &[&str] = &[
    "nice to have",
    "nice-to-have",
    "preferred",
    "preferable",
    "preference will",
    "a plus",
    "big plus",
    "plus if",
    "bonus",
    "an asset",
    "is an asset",
    "considered an asset",
    "desirable",
    "advantageous",
    "would be great",
    "we'd love",
    "we’d love",
    "ideally",
    "optional",
    "not required",
    "familiarity with",
    "exposure to",
    "some experience",
    "any experience",
    "helpful but",
    "good to have",
];

/// Cues that mean the skill is a hard requirement, wherever the line sits.
const MUST_CUES: &[&str] = &[
    "must have",
    "must-have",
    "must be able",
    "you must",
    "is required",
    "are required",
    "required:",
    "requirement",
    "minimum qualification",
    "basic qualification",
    "essential",
    "proficien",
    "expertise",
    "expert in",
    "strong ",
    "solid ",
    "deep ",
    "extensive",
    "advanced knowledge",
    "in-depth",
    "hands-on experience",
    "demonstrated",
    "track record",
];

/// Whether a line asks for a number of years, which is the most reliable
/// "this is a hard requirement" mark a posting has. Written out rather than
/// pulled from `regex` because it runs once per line of every posting in an
/// 18,000-item cache and a scan for one word is cheaper than a match.
fn wants_years(line: &str) -> bool {
    let bytes = line.as_bytes();
    let mut from = 0;
    while let Some(at) = line[from..].find("year") {
        let at = from + at;
        let back = at.saturating_sub(8);
        if bytes[back..at].iter().any(u8::is_ascii_digit) {
            return true;
        }
        from = at + 4;
    }
    false
}

/// The three cue lists in one automaton, pattern index → what it means.
///
/// Read once per line of every posting in the cache, which is the reason this
/// is not three `iter().any(|cue| line.contains(cue))` passes: that is
/// seventy-odd scans of every line, and it cost half a second of the cold
/// ranking pass over a 13,000-item store all by itself.
fn cues() -> &'static (aho_corasick::AhoCorasick, Vec<Tone>) {
    static CUES: OnceLock<(aho_corasick::AhoCorasick, Vec<Tone>)> = OnceLock::new();
    CUES.get_or_init(|| {
        let mut patterns: Vec<&str> = Vec::new();
        let mut tones: Vec<Tone> = Vec::new();
        for (list, tone) in [
            (LEARN_CUES, Tone::Learn),
            (NICE_CUES, Tone::Nice),
            (MUST_CUES, Tone::Must),
        ] {
            for cue in list {
                patterns.push(cue);
                tones.push(tone);
            }
        }
        let automaton =
            aho_corasick::AhoCorasick::new(&patterns).expect("the cue lists build an automaton");
        (automaton, tones)
    })
}

/// Tones ordered by how specific they are, for picking between two on one line.
fn rank(tone: Tone) -> u8 {
    match tone {
        Tone::Learn => 0,
        Tone::Nice => 1,
        Tone::Must => 2,
        Tone::Plain => 3,
    }
}

/// Longest a line may be and still govern the ones under it. Borrowed from
/// `scrapers::sections`, and for the same reason: "Preferred qualifications"
/// is a heading, a 200-character paragraph mentioning a preference is not.
const MAX_CUE_HEADING: usize = 90;

/// Which part of the posting a line came from.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Field {
    Title,
    Requirements,
    Responsibilities,
    Description,
    /// The one line the unenriched sources give us. Read as an ask rather than
    /// a mention: it is everything we know about the posting, so a stack named
    /// there is the posting's stack.
    Scraped,
}

impl Field {
    /// How much a mention here is worth. The requirements are where a posting
    /// says what it wants; the blurb is where it says what the company does.
    fn base(self) -> f32 {
        match self {
            Field::Requirements => 3.0,
            Field::Title | Field::Scraped => 2.0,
            Field::Responsibilities => 1.6,
            Field::Description => 1.0,
        }
    }

    /// What an uncued mention here means.
    fn plain_emphasis(self) -> Emphasis {
        match self {
            Field::Requirements | Field::Title | Field::Scraped => Emphasis::Must,
            Field::Responsibilities | Field::Description => Emphasis::Mentioned,
        }
    }
}

/// What this posting asks for, in catalog order, with how hard it asks.
///
/// Read off the posting and nothing else: catalog keywords found in the
/// requirements, the duties and the description the enrichment pass fetched,
/// plus any skill the source tagged the posting with. A posting whose page we
/// could not read reports nothing, and the UI says so.
///
/// The reading is line by line rather than over one concatenated blob, because
/// where a mention sits is most of what it means. A line under "Preferred
/// qualifications" is a different ask from the same words under "Minimum
/// qualifications", and "you will learn Kubernetes" is the posting telling you
/// it is *not* asking for Kubernetes — which the blob read as a requirement
/// like any other.
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
pub fn demand(item: &Item) -> Vec<Demand> {
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

    let found = demand_uncached(item);
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

/// Which catalog skills this posting wants, in catalog order.
///
/// The names out of [`demand`], for the callers that only need the set —
/// `Item::required_skills`, the match intersection, the chips. Everything that
/// has to tell a must-have from a nice-to-have reads `demand` instead.
pub fn extract(item: &Item) -> Vec<String> {
    demand(item).into_iter().map(|d| d.skill).collect()
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

fn memo() -> &'static Mutex<HashMap<String, Vec<Demand>>> {
    static MEMO: OnceLock<Mutex<HashMap<String, Vec<Demand>>>> = OnceLock::new();
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

/// Bytes that continue a word, for deciding whether a keyword landed on one.
///
/// `+`, `#` and `&` are word bytes here even though they are punctuation
/// everywhere else, and each earns it: without `+` and `#` the keyword `" c "`
/// matches the "C" of "C++" and "C#", which are their own skills; without `&`
/// the keyword `" r "` matches the "R" of "R&D", which is not a language.
fn is_word(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'+' | b'#' | b'&')
}

/// One catalog keyword as the automaton stores it.
///
/// A leading or trailing space in a keyword means "a word boundary here", not
/// "a literal space" — which is what lets `" rust "` match "(Rust)", "Rust/Go"
/// and "Rust-based" as well as "in Rust ", all of which a literal space missed
/// and all of which are how postings really write it. Keywords with no padding
/// keep matching anywhere, so "Dockerized" still counts as Docker.
struct Pattern {
    /// Index into the flattened catalog.
    owner: usize,
    left: bool,
    right: bool,
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
    patterns: Vec<Pattern>,
}

impl Matcher {
    fn build(long: bool) -> Self {
        let mut literals: Vec<&str> = Vec::new();
        let mut patterns: Vec<Pattern> = Vec::new();
        for (skill, (name, keywords)) in catalog().iter().enumerate() {
            for keyword in keywords_for(name, keywords, long) {
                // Only the outer spaces are boundary marks; the one inside
                // "vue 3" or "shell script" is a space the posting has to have.
                let literal = keyword.trim_matches(' ');
                if literal.is_empty() {
                    continue;
                }
                literals.push(literal);
                patterns.push(Pattern {
                    owner: skill,
                    left: keyword.starts_with(' '),
                    right: keyword.ends_with(' '),
                });
            }
        }
        Matcher {
            automaton: aho_corasick::AhoCorasick::new(&literals)
                .expect("the catalog's keywords build an automaton"),
            patterns,
        }
    }

    /// Calls `on_hit(offset, skill)` for every keyword that lands on a word
    /// boundary, in increasing position — one pass over the whole text.
    ///
    /// Deliberately one call for a whole posting rather than one per line.
    /// Restarting the automaton per line cost 150 ms of the cold ranking pass
    /// over a 13,000-item store; the caller wants line context, and a binary
    /// search from the offset is far cheaper than the restarts.
    fn scan(&self, text: &str, mut on_hit: impl FnMut(usize, usize)) {
        let bytes = text.as_bytes();
        for m in self.automaton.find_overlapping_iter(text) {
            let pattern = &self.patterns[m.pattern().as_usize()];
            if pattern.left && m.start() > 0 && is_word(bytes[m.start() - 1]) {
                continue;
            }
            if pattern.right && m.end() < bytes.len() && is_word(bytes[m.end()]) {
                continue;
            }
            on_hit(m.start(), pattern.owner);
        }
    }

    /// Which catalog entries this text names, as a flag per skill.
    fn hits(&self, text: &str) -> Vec<bool> {
        let mut found = vec![false; catalog().len()];
        self.scan(text, |_, skill| found[skill] = true);
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
/// The posting side reaches this through [`demand`]; a résumé bullet reaches
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

/// One line of the posting, as a range into the buffer every field was
/// lowercased into. Ranges rather than strings so the whole read is one
/// allocation, which matters: this runs over every opportunity in the cache.
struct Line {
    span: std::ops::Range<usize>,
    field: Field,
}

/// Appends a field's lines to the buffer, recording where each one landed.
fn push_field(buf: &mut String, lines: &mut Vec<Line>, field: Field, text: &str) {
    // `scrapers::details::field` joins a section's lines with newlines, which
    // is exactly the structure this wants back: one bullet, one line.
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let start = buf.len();
        buf.push_str(trimmed);
        lines.push(Line {
            span: start..buf.len(),
            field,
        });
        buf.push('\n');
    }
}

/// Which line an offset falls in. The spans are sorted and disjoint, and every
/// keyword sits inside one of them because none of them contains a newline.
fn line_at(lines: &[Line], at: usize) -> Option<usize> {
    let index = lines.partition_point(|line| line.span.end <= at);
    lines
        .get(index)
        .filter(|line| line.span.contains(&at))
        .map(|_| index)
}

/// The tone every line carries, from one pass of the cue automaton over the
/// whole buffer. `None` means the line said nothing either way and should
/// inherit the heading above it.
fn tone_per_line(buf: &str, lines: &[Line]) -> Vec<Option<Tone>> {
    let mut tones: Vec<Option<Tone>> = vec![None; lines.len()];
    let (automaton, of_pattern) = cues();
    for m in automaton.find_overlapping_iter(buf) {
        let Some(index) = line_at(lines, m.start()) else {
            continue;
        };
        let tone = of_pattern[m.pattern().as_usize()];
        tones[index] = Some(match tones[index] {
            Some(current) if rank(current) <= rank(tone) => current,
            _ => tone,
        });
    }
    // "4+ years of Kubernetes" is a hard requirement, but a line that already
    // said "preferred" is still preferred however many years it names.
    for (index, line) in lines.iter().enumerate() {
        if tones[index].is_none() && wants_years(&buf[line.span.clone()]) {
            tones[index] = Some(Tone::Must);
        }
    }
    tones
}

fn demand_uncached(item: &Item) -> Vec<Demand> {
    // Everything the employer wrote, read together but not read as one thing.
    //
    // `perks` is deliberately absent: a benefits list is what the job pays,
    // not what it asks for, and it is where "compensation" language lives.
    //
    // `content_text` is the fallback for the sources the enrichment pass could
    // not reach — one scraped line, usually the office location, which is what
    // [`LONG_TEXT`] distinguishes from a real description below.
    let body = [
        (Field::Requirements, &item.requirements),
        (Field::Responsibilities, &item.responsibilities),
        (Field::Description, &item.description),
    ];
    let body_len: usize = body
        .iter()
        .flat_map(|(_, text)| text.iter())
        .map(|t| t.len() + 1)
        .sum();

    let mut buf = String::with_capacity(item.title.len() + body_len + item.content_text.len() + 3);
    let mut lines: Vec<Line> = Vec::new();
    push_field(&mut buf, &mut lines, Field::Title, &item.title);
    let title_end = buf.len();
    for (field, text) in body {
        if let Some(text) = text {
            push_field(&mut buf, &mut lines, field, text);
        }
    }
    if buf.len() == title_end {
        push_field(&mut buf, &mut lines, Field::Scraped, &item.content_text);
    }
    // Every catalog keyword is ASCII, so ASCII casefolding is exactly as
    // precise here as the Unicode one and does not have to reallocate. It also
    // leaves every byte offset alone, which is what makes the spans valid.
    buf.make_ascii_lowercase();
    let long = buf.len() > LONG_TEXT;

    // Two passes over the buffer, each of them one pass — which skills every
    // line names, and what tone every line carries — and then the read itself
    // in line order. The alternative, running both automatons per line, is the
    // same bytes and several hundred thousand more automaton restarts.
    let mut hits: Vec<(usize, usize)> = Vec::new();
    let mut last = vec![usize::MAX; catalog().len()];
    matcher(long).scan(&buf, |at, skill| {
        let Some(line) = line_at(&lines, at) else {
            return;
        };
        // A line naming Python twice is one line about Python.
        if last[skill] == line {
            return;
        }
        last[skill] = line;
        hits.push((line, skill));
    });
    // Matches arrive in increasing position and every keyword sits inside one
    // line, so this is already grouped by line — but sorting is cheap and the
    // grouping below must not depend on the automaton's iteration order.
    hits.sort_unstable();

    // Most of the cache is postings that name nothing the catalog knows —
    // events, articles, a listing whose page we never reached. Reading their
    // requirement cues is work with nowhere to land.
    if hits.is_empty() && item.tagged_skills.is_empty() {
        return Vec::new();
    }
    let tone = tone_per_line(&buf, &lines);

    let n = catalog().len();
    let mut weight = vec![0.0f32; n];
    let mut mentions = vec![0u32; n];
    let mut emphasis = vec![Emphasis::Mentioned; n];
    let mut present = vec![false; n];
    // A skill named only in a "you will learn this" line is not being asked
    // for, and reporting it as a requirement is how a posting that offers
    // training reads as one that demands it.
    let mut demanded = vec![false; n];

    let mut cursor = 0;
    let mut field = None;
    let mut sticky = Tone::Plain;
    for (index, line) in lines.iter().enumerate() {
        if field != Some(line.field) {
            field = Some(line.field);
            sticky = Tone::Plain;
        }
        let from = cursor;
        while cursor < hits.len() && hits[cursor].0 == index {
            cursor += 1;
        }
        let named = &hits[from..cursor];
        let own = tone[index];

        // A short line that names no skill and carries a cue is a heading, and
        // it governs the lines under it until the next one: "Preferred
        // qualifications:" is how most postings mark the half of the list that
        // is optional, and the bullets beneath it never repeat the word.
        if named.is_empty() {
            let text = &buf[line.span.clone()];
            if text.len() <= MAX_CUE_HEADING {
                match own {
                    // "Nice to have" and "You'll learn" open a block whether
                    // or not whoever wrote them reached for a colon.
                    Some(tone @ (Tone::Nice | Tone::Learn)) => sticky = tone,
                    // Any other heading closes the block above it. A cue-less
                    // short bullet — "Comfortable with ambiguity" — is not a
                    // heading and must leave the block it sits inside alone,
                    // or the second half of a preferred list reads as required.
                    _ if text.ends_with(':') => sticky = own.unwrap_or(Tone::Plain),
                    _ => {}
                }
            }
            continue;
        }

        let tone = match (own.unwrap_or(sticky), sticky) {
            // A "strong" or a "5+ years" inside a preferred block is still
            // preferred. The heading is the more reliable signal of the two.
            (Tone::Must, Tone::Nice) => Tone::Nice,
            (tone, _) => tone,
        };
        let factor = match tone {
            Tone::Must => 1.5,
            Tone::Plain => 1.0,
            Tone::Nice => 0.45,
            Tone::Learn => 0.1,
        };
        let said = match tone {
            Tone::Must => Emphasis::Must,
            Tone::Nice => Emphasis::Nice,
            Tone::Learn => Emphasis::Mentioned,
            Tone::Plain => line.field.plain_emphasis(),
        };

        for &(_, skill) in named {
            // Diminishing returns per mention: a posting that says "Python" in
            // nine bullets wants Python more than one that says it once, but
            // not nine times more, and without the damping a boilerplate-heavy
            // description outranks a terse one that is entirely about the job.
            weight[skill] += line.field.base() * factor / (1.0 + 0.5 * mentions[skill] as f32);
            mentions[skill] += 1;
            present[skill] = true;
            emphasis[skill] = emphasis[skill].min(said);
            demanded[skill] |= tone != Tone::Learn;
        }
    }

    // Skills the source tagged the posting with, where they name something the
    // catalog knows. Trusted outright — a tag is somebody's judgement about
    // the posting, not a substring that happened to appear in it.
    for tag in &item.tagged_skills {
        let Some(name) = canonical(tag) else { continue };
        let Some(at) = catalog().iter().position(|(n, _)| *n == name) else {
            continue;
        };
        weight[at] += Field::Requirements.base();
        present[at] = true;
        demanded[at] = true;
        emphasis[at] = Emphasis::Must;
    }

    catalog()
        .iter()
        .enumerate()
        .filter(|(skill, _)| present[*skill] && demanded[*skill])
        .map(|(skill, (name, _))| Demand {
            skill: name.to_string(),
            weight: weight[skill],
            emphasis: emphasis[skill],
        })
        .collect()
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
        let from_posting: Vec<String> = demand_uncached(&item)
            .into_iter()
            .map(|d| d.skill)
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

    /// What the automaton replaced, with the boundary rule spelled out: one
    /// `match_indices` pass per keyword, checking the ends by hand.
    fn naive_hits(text: &str, long: bool) -> Vec<bool> {
        catalog()
            .iter()
            .map(|(name, keywords)| {
                keywords_for(name, keywords, long).iter().any(|kw| {
                    let literal = kw.trim_matches(' ');
                    let bytes = text.as_bytes();
                    text.match_indices(literal).any(|(at, _)| {
                        let end = at + literal.len();
                        let left = !kw.starts_with(' ') || at == 0 || !is_word(bytes[at - 1]);
                        let right =
                            !kw.ends_with(' ') || end == bytes.len() || !is_word(bytes[end]);
                        left && right
                    })
                })
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

    fn demanded(item: &Item, skill: &str) -> Demand {
        demand(item)
            .into_iter()
            .find(|d| d.skill == skill)
            .unwrap_or_else(|| panic!("{skill} was not read off the posting"))
    }

    /// The line a skill sits on is most of what it means. Everything under
    /// "Preferred qualifications" is an ask the reader can fail and still be
    /// hired, and rendering it as the same chip as "Minimum qualifications"
    /// throws away the only thing they wanted to know.
    #[test]
    fn a_preferred_heading_makes_what_follows_optional() {
        let mut item = posting("Backend Intern", "Location: Toronto, ON");
        item.requirements = Some(
            "Minimum qualifications:\n             Experience building services in Python.\n             Preferred qualifications:\n             Experience operating Kubernetes.\n             Comfortable with ambiguity."
                .into(),
        );

        assert_eq!(demanded(&item, "Python").emphasis, Emphasis::Must);
        assert_eq!(demanded(&item, "Kubernetes").emphasis, Emphasis::Nice);
        assert!(
            demanded(&item, "Python").weight > demanded(&item, "Kubernetes").weight,
            "a preferred skill outweighed a required one"
        );
    }

    /// A cue-less bullet inside a preferred block must not end it. This used
    /// to promote the second half of every "nice to have" list to required.
    #[test]
    fn an_ordinary_bullet_does_not_close_a_preferred_block() {
        let mut item = posting("Backend Intern", "Location: Toronto, ON");
        item.requirements = Some(
            "Nice to have:\n             Curiosity and a bias to ship.\n             Experience operating Kubernetes."
                .into(),
        );
        assert_eq!(demanded(&item, "Kubernetes").emphasis, Emphasis::Nice);
    }

    /// A posting that offers to teach a skill is saying it is *not* asking for
    /// it, and reading that line as a requirement is how an internship written
    /// to be welcoming ends up looking like the hardest one in the feed.
    #[test]
    fn a_skill_the_posting_offers_to_teach_is_not_an_ask() {
        let mut item = posting("Platform Intern", "Location: Toronto, ON");
        item.requirements = Some(
            "Strong Python fundamentals.\n             You will learn Kubernetes and Terraform on the job — no prior \
             experience with either is expected."
                .into(),
        );
        let found = extract(&item);
        assert!(found.iter().any(|s| s == "Python"), "{found:?}");
        for taught in ["Kubernetes", "Terraform"] {
            assert!(
                !found.iter().any(|s| s == taught),
                "{taught} was read as a requirement in {found:?}"
            );
        }
    }

    /// A skill the posting only teaches is still dropped when it is the only
    /// thing said about it — but one that is *also* asked for elsewhere stays.
    #[test]
    fn a_taught_skill_still_counts_where_the_posting_also_asks_for_it() {
        let mut item = posting("Platform Intern", "Location: Toronto, ON");
        item.requirements = Some("Experience with Kubernetes in production.".into());
        item.description = Some("You will learn Kubernetes at a much bigger scale.".into());
        assert_eq!(demanded(&item, "Kubernetes").emphasis, Emphasis::Must);
    }

    /// The padded keywords used to need a literal space on both sides, so
    /// every one of these — all of them ordinary ways to write a stack — read
    /// as a posting that named no language at all.
    #[test]
    fn punctuation_no_longer_hides_a_language() {
        let found = extract(&posting(
            "Systems Intern",
            "Stack: Rust/Go, some Java-based services, and (TypeScript) on the client.",
        ));
        for expected in ["Rust", "Go", "Java", "TypeScript"] {
            assert!(
                found.iter().any(|s| s == expected),
                "missing {expected} in {found:?}"
            );
        }
    }

    /// Word boundaries are not the same as whitespace, and the difference is
    /// what keeps "C" out of "C++" and "R" out of "R&D".
    #[test]
    fn a_boundary_is_not_just_a_space() {
        let found = extract(&posting("Intern", "We write C++ in the R&D group."));
        assert!(found.iter().any(|s| s == "C++"), "{found:?}");
        assert!(!found.iter().any(|s| s == "C"), "C fired inside C++");
        assert!(!found.iter().any(|s| s == "R"), "R fired inside R&D");
    }

    /// Where a skill is named decides how much it counts for. The requirements
    /// are the employer saying what they want; the blurb is the employer
    /// describing itself.
    #[test]
    fn the_requirements_outweigh_the_blurb() {
        let mut item = posting("Intern at Foo", "Location: Toronto, ON");
        item.requirements = Some("Experience writing Python.".into());
        item.description = Some("Our platform is built on Kotlin.".into());

        assert!(demanded(&item, "Python").weight > demanded(&item, "Kotlin").weight);
        assert_eq!(demanded(&item, "Python").emphasis, Emphasis::Must);
        assert_eq!(demanded(&item, "Kotlin").emphasis, Emphasis::Mentioned);
    }

    /// "4+ years of Kubernetes" is the least ambiguous requirement mark a
    /// posting has, wherever the line it sits on happens to be.
    #[test]
    fn a_years_of_experience_line_is_a_hard_requirement() {
        let mut item = posting("Intern at Foo", "Location: Toronto, ON");
        item.responsibilities = Some("You will help out with Terraform.".into());
        item.description = Some("4+ years of hands-on Kubernetes.".into());

        assert_eq!(demanded(&item, "Terraform").emphasis, Emphasis::Mentioned);
        assert_eq!(demanded(&item, "Kubernetes").emphasis, Emphasis::Must);
    }

    /// Saying it nine times does not mean it nine times.
    #[test]
    fn repetition_has_diminishing_returns() {
        let mut once = posting("Intern at Foo", "Location: Toronto, ON");
        once.requirements = Some("Experience with Python.".into());
        let mut often = posting("Intern at Foo", "Location: Toronto, ON");
        often.requirements = Some(["Experience with Python."; 9].join("\n"));

        let one = demanded(&once, "Python").weight;
        let nine = demanded(&often, "Python").weight;
        assert!(nine > one, "more mentions did not count for more");
        assert!(nine < one * 4.0, "nine mentions counted like nine postings");
    }

    #[test]
    fn demands_follow_catalog_order() {
        let mut item = posting("Intern", "Location: Toronto, ON");
        item.requirements = Some("We use Kubernetes, and Python too.".into());
        let names: Vec<String> = demand(&item).into_iter().map(|d| d.skill).collect();
        let python = names.iter().position(|s| s == "Python").expect("python");
        let k8s = names
            .iter()
            .position(|s| s == "Kubernetes")
            .expect("kubernetes");
        assert!(python < k8s);
    }
}
