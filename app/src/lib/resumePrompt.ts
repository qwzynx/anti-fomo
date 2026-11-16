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
 * bullets per job when tailoring wants more than fit to choose from.
 *
 * So the prompt is generated, not written down. Two things in it have to track
 * the code rather than a copy of the code: the field list, which is exactly
 * what `resume::jsonresume::import` reads, and the skill vocabulary, which is
 * Rust-owned and arrives through `list_skills`. A bullet is matched to a
 * posting by intersecting the two sides' catalog skills, so a bullet written
 * as "Golang" when the catalog says "Go" is a match silently not made.
 *
 * The writing advice is deliberately field-agnostic. The reader may be in
 * software, nursing, finance or design, so the prompt asks for what makes any
 * bullet strong — an action, the means, a result that is in the files — and
 * for each role's bullets to cover different sides of the work, without
 * naming which sides. A list of software angles would push a non-software
 * history into a shape that is not true of it.
 *
 * It may sharpen wording, never facts: the assistant is working from the
 * user's files, and the prompt says so in as many words. This app never
 * rewords a bullet itself.
 */

/** How many bullets per substantial role the tailoring pass wants to choose from. */
const BULLETS_PER_ROLE = "6–10";

function vocabulary(catalog: SkillCategory[]): string {
  if (catalog.length === 0) return "";
  const lines = catalog.map((c) => `- ${c.name}: ${c.skills.join(", ")}`).join("\n");
  return `
## Spelling of tools and skills

The app matches a bullet to a posting partly by the skills both of them name,
using the vocabulary below. If I used one of these, spell it exactly as listed
("Go", not "Golang"; "PostgreSQL", not "Postgres"). Never add one I did not use,
and do not bend a bullet to fit the list — anything not on it is fine to write.

${lines}
`;
}

/** Builds the copy-pastable prompt against the live skill catalog. */
export function buildImportPrompt(catalog: SkillCategory[]): string {
  return `Help me turn my existing material into a master résumé, as a JSON Resume
document I can import into a résumé app.

The app keeps one master résumé and, for each job posting, picks the entries and
bullets that fit one page and best match that posting. So the master should hold
*more* than one page — every true, useful bullet — and the app does the cutting.

Read everything I have attached or written in this conversation (résumés, CVs,
LinkedIn exports, project notes, performance reviews, job descriptions of roles
I have held) and combine it into one document.

## Ground rules

- **Facts come only from my material.** Never invent an employer, title, date,
  number, tool or achievement. You may sharpen wording; you may not add claims.
- **Merge duplicates.** If the same role or project appears in several files,
  combine it into one entry and keep the most specific version of each detail.
- **Ask instead of guessing.** If something important is missing or unclear,
  leave the field out and list your questions *after* the JSON.
- **Reply with exactly one \`\`\`json code block** containing only the document:
  valid JSON, no comments, no trailing commas, no placeholder text. Omit any
  field you have nothing for rather than leaving it empty.

## What makes a strong bullet

- One sentence, starting with a strong verb (past tense; present tense for a role
  I still hold). No "Responsible for", no "Helped with" when I did the thing.
- Say **what I did, how** (the method, tool or skill), **and what came of it** —
  scale, outcome, or who benefited.
- Use numbers when my material has them (people, money, time, volume,
  percentages). Never estimate or invent one; a bullet without a number is fine.
- Specific beats general: name the actual tool, process, audience or result.
- Each bullet stands on its own — no "also", no references to other bullets.

## Cover the whole role, not one side of it

Different postings look for different things, so give each role bullets that
show *different* sides of it: the core work, the tools or methods used, results
achieved, problems solved, people led or worked with, things improved or built
from scratch. If two bullets would appeal to exactly the same posting and say
nothing different, merge them.

Aim for about ${BULLETS_PER_ROLE} bullets for each substantial job or project, strongest
first, and fewer for smaller ones — only as many as my material truthfully
supports. Include every role, project and activity, even ones I might cut from
a one-page résumé; a bullet that is not in the master can never be chosen.

## The shape

Only these keys are read; anything else is ignored.

\`\`\`json
{
  "basics": {
    "name": "", "label": "", "email": "", "phone": "", "url": "",
    "location": { "city": "", "region": "" },
    "profiles": [{ "network": "LinkedIn", "username": "", "url": "" }]
  },
  "education": [{
    "institution": "", "studyType": "", "area": "", "location": "",
    "startDate": "", "endDate": "", "score": "", "highlights": []
  }],
  "work": [{
    "name": "", "position": "", "location": "", "url": "", "keywords": [],
    "startDate": "", "endDate": "", "highlights": []
  }],
  "projects": [{
    "name": "", "url": "", "keywords": [],
    "startDate": "", "endDate": "", "highlights": []
  }],
  "volunteer": [{
    "organization": "", "position": "", "location": "", "keywords": [],
    "startDate": "", "endDate": "", "highlights": []
  }],
  "awards": [{ "title": "", "awarder": "", "date": "" }],
  "skills": [{ "name": "", "keywords": [] }]
}
\`\`\`

How the app reads it:

- **Dates are printed exactly as written**, never parsed. Write them as they
  should appear, consistently: "May 2025", "Present", "Expected Apr 2027".
- **Every bullet goes in \`highlights\`.** Do not use \`summary\` or
  \`description\` — those become one oversized bullet.
- \`basics.label\` is the one line under my name (e.g. "Mechanical Engineering
  Student", "Registered Nurse"). Leave it out if my material does not support one.
- \`education[].score\` is a single detail line under the degree: GPA, honours,
  or a distinction. Relevant coursework or a thesis can go in its \`highlights\`.
- \`keywords\` on a project, a job or an activity is its "Skills & tools" line,
  printed under the role: the main tools, methods or skills that work used.
  Keep it to the few that matter; the app can leave any of them out per job.
- \`volunteer\` is printed as Leadership & Activities — clubs, societies,
  volunteering, mentoring, organising. Keep \`work\` for paid or formal roles,
  including internships and co-ops.
- \`skills\`: a few groups (about 3–6), each named for how an employer would look
  for it — e.g. "Languages", "Tools", "Lab Techniques", "Certifications" — with
  the most relevant items first. Only skills my material shows I have.
- Sections print in this order: Education, Experience, Projects, Leadership,
  Awards, Skills. Within a section, list the most recent first.
${vocabulary(catalog)}
## Before you reply

Check that the JSON parses, that every bullet traces back to my material, that
nothing was invented, that dates are written consistently, and that no role has
two bullets making the same point.`;
}
