import type { SkillCategory } from "./api";

/**
 * The prompt the user hands to a chat assistant to turn the documents they
 * already have into a `resume.json` this app can import.
 *
 * The first five minutes of a résumé builder are the whole battle, and the
 * honest state of things is that most people's work history lives in a chat
 * thread with their old CV, a project README and a performance review already
 * attached. Asking them to retype that here loses; asking them to type
 * "convert this to JSON Resume" loses differently, because a free-form answer
 * gets the parts this app is picky about wrong — dates it tries to normalise,
 * prose in `summary` that lands on the page as a stray bullet, three tidy
 * bullets per job when tailoring wants twelve to choose from.
 *
 * So the prompt is generated, not written down. Two things in it have to track
 * the code rather than a copy of the code: the field list, which is exactly
 * what `resume::jsonresume::import` reads, and the skill vocabulary, which is
 * Rust-owned and arrives through `list_skills`. A bullet is matched to a
 * posting by intersecting the two sides' catalog skills, so a bullet written
 * as "Golang" when the catalog says "Go" is a match silently not made.
 *
 * Nothing here asks for wording help with what the user did — the assistant is
 * transcribing their files into a shape, and the prompt says so in as many
 * words. This app never rewords a bullet either.
 */

/** How many bullets per role the tailoring pass wants to choose from. */
const BULLETS_PER_ROLE = "8–12";

function vocabulary(catalog: SkillCategory[]): string {
  if (catalog.length === 0) return "";
  const lines = catalog.map((c) => `- ${c.name}: ${c.skills.join(", ")}`).join("\n");
  return `
## Name technologies the way the app names them

The app scores a bullet against a posting by intersecting the skills each one
names, over this fixed vocabulary. Where one of these is what I actually used,
spell it exactly like this — "Go", not "Golang"; "PostgreSQL", not "Postgres".

${lines}

Never force one of these into a bullet that is not true. Anything outside the
list is fine to write; it simply does not count towards a posting's match.
`;
}

/** Builds the copy-pastable prompt against the live skill catalog. */
export function buildImportPrompt(catalog: SkillCategory[]): string {
  return `I am filling in the résumé builder in an app called Anti-FOMO. It takes one
master résumé and, for each job posting I open, picks the bullets that fit the
page and speak to that posting. It never rewrites my words.

Read every file I have attached or written in this conversation — résumés, CVs,
project notes, job descriptions I have held, anything — and turn them into one
JSON Resume document I can paste into the app.

## Ground rules

- Every fact comes from my files. Do not invent an employer, a date, a metric or
  a technology. If a bullet has no number in my files, write the bullet without
  a number.
- Reply with exactly one \`\`\`json code block and nothing inside it but the
  document: no comments, no trailing commas, no placeholder text.
- If something important is missing or ambiguous, ask me about it *after* the
  code block, and leave the field out rather than guessing.

## The shape

Only these keys are read. Anything else is ignored.

\`\`\`json
{
  "basics": {
    "name": "", "label": "", "email": "", "phone": "", "url": "",
    "location": { "city": "", "region": "" },
    "profiles": [{ "network": "GitHub", "username": "", "url": "" }]
  },
  "education": [{
    "institution": "", "studyType": "", "area": "", "location": "",
    "startDate": "", "endDate": "", "score": "", "highlights": []
  }],
  "work": [{
    "name": "", "position": "", "location": "", "url": "",
    "startDate": "", "endDate": "", "highlights": []
  }],
  "projects": [{
    "name": "", "url": "", "keywords": [],
    "startDate": "", "endDate": "", "highlights": []
  }],
  "volunteer": [{
    "organization": "", "position": "", "location": "",
    "startDate": "", "endDate": "", "highlights": []
  }],
  "awards": [{ "title": "", "awarder": "", "date": "" }],
  "skills": [{ "name": "Languages", "keywords": [] }]
}
\`\`\`

How the importer reads it, which is worth knowing before you fill it in:

- **Dates are printed exactly as written and never parsed.** Write them the way
  they should appear on the page: "May 2025", "Aug 2025", "Present",
  "Expected Apr 2027".
- **Every bullet goes in \`highlights\`.** Do not use \`summary\` or
  \`description\`: the importer turns those into bullets too, so a paragraph
  there lands on the page as one enormous bullet.
- \`education[].score\` is the single detail line under a degree — "GPA 3.9/4.0",
  "Dean's List 2024".
- \`projects[].keywords\` is the stack line under a project title, so put the
  technologies there — "Rust", "Tauri", "SQLite" — rather than in a bullet whose
  only content is a list.
- \`skills\` groups render one line each, label then run: 4–6 groups of 6–10
  keywords, named for how I would be searched for ("Languages", "Backend",
  "Cloud & DevOps"), not one group of forty.
- \`volunteer\` becomes the Leadership & Activities section — clubs, teaching,
  hackathon organising, society work. Keep \`work\` for paid or formal roles.
- \`basics.label\` is the one line printed under my name ("Software Engineer").
  Leave it out if my files do not support one.
- Sections print in this order: Education, Experience, Projects, Leadership,
  Awards, Skills.

## Write a superset, not a finished page

This is the part that matters most, and it is the opposite of normal résumé
advice. The app trims to fit and chooses per posting, so give it more material
than fits on one page and let it choose.

- ${BULLETS_PER_ROLE} highlights for every job and substantial project, strongest first.
- One sentence each, self-contained, opening with a verb — past tense, or
  present for a role I still hold — and naming the concrete technology used.
- **No two bullets in the same role may be interchangeable.** Each one is aimed
  at a different kind of job, so that a different posting can reach for it:

  - one backend — the service, the API, the data model
  - one frontend or UI — what a user actually saw
  - one data or database — the schema, the query, the pipeline
  - one infrastructure — deployment, CI, containers, cloud
  - one security or correctness — auth, validation, a class of bug closed off
  - one performance or scale — the number that moved, and how
  - one testing, debugging or ownership — the failure I chased down
  - one collaboration, mentoring or process — code review, planning, handover
  - one product or impact — who used it and what changed for them

  Take only the angles that role honestly had. A role with six true bullets gets
  six; do not pad it to a full set, and never invent an angle I did not work on.
- Read the set back before moving on: if two bullets would both be picked by the
  same posting and neither says anything the other does not, merge them and
  write a different angle instead. Different verb, different technology,
  different part of the work — that is what makes them distinct.
- Include every job, project and activity in my files, even the ones I would cut
  from a one-page résumé. Cutting happens in the app, per posting, and a bullet
  that is not in the document can never be chosen.
${vocabulary(catalog)}
## Before you send it

Check the JSON parses. Check every highlight traces back to something in my
files. Check no employer, title or date was invented. Check each substantial
role has at least eight highlights where the material honestly supports it, and
that no two of them are the same claim with different words.`;
}
