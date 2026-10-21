import { describe, expect, it } from "vitest";
import { projectUrl, userUrl, versionUrl } from "./links.js";

describe("projectUrl", () => {
  it("builds a Modrinth project URL", () => {
    expect(projectUrl("mod", "sodium")).toBe("https://modrinth.com/mod/sodium");
  });

  it("builds a CurseForge URL when the source is curse", () => {
    expect(projectUrl("mod", "jei", "curseforge")).toBe(
      "https://www.curseforge.com/minecraft/mc-mods/jei",
    );
  });
});

describe("versionUrl", () => {
  it("appends the version id", () => {
    expect(versionUrl("mod", "sodium", "abc")).toBe("https://modrinth.com/mod/sodium/version/abc");
  });
});

describe("userUrl", () => {
  it("builds a user profile URL", () => {
    expect(userUrl("jellysquid3")).toBe("https://modrinth.com/user/jellysquid3");
  });
});
