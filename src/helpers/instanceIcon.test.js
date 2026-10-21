import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: (path) => `asset://localhost/${path}`,
}));

import { instanceIconSrc } from "./instanceIcon.js";

describe("instanceIconSrc", () => {
  it("prefers a data URL over a file path", () => {
    expect(
      instanceIconSrc({
        iconDataUrl: "data:image/png;base64,abc",
        icon_path: "C:\\\\icons\\\\x.png",
      }),
    ).toBe("data:image/png;base64,abc");
  });

  it("returns null when no icon is set", () => {
    expect(instanceIconSrc({})).toBeNull();
    expect(instanceIconSrc({ icon_path: "null" })).toBeNull();
  });

  it("passes through http and data paths", () => {
    expect(instanceIconSrc({ icon_path: "https://cdn.example/a.png" })).toBe(
      "https://cdn.example/a.png",
    );
    expect(instanceIconSrc({ icon_path: "data:image/png;base64,xyz" })).toBe(
      "data:image/png;base64,xyz",
    );
  });
});
