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
            ("SQL", &[" sql ", " sql,", " sql.", " sql/", "t-sql", "pl/sql"]),
            ("R", &[" r ", " r,", " r/", "rstudio", "tidyverse"]),
            ("MATLAB", &["matlab", "simulink"]),
            ("Shell / Bash", &["bash", "shell script", "zsh", "powershell"]),
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
            ("Webpack / Vite", &["webpack", " vite ", " vite,", "vite.config", "esbuild"]),
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
            ("Express", &["express.js", "expressjs", "express framework", "express server"]),
            ("Django", &["django"]),
            ("Flask", &["flask"]),
            ("FastAPI", &["fastapi"]),
            ("Spring Boot", &["spring boot", "spring framework", "springboot"]),
            (".NET", &[".net", "dotnet", "asp.net"]),
            // Not a bare "rails ", which fires inside "guardrails".
            ("Ruby on Rails", &["ruby on rails", "rails app", "activerecord"]),
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
            ("Apache Spark", &["apache spark", "pyspark", "spark sql", "spark job"]),
            ("Kafka", &["kafka", "kinesis", "event streaming"]),
            (
                "ETL / Pipelines",
                &[" etl ", " elt ", "etl pipeline", "data pipeline", "ingestion pipeline"],
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
                &["natural language processing", " nlp ", "text classification"],
            ),
            (
                "Computer Vision",
                &["computer vision", "image recognition", "opencv", "object detection"],
            ),
            (
                "LLMs",
                &["large language model", " llm ", "llms", "gpt-", "prompt engineering"],
            ),
            (
                "RAG",
                &["retrieval-augmented", " rag ", "vector database", "embeddings"],
            ),
            ("Hugging Face", &["hugging face", "huggingface", "transformers library"]),
            ("MLOps", &["mlops", "model serving", "model deployment", "feature store"]),
            ("Reinforcement Learning", &["reinforcement learning", " rlhf ", "policy gradient"]),
        ],
    ),
    (
        "Cloud & DevOps",
        &[
            // " aws" rather than "aws", which fires inside "laws" and "flaws".
            (
                "AWS",
                &[" aws ", " aws,", " aws.", " aws/", "amazon web services", "ec2 ", "lambda function"],
            ),
            ("Azure", &["azure"]),
            ("GCP", &["gcp", "google cloud"]),
            ("Docker", &["docker", "containeriz"]),
            ("Kubernetes", &["kubernetes", "k8s", "helm chart"]),
            ("Terraform", &["terraform", "pulumi", "infrastructure as code"]),
            ("CI/CD", &["ci/cd", "continuous integration", "continuous deployment"]),
            ("GitHub Actions", &["github actions"]),
            ("Jenkins", &["jenkins", "circleci", "gitlab ci", "teamcity"]),
            ("Linux", &["linux", "unix", "ubuntu", "debian"]),
            ("Nginx", &["nginx", "apache http", "load balanc", "reverse proxy"]),
            ("Serverless", &["serverless", "cloud function", "edge function"]),
            (
                "Observability",
                &["observability", "prometheus", "grafana", "datadog", "opentelemetry"],
            ),
            ("Ansible", &["ansible", "puppet", " chef "]),
        ],
    ),
    (
        "Systems & Low-level",
        &[
            ("Operating Systems", &["operating system", "kernel", "scheduler design"]),
            ("Compilers", &["compiler", "llvm", "interpreter design", "parser generator"]),
            ("Embedded / Firmware", &["embedded", "firmware", "microcontroller", "rtos"]),
            (
                "Concurrency",
                &["concurrency", "multithread", "parallel programming", "async runtime"],
            ),
            ("Networking", &["tcp/ip", "networking protocol", "socket programming", " dns "]),
            ("Memory Management", &["memory management", "memory safety", "garbage collect"]),
            ("Assembly", &["assembly language", "x86 assembly", "arm assembly"]),
            ("FPGA / Verilog", &["fpga", "verilog", "vhdl", "hardware description"]),
            ("Robotics / ROS", &["robotics", " ros ", "ros2", "motion planning"]),
            ("Real-time Systems", &["real-time system", "low latency", "deterministic latency"]),
        ],
    ),
    (
        "Security",
        &[
            (
                "Application Security",
                &["application security", "appsec", "owasp", "secure development"],
            ),
            ("Cryptography", &["cryptograph", "encryption", " tls ", "public key"]),
            (
                "Penetration Testing",
                &["penetration test", "pentest", "red team", "ethical hacking"],
            ),
            ("Threat Modeling", &["threat model", "risk assessment", "attack surface"]),
            ("OAuth / Identity", &["oauth", "openid", " saml ", "single sign-on", " iam "]),
            ("Reverse Engineering", &["reverse engineer", "binary analysis", "disassembl"]),
            ("Incident Response", &["incident response", " soc ", "siem", "threat detection"]),
            ("Secure Code Review", &["secure code review", "static analysis", " sast ", " dast "]),
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
            ("Code Review", &["code review", "pull request", "peer review"]),
            ("Unit Testing", &["unit test", "jest", "pytest", "junit"]),
            (
                "Integration Testing",
                &["integration test", "end-to-end test", "e2e test", "cypress", "playwright"],
            ),
            ("TDD", &["test-driven", " tdd ", "behaviour-driven", " bdd "]),
            ("Debugging", &["debugging", "troubleshoot", "root cause analysis"]),
            (
                "System Design",
                &["system design", "software architecture", "design pattern", "scalable system"],
            ),
            (
                "Data Structures & Algorithms",
                &["data structures", "algorithms", "complexity analysis"],
            ),
            // Not a bare "documentation", which appears in almost every
            // posting and would put this chip on all of them.
            (
                "Technical Writing",
                &["technical writing", "technical documentation", "write documentation"],
            ),
            ("Jira", &["jira", "confluence", "linear.app"]),
            ("Figma", &["figma", "design system", "wireframe"]),
        ],
    ),
];

