import { describe, expect, it } from "vitest";
import { moved } from "./sortable";

describe("moved", () => {
  it("moves an item down and up", () => {
    expect(moved(["a", "b", "c", "d"], 0, 2)).toEqual(["b", "c", "a", "d"]);
    expect(moved(["a", "b", "c", "d"], 3, 1)).toEqual(["a", "d", "b", "c"]);
  });
  it("leaves the input alone", () => {
    const list = ["a", "b"];
    moved(list, 0, 1);
    expect(list).toEqual(["a", "b"]);
  });
});
