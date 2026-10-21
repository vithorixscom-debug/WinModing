import { describe, expect, it } from "vitest";
import { FALLBACK_CATEGORIES, FALLBACK_LOADERS } from "./tags.js";

describe("FALLBACK_LOADERS", () => {
  it("covers the four Minecraft mod loaders", () => {
    const names = FALLBACK_LOADERS.map((item) => item.name);
    expect(names).toEqual(["fabric", "forge", "quilt", "neoforge"]);
    expect(FALLBACK_LOADERS.every((item) => item.supported_project_types.includes("mod"))).toBe(
      true,
    );
  });
});

describe("FALLBACK_CATEGORIES", () => {
  it("includes mod categories so the catalog sidebar is never empty", () => {
    const mods = FALLBACK_CATEGORIES.filter((item) => item.project_type === "mod");
    expect(mods.length).toBeGreaterThan(8);
    expect(mods.some((item) => item.name === "optimization")).toBe(true);
  });
});
