import { describe, expect, it, vi } from "vitest";
import { debounce, releaseColor } from "./utils.js";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@/helpers/profile", () => ({
  get_full_path: vi.fn(),
  get_mod_full_path: vi.fn(),
}));

describe("releaseColor", () => {
  it("maps known channels", () => {
    expect(releaseColor("release")).toBe("green");
    expect(releaseColor("beta")).toBe("orange");
    expect(releaseColor("alpha")).toBe("red");
    expect(releaseColor("other")).toBe("");
  });
});

describe("debounce", () => {
  it("calls the wrapped function once after the wait", () => {
    vi.useFakeTimers();
    const fn = vi.fn();
    const wrapped = debounce(fn, 50);
    wrapped(1);
    wrapped(2);
    expect(fn).not.toHaveBeenCalled();
    vi.advanceTimersByTime(50);
    expect(fn).toHaveBeenCalledTimes(1);
    expect(fn).toHaveBeenCalledWith(2);
    vi.useRealTimers();
  });
});
