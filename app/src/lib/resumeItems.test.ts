import { describe, expect, it } from "vitest";
import { detailItems, skillItems } from "./resumeItems";

// Kept in step with `detail_lines_split_on_what_they_were_written_with` in
// resume/mod.rs: both sides must split a line identically.
describe("detailItems", () => {
  it("splits on the separator the line was written with", () => {
    expect(detailItems("Rust · SQLite, bundled")).toEqual(["Rust", "SQLite, bundled"]);
    expect(detailItems("a, b")).toEqual(["a", "b"]);
    expect(detailItems("GPA 3.9/4.0")).toEqual(["GPA 3.9/4.0"]);
    expect(detailItems("  ")).toEqual([]);
    expect(detailItems(null)).toEqual([]);
  });
});

describe("skillItems", () => {
  it("splits a skills run on commas", () => {
    expect(skillItems("Rust, TypeScript ,  Go")).toEqual(["Rust", "TypeScript", "Go"]);
  });
});
