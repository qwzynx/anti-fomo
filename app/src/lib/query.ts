/**
 * Text search over the cached lists, shared by the ⌘K palette and Roles.
 *
 * The palette used to match the whole query as one literal substring — "react
 * toronto" found only postings containing that exact phrase — and Roles had no
 * text search at all. This is one reader for both:
 *
 *   react toronto          every word must appear (in any field, any order)
 *   "machine learning"     a phrase, as whole words
 *   "java"                 a whole word only — not "javascript"
 *   -senior                must not appear
 *   company:shopify        the employer (also `at:`); quote a multi-word name
 *
 * An unquoted word matches at the *start* of a word in the posting, so "react"
 * finds "React" and "React Native", "eng" finds "engineer", and "ai" does not
 * fire inside "maintain". A title or employer hit outranks one that only turns
 * up in the body.
 *
 * Nothing per keystroke builds a string per item: `indexOf` casefolds each
 * posting once per list snapshot (the lists are replaced whole, never edited),
 * and memoizes on the array.
 */

import type { ScrapedItem } from "./api";
import { companyOf } from "./item";

export interface ParsedQuery {
  /** Plain words; each must match at a word start somewhere. */
  terms: string[];
  /** Quoted words and phrases; each must appear as whole words. */
  phrases: string[];
  /** Words or phrases that must not appear anywhere. */
  excludes: string[];
  /** Employer filters (`company:` / `at:`); any one may match. */
  companies: string[];
  /** Nothing to filter on. */
  empty: boolean;
}

const COMPANY_PREFIXES = ["company:", "at:"];

/** Splits on whitespace, keeping quoted runs (and a prefix glued to one) whole. */
function tokenize(raw: string): string[] {
  const tokens: string[] = [];
  let current = "";
  let quoted = false;
  for (const ch of raw) {
    if (ch === '"') {
      quoted = !quoted;
      current += ch;
    } else if (!quoted && /\s/.test(ch)) {
      if (current) tokens.push(current);
      current = "";
    } else {
      current += ch;
    }
  }
  if (current) tokens.push(current);
  return tokens;
}

