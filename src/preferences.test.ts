import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  initialLayout,
  initialPanelWidth,
  initialTheme,
  savePreference,
  applyTheme,
} from "./preferences";
let values: Map<string, string>;
beforeEach(() => {
  values = new Map();
  vi.stubGlobal("localStorage", {
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => values.set(key, value),
  });
  vi.stubGlobal("window", { matchMedia: () => ({ matches: true }) });
  vi.stubGlobal("document", { documentElement: { dataset: {}, style: {} } });
});
afterEach(() => vi.unstubAllGlobals());
describe("appearance preferences", () => {
  it("uses the system appearance for the first launch", () =>
    expect(initialTheme()).toBe("light"));
  it("remembers the chosen appearance over the system setting", () => {
    savePreference("theme", "dark");
    expect(initialTheme()).toBe("dark");
    applyTheme("dark");
    expect(document.documentElement.dataset.theme).toBe("dark");
    expect(document.documentElement.style.colorScheme).toBe("dark");
  });
  it("starts with side layout and restores a chosen stacked layout", () => {
    expect(initialLayout()).toBe("side");
    savePreference("layout", "stacked");
    expect(initialLayout()).toBe("stacked");
  });
  it("restores the last sidebar width", () => {
    expect(initialPanelWidth()).toBe(420);
    for (const width of [260, 280, 340, 387.5, 560]) {
      savePreference("panelWidth", String(width));
      expect(initialPanelWidth()).toBe(width);
    }
  });
  it("falls back to the default width for invalid saved values", () => {
    for (const value of ["", "unknown", "NaN", "Infinity", "259", "561"]) {
      savePreference("panelWidth", value);
      expect(initialPanelWidth()).toBe(420);
    }
  });
  it("ignores obsolete or invalid preference values", () => {
    savePreference("theme", "unknown");
    savePreference("layout", "unknown");
    expect(initialTheme()).toBe("light");
    expect(initialLayout()).toBe("side");
  });
  it("still opens when optional preference storage is unavailable", () => {
    vi.stubGlobal("localStorage", {
      getItem: () => {
        throw Error("unavailable");
      },
      setItem: () => {
        throw Error("unavailable");
      },
    });
    expect(initialTheme()).toBe("light");
    expect(initialLayout()).toBe("side");
    expect(() => savePreference("theme", "dark")).not.toThrow();
    expect(initialPanelWidth()).toBe(420);
  });
});