/// Skills a role is understood to want, keyed by phrases in the job title.
///
/// This exists because of what the sources actually give us. Every opportunity
/// scraper reads a *list* endpoint — a GitHub table, a board's index page — so
/// `content_text` arrives at 24-70 characters and is mostly the office
/// location. Measured over a full cache of 1,535 postings, literal keyword
/// extraction fired on 4% of them. Fetching 1,500 description pages per
/// refresh is not an option this app has.
///
/// Titles are the signal that survives: 59% name a recognisable role. So a
/// "Frontend Engineer Intern" is credited with what frontend work wants, and
/// the UI says "typically wanted" rather than claiming the posting named them.
/// The remaining 41% — bare "Internship at Foo" — get nothing, which is the
/// honest answer for a posting that told us nothing.
///
/// Order is longest-phrase-first within a family so "data engineer" is not
/// swallowed by "data". Every profile that matches contributes, so a
/// "Full-Stack Software Engineer" collects all three sets.
pub const ROLES: &[(&[&str], &[&str])] = &[
    // Generic engineering, deliberately small: it matches ~45% of the corpus,
    // and a large set here would make every posting look identical.
    (
        &["software engineer", "software developer", "software dev "],
        &["Data Structures & Algorithms", "Git", "Code Review", "Debugging", "Unit Testing"],
    ),
    (
        &["frontend", "front-end", "front end", "ui engineer", "web develop"],
        &["JavaScript", "TypeScript", "React", "HTML / CSS", "Web Accessibility"],
    ),
    (
        &["backend", "back-end", "back end", "server-side"],
        &["REST APIs", "SQL", "PostgreSQL", "Microservices", "Docker"],
    ),
    (
        &["full-stack", "full stack", "fullstack"],
        &["JavaScript", "TypeScript", "React", "REST APIs", "SQL"],
    ),
    (
        &["machine learning", "deep learning", " ml ", " ai ", "artificial intelligence"],
        &["Python", "PyTorch", "Deep Learning", "scikit-learn", "Pandas"],
    ),
    (
        &["data scien", "data analy", "analytics"],
        &["Python", "SQL", "Pandas", "Data Structures & Algorithms"],
    ),
    (
        &["data engineer"],
        &["Python", "SQL", "ETL / Pipelines", "Apache Spark", "Airflow"],
    ),
    (
        &["devops", "site reliability", " sre ", "platform engineer", "cloud engineer", "infrastructure engineer"],
        &["Docker", "Kubernetes", "AWS", "CI/CD", "Terraform", "Linux"],
    ),
    (
        &["security engineer", "cyber", "infosec", "application security"],
        &["Application Security", "Cryptography", "Threat Modeling", "Linux"],
    ),
    (
        &["mobile", "ios engineer", "ios develop", "android engineer", "android develop"],
        &["Swift", "Kotlin", "iOS", "Android"],
    ),
    (
        &["embedded", "firmware"],
        &["C", "C++", "Embedded / Firmware", "Operating Systems"],
    ),
    (
        &["hardware engineer", "asic", "silicon", "chip design"],
        &["FPGA / Verilog", "C", "Assembly"],
    ),
    (
        &["systems engineer", "kernel", "compiler"],
        &["C", "C++", "Operating Systems", "Concurrency"],
    ),
    (
        &["qa engineer", " qa ", "test engineer", "quality assurance"],
        &["Unit Testing", "Integration Testing", "Debugging", "TDD"],
    ),
    (
        &["game develop", "game engineer", "gameplay"],
        &["C++", "Data Structures & Algorithms", "WebGL"],
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
/// Two passes, unioned:
///
/// 1. **Literal** — catalog keywords appearing in the title or body. Precise,
///    but our sources give so little body text that it alone covers 4% of the
///    cache.
/// 2. **Role** — what the role named in the title typically wants, from
///    [`ROLES`]. This is what makes the feature work on real data, and it is
///    why the UI is worded "typically wanted" rather than "required".
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

/// Roughly twice a full cache, so ordinary use never trips the reset.
const MEMO_CAPACITY: usize = 8000;

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
    ("RAG", &["retrieval-augmented", "vector database", "embeddings"]),
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

/// Above this many characters, the haystack is a real description rather than
/// a line of scraped metadata, and [`AMBIGUOUS`] skills tighten up.
const LONG_TEXT: usize = 400;

fn extract_uncached(item: &Item) -> Vec<String> {
    // Prefer what the posting actually asks for. The full description is
    // mostly company boilerplate, which is where false positives live, so it
    // is only the fallback — and `content_text` the fallback below that, for
    // the sources we cannot fetch.
    let mut body = String::new();
    for text in [&item.requirements, &item.responsibilities].into_iter().flatten() {
        body.push(' ');
        body.push_str(text);
    }
    if body.trim().is_empty() {
        if let Some(text) = &item.description {
            body.push_str(text);
        }
    }
    if body.trim().is_empty() {
        body.push_str(&item.content_text);
    }

    // Padded so keywords written with leading/trailing spaces (" go ") can
    // match at the very start or end of the text.
    let text = format!(" {} {} ", item.title, body).to_lowercase();
    let title = format!(" {} ", item.title).to_lowercase();
    let long = text.len() > LONG_TEXT;

    let mut wanted: std::collections::HashSet<&str> = SKILLS
        .iter()
        .flat_map(|(_, skills)| skills.iter())
        .filter(|(name, keywords)| {
            keywords_for(name, keywords, long)
                .iter()
                .any(|kw| text.contains(kw))
        })
        .map(|(name, _)| *name)
        .collect();

    // Skills the source tagged the posting with, where they name something the
    // catalog knows. Trusted outright — a tag is somebody's judgement about
    // the posting, not a substring that happened to appear in it.
    for tag in &item.tagged_skills {
        if let Some(name) = canonical(tag) {
            wanted.insert(name);
        }
    }

    for (phrases, skills) in ROLES {
        if phrases.iter().any(|p| title.contains(p)) {
            wanted.extend(skills.iter().copied());
        }
    }

    // Re-walk the catalog rather than draining the set, so the result is in
    // catalog order and therefore stable between calls.
    SKILLS
        .iter()
        .flat_map(|(_, skills)| skills.iter())
        .filter(|(name, _)| wanted.contains(name))
        .map(|(name, _)| name.to_string())
        .collect()
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
        let url = format!("https://example.com/{}", NEXT.fetch_add(1, Ordering::Relaxed));
        Item::new(title, "Test", ItemType::Internship, url).with_content(body)
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
            assert!(found.iter().any(|s| s == expected), "missing {expected} in {found:?}");
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
            assert!(!found.iter().any(|s| s == never), "false positive {never} in {found:?}");
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
        for never in ["SQL", "AWS", "Rust", "Scala", "Webpack / Vite", "iOS", "Ruby on Rails"] {
            assert!(!found.iter().any(|s| s == never), "false positive {never} in {found:?}");
        }
    }

    #[test]
    fn a_database_name_is_not_also_the_language() {
        // PostgreSQL is its own skill; it must not also claim SQL.
        let found = extract(&posting("Data Intern", "You will use PostgreSQL and MySQL daily."));
        assert!(found.iter().any(|s| s == "PostgreSQL"));
        assert!(found.iter().any(|s| s == "MySQL"));
        assert!(!found.iter().any(|s| s == "SQL"), "{found:?}");

        // Written on its own it still matches.
        let plain = extract(&posting("Data Intern", "Strong SQL, and comfort with dashboards."));
        assert!(plain.iter().any(|s| s == "SQL"), "{plain:?}");
    }

    #[test]
    fn every_role_names_real_catalog_skills() {
        // A typo here would silently give a role a skill the picker can never
        // offer, so the user could never match it.
        for (phrases, skills) in ROLES {
            for skill in *skills {
                assert!(
                    category_of(skill).is_some(),
                    "role {phrases:?} names unknown skill {skill}"
                );
            }
        }
    }

    #[test]
    fn a_role_in_the_title_stands_in_for_the_missing_description() {
        // The real shape of our data: a title, and a body that is just the
        // office location.
        let found = extract(&posting("Frontend Engineer Intern", "Location: Toronto, ON"));
        for expected in ["JavaScript", "TypeScript", "React", "HTML / CSS"] {
            assert!(found.iter().any(|s| s == expected), "missing {expected} in {found:?}");
        }
    }

    #[test]
    fn a_titleless_posting_claims_nothing() {
        // 41% of the corpus looks like this. Inventing skills for it would
        // put a meaningless match score on every one of them.
        assert!(extract(&posting("Internship at Skyscanner", "Location: London")).is_empty());
    }

    #[test]
    fn roles_compound_without_duplicating() {
        let found = extract(&posting("Full Stack Software Engineer", "Location: Remote"));
        // Both the generic and the full-stack profile contributed.
        assert!(found.iter().any(|s| s == "Git"));
        assert!(found.iter().any(|s| s == "React"));

        let mut deduped = found.clone();
        deduped.sort();
        deduped.dedup();
        assert_eq!(deduped.len(), found.len(), "duplicate skills in {found:?}");
    }

    #[test]
    fn literal_hits_still_win_where_there_is_real_text() {
        // Role inference is a fallback, not a replacement: a posting that does
        // describe itself gets both.
        let found = extract(&posting(
            "Backend Engineer Intern",
            "You'll work in Rust on our Kafka pipeline.",
        ));
        assert!(found.iter().any(|s| s == "Rust"), "{found:?}");
        assert!(found.iter().any(|s| s == "Kafka"), "{found:?}");
        assert!(found.iter().any(|s| s == "REST APIs"), "{found:?}");
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
    fn requirements_are_preferred_over_the_scraped_line() {
        let found = extract(&enriched(
            "Intern at Foo",
            "Experience with Kubernetes and Terraform. Familiarity with Kafka.",
        ));
        for expected in ["Kubernetes", "Terraform", "Kafka"] {
            assert!(found.iter().any(|s| s == expected), "missing {expected} in {found:?}");
        }
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
            assert!(!found.iter().any(|s| s == never), "false positive {never} in {found:?}");
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
            assert!(found.iter().any(|s| s == expected), "missing {expected} in {found:?}");
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
            assert!(found.iter().any(|s| s == expected), "missing {expected} in {found:?}");
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
        let k8s = found.iter().position(|s| s == "Kubernetes").expect("kubernetes");
        assert!(python < k8s);
    }
}
