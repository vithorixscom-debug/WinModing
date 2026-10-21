import { describe, expect, it } from "vitest";
import { isModrinthProjectId, wrapSearch } from "./cache.js";

describe("isModrinthProjectId", () => {
  it("accepts slug-like ids", () => {
    expect(isModrinthProjectId("sodium")).toBe(true);
    expect(isModrinthProjectId("AANobbMI")).toBe(true);
  });

  it("rejects curseforge, github, paths, and numeric ids", () => {
    expect(isModrinthProjectId("12345")).toBe(false);
    expect(isModrinthProjectId("curseforge:123")).toBe(false);
    expect(isModrinthProjectId("github:owner/repo")).toBe(false);
    expect(isModrinthProjectId("folder/mod.jar")).toBe(false);
    expect(isModrinthProjectId("mod.jar")).toBe(false);
    expect(isModrinthProjectId("null")).toBe(false);
  });
});

describe("wrapSearch", () => {
  it("passes through nested result objects", () => {
    const result = { hits: [1], offset: 0, limit: 20, total_hits: 1 };
    expect(wrapSearch({ result })).toEqual({ result });
  });

  it("normalizes a flat search payload", () => {
    expect(wrapSearch({ hits: ["a"], offset: 5, limit: 10, total_hits: 40 })).toEqual({
      result: { hits: ["a"], offset: 5, limit: 10, total_hits: 40 },
    });
  });
});