function unquote(token: string): { text: string; quoted: boolean } {
  const quoted = token.startsWith('"');
  return { text: token.replace(/"/g, "").trim(), quoted };
}

export function parseQuery(raw: string): ParsedQuery {
  const q: ParsedQuery = { terms: [], phrases: [], excludes: [], companies: [], empty: true };
  for (let token of tokenize(raw.toLowerCase())) {
    let negate = false;
    if (token.startsWith("-") && token.length > 1) {
      negate = true;
      token = token.slice(1);
    }
    const prefix = COMPANY_PREFIXES.find((p) => token.startsWith(p));
    if (prefix && !negate) {
      const { text } = unquote(token.slice(prefix.length));
      if (text) q.companies.push(text);
      continue;
    }
    const { text, quoted } = unquote(token);
    if (!text) continue;
    if (negate) q.excludes.push(text);
    else if (quoted) q.phrases.push(text);
    else q.terms.push(text);
  }
  q.empty =
    q.terms.length === 0 &&
    q.phrases.length === 0 &&
    q.excludes.length === 0 &&
    q.companies.length === 0;
  return q;
}

/** One posting, casefolded once. */
export interface IndexEntry {
  item: ScrapedItem;
  /** Title and employer: where a hit counts the most. */
  head: string;
  company: string;
  /** Everything else worth searching: location, snippet, skills, source. */
  body: string;
}

const indexes = new WeakMap<readonly ScrapedItem[], IndexEntry[]>();

/** The index for a list snapshot, built on first use and kept with the array. */
export function indexOf(items: readonly ScrapedItem[]): IndexEntry[] {
  const hit = indexes.get(items);
  if (hit) return hit;
  const built = items.map((item) => {
    const company = (companyOf(item) ?? "").toLowerCase();
    return {
      item,
      head: `${item.title} ${company}`.toLowerCase(),
      company,
      body: [
        item.location ?? "",
        item.content_text ?? "",
        item.required_skills?.join(" ") ?? "",
        item.source_platform,
        item.seniority ?? "",
      ]
        .join(" ")
        .toLowerCase(),
    };
  });
  indexes.set(items, built);
  return built;
}

/** Characters that make up a word, for the "starts a word" test. */
function isWordChar(code: number): boolean {
  return (
    (code >= 97 && code <= 122) || // a-z
    (code >= 48 && code <= 57) || // 0-9
    code > 127 // anything non-ASCII: accented letters, CJK
  );
}

/**
 * Whether `needle` appears in `hay` starting at a word boundary. A needle that
 * itself starts with punctuation (".net", "#rust") has no boundary to respect.
 */
export function hasWordPrefix(hay: string, needle: string): boolean {
  if (!needle) return true;
  const boundary = isWordChar(needle.charCodeAt(0));
  let at = hay.indexOf(needle);
  while (at !== -1) {
    if (!boundary || at === 0 || !isWordChar(hay.charCodeAt(at - 1))) return true;
    at = hay.indexOf(needle, at + 1);
  }
  return false;
}

/** Whether `phrase` appears in `hay` as whole words: a boundary at both ends. */
export function hasWhole(hay: string, phrase: string): boolean {
  if (!phrase) return true;
  const startBoundary = isWordChar(phrase.charCodeAt(0));
  const endBoundary = isWordChar(phrase.charCodeAt(phrase.length - 1));
  let at = hay.indexOf(phrase);
  while (at !== -1) {
    const end = at + phrase.length;
    const startOk = !startBoundary || at === 0 || !isWordChar(hay.charCodeAt(at - 1));
    const endOk = !endBoundary || end === hay.length || !isWordChar(hay.charCodeAt(end));
    if (startOk && endOk) return true;
    at = hay.indexOf(phrase, at + 1);
  }
  return false;
}

/**
 * Postings whose fetched description has a word starting with each term, from
 * Rust's full-text index (`search_descriptions`). List rows carry no
 * description, so this is the only way a word that appears only in the body
 * of a posting can find it.
 */
export type DescriptionHits = Map<string, Set<string>>;

/** The terms worth asking the description index about. Mirrors `db::indexable_term`. */
export function indexableTerms(q: ParsedQuery): string[] {
  return [...new Set(q.terms)].filter((t) => t.length >= 2 && t.length <= 40 && /^[\p{L}\p{N}]+$/u.test(t));
}

/**
 * How well an entry matches: 0 = no match, 1 = matched only in the body or
 * the fetched description, 2 = every positive word or phrase was in the title
 * or employer.
 */
export function score(entry: IndexEntry, q: ParsedQuery, described?: DescriptionHits): 0 | 1 | 2 {
  if (q.companies.length > 0 && !q.companies.some((c) => entry.company.includes(c))) return 0;
  for (const x of q.excludes) {
    if (hasWordPrefix(entry.head, x) || hasWordPrefix(entry.body, x)) return 0;
  }
  let allInHead = true;
  for (const t of q.terms) {
    if (hasWordPrefix(entry.head, t)) continue;
    if (!hasWordPrefix(entry.body, t) && !described?.get(t)?.has(entry.item.url)) return 0;
    allInHead = false;
  }
  for (const p of q.phrases) {
    if (hasWhole(entry.head, p)) continue;
    if (!hasWhole(entry.body, p)) return 0;
    allInHead = false;
  }
  return allInHead ? 2 : 1;
}

/**
 * The items that match, and how well. Keeps the input order: callers sort
 * afterwards, and may use the tier map to put title hits first.
 */
export function search(
  items: readonly ScrapedItem[],
  q: ParsedQuery,
  described?: DescriptionHits,
): { items: readonly ScrapedItem[]; tier: Map<string, number> } {
  const tier = new Map<string, number>();
  // The same array back, not a copy: callers search the stable snapshot first
  // (whose index is memoized on it) and filter afterwards.
  if (q.empty) return { items, tier };
  const out: ScrapedItem[] = [];
  for (const entry of indexOf(items)) {
    const s = score(entry, q, described);
    if (s > 0) {
      out.push(entry.item);
      tier.set(entry.item.url, s);
    }
  }
  return { items: out, tier };
}

/** The words worth highlighting in a title: terms, phrases and company names. */
export function highlightTerms(q: ParsedQuery): string[] {
  return [...q.phrases, ...q.terms, ...q.companies].filter((t) => t.length >= 2);
}

/**
 * Splits `text` into runs, marking the ones that start a highlighted term.
 * Case-insensitive; respects the same word-start rule the matcher uses.
 */
export function highlight(text: string, terms: string[]): { text: string; hit: boolean }[] {
  if (terms.length === 0) return [{ text, hit: false }];
  const lower = text.toLowerCase();
  const marks = new Uint8Array(text.length);
  for (const term of terms) {
    const boundary = isWordChar(term.charCodeAt(0));
    let at = lower.indexOf(term);
    while (at !== -1) {
      if (!boundary || at === 0 || !isWordChar(lower.charCodeAt(at - 1))) {
        marks.fill(1, at, at + term.length);
      }
      at = lower.indexOf(term, at + 1);
    }
  }
  const runs: { text: string; hit: boolean }[] = [];
  let start = 0;
  for (let i = 1; i <= text.length; i++) {
    if (i === text.length || marks[i] !== marks[start]) {
      runs.push({ text: text.slice(start, i), hit: marks[start] === 1 });
      start = i;
    }
  }
  return runs;
}
