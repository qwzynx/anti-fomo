import { describe, expect, it } from "vitest";
import type { ScrapedItem } from "./api";
import { filterRoles, matchesSpecialty, sortItems } from "./filters";
import { DEFAULT_FILTERS } from "./roles.svelte";

function role(title: string, extra: Partial<ScrapedItem> = {}): ScrapedItem {
  return {
    title,
    source_platform: "Lever",
    item_type: "Job",
    url: `https://x.test/${encodeURIComponent(title)}`,
    content_text: "",
    timestamp: "2026-11-01T00:00:00Z",
    discipline: null,
    relevance_score: 0,
    ...extra,
  };
}

describe("matchesSpecialty", () => {
  it("does not fire inside other words", () => {
    expect(matchesSpecialty("database administrator", "Data")).toBe(false);
    expect(matchesSpecialty("production support engineer", "Product")).toBe(false);
    expect(matchesSpecialty("maintain the pipeline", "AI/ML")).toBe(false);
  });

  it("accepts the ways postings spell a field", () => {
    expect(matchesSpecialty("back-end engineer", "Backend")).toBe(true);
    expect(matchesSpecialty("front end developer", "Frontend")).toBe(true);
    expect(matchesSpecialty("machine learning intern", "AI/ML")).toBe(true);
    expect(matchesSpecialty("data engineer, new grad", "Data")).toBe(true);
    expect(matchesSpecialty("associate product manager", "Product")).toBe(true);
  });
});

describe("filterRoles", () => {
  const none = () => null;
  const now = Date.parse("2026-11-13T00:00:00Z");
  const f = { ...structuredClone(DEFAULT_FILTERS) };

  it("filters by level, reading an internship with no stated level as Intern", () => {
    const items = [
      role("Software Intern", { item_type: "Internship" }),
      role("Senior Engineer", { seniority: "Senior" }),
      role("Engineer"),
    ];
    const kept = filterRoles(items, { ...f, seniority: ["Intern"] }, none, now);
    expect(kept.map((i) => i.title)).toEqual(["Software Intern"]);
  });

  it("filters by an annual pay floor, dropping undisclosed pay", () => {
    const items = [
      role("Hourly", { salary_min: 40, salary_period: "hour" }), // 83,200/yr
      role("Low", { salary_min: 50_000, salary_period: "year" }),
      role("Unknown"),
    ];
    const kept = filterRoles(items, { ...f, minPay: 60_000 }, none, now);
    expect(kept.map((i) => i.title)).toEqual(["Hourly"]);
  });

  it("filters by an upcoming deadline", () => {
    const items = [
      role("Soon", { closes_at: "2026-11-15T00:00:00Z" }),
      role("Later", { closes_at: "2026-12-30T00:00:00Z" }),
      role("Closed", { closes_at: "2026-11-01T00:00:00Z" }),
      role("None"),
    ];
    expect(filterRoles(items, { ...f, deadline: "Has deadline" }, none, now).map((i) => i.title)).toEqual([
      "Soon",
      "Later",
    ]);
    expect(
      filterRoles(items, { ...f, deadline: "Closing this week" }, none, now).map((i) => i.title),
    ).toEqual(["Soon"]);
  });

  it("filters by freshness on the ISO timestamp", () => {
    const items = [
      role("New", { timestamp: "2026-11-12T12:00:00Z" }),
      role("Old", { timestamp: "2026-10-01T00:00:00Z" }),
    ];
    const realNow = Date.now;
    Date.now = () => now;
    try {
      expect(filterRoles(items, { ...f, freshness: "Past week" }, none, now).map((i) => i.title)).toEqual([
        "New",
      ]);
    } finally {
      Date.now = realNow;
    }
  });
});

describe("sortItems", () => {
  it("puts missing keys last whichever way it sorts", () => {
    const items = [role("A"), role("B", { salary_min: 90_000, salary_period: "year" })];
    expect(sortItems(items, "Highest pay").map((i) => i.title)).toEqual(["B", "A"]);
  });
});
