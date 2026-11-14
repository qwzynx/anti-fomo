import { describe, expect, it } from "vitest";
import type { ScrapedItem } from "./api";
import { hasWhole, hasWordPrefix, highlight, parseQuery, search } from "./query";

function role(title: string, extra: Partial<ScrapedItem> = {}): ScrapedItem {
  return {
    title,
    source_platform: "Greenhouse",
    item_type: "Job",
    url: `https://x.test/${encodeURIComponent(title)}`,
    content_text: "",
    timestamp: "2026-11-01T00:00:00Z",
    discipline: null,
    relevance_score: 0,
    ...extra,
  };
}

const titles = (items: readonly ScrapedItem[]) => items.map((i) => i.title);

describe("parseQuery", () => {
  it("splits words, phrases, exclusions and company filters", () => {
    const q = parseQuery('React  "machine learning" -senior company:"Jane Street" at:stripe');
    expect(q.terms).toEqual(["react"]);
    expect(q.phrases).toEqual(["machine learning"]);
    expect(q.excludes).toEqual(["senior"]);
    expect(q.companies).toEqual(["jane street", "stripe"]);
    expect(q.empty).toBe(false);
  });

  it("treats whitespace and a lone dash as nothing", () => {
    expect(parseQuery("   ").empty).toBe(true);
    expect(parseQuery("-").terms).toEqual(["-"]);
  });

  it("keeps a quoted single word as a whole-word phrase", () => {
    expect(parseQuery('"java"').phrases).toEqual(["java"]);
  });
});

describe("word matching", () => {
  it("matches at the start of a word only", () => {
    expect(hasWordPrefix("software engineer", "eng")).toBe(true);
    expect(hasWordPrefix("maintain systems", "ai")).toBe(false);
    expect(hasWordPrefix("c++ developer", "c++")).toBe(true);
    expect(hasWordPrefix("asp.net core", ".net")).toBe(true);
  });

  it("matches a quoted word whole", () => {
    expect(hasWhole("java and javascript", "java")).toBe(true);
    expect(hasWhole("javascript only", "java")).toBe(false);
  });
});

describe("search", () => {
  const items = [
    role("Backend Engineer", { company: "Shopify", location: "Toronto, ON" }),
    role("Frontend Developer", { company: "Stripe", content_text: "React and TypeScript", location: "Remote" }),
    role("Senior React Engineer", { company: "Meta", location: "Toronto" }),
    role("Data Analyst", { company: "Jane Street", required_skills: ["Python", "SQL"] }),
  ];

  it("ANDs every word across fields", () => {
    expect(titles(search(items, parseQuery("react toronto")).items)).toEqual(["Senior React Engineer"]);
  });

  it("excludes", () => {
    expect(titles(search(items, parseQuery("react -senior")).items)).toEqual(["Frontend Developer"]);
  });

  it("filters by employer", () => {
    expect(titles(search(items, parseQuery("company:jane")).items)).toEqual(["Data Analyst"]);
  });

  it("searches skills", () => {
    expect(titles(search(items, parseQuery("python")).items)).toEqual(["Data Analyst"]);
  });

  it("ranks title and employer hits above body hits", () => {
    const hit = search(items, parseQuery("react"));
    expect(hit.tier.get(items[2].url)).toBe(2);
    expect(hit.tier.get(items[1].url)).toBe(1);
  });

  it("returns the same array for an empty query", () => {
    expect(search(items, parseQuery("")).items).toBe(items);
  });
});

describe("highlight", () => {
  it("marks word-start hits, case-insensitively", () => {
    expect(highlight("React Engineer", ["react"])).toEqual([
      { text: "React", hit: true },
      { text: " Engineer", hit: false },
    ]);
    expect(highlight("Maintainer", ["ai"])).toEqual([{ text: "Maintainer", hit: false }]);
  });
});
