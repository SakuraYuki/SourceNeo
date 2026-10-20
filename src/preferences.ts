export type Theme = "light" | "dark";
export type Layout = "side" | "stacked";
export const MIN_PANEL_WIDTH = 260;
export const MAX_PANEL_WIDTH = 560;

function read(key: string): string | null {
  try {
    return localStorage.getItem(`sourceneo.${key}`);
  } catch {
    return null;
  }
}
export function savePreference(key: string, value: string) {
  try {
    localStorage.setItem(`sourceneo.${key}`, value);
  } catch {
    /* Preferences are optional when storage is unavailable. */
  }
}
export function initialTheme(): Theme {
  const saved = read("theme");
  if (saved === "light" || saved === "dark") return saved;
  return window.matchMedia("(prefers-color-scheme: light)").matches
    ? "light"
    : "dark";
}
export function initialLayout(): Layout {
  return read("layout") === "stacked" ? "stacked" : "side";
}
export function initialPanelWidth(): number {
  const width = Number(read("panelWidth"));
  return Number.isFinite(width) &&
    width >= MIN_PANEL_WIDTH &&
    width <= MAX_PANEL_WIDTH
    ? width
    : 420;
}
export function applyTheme(theme: Theme) {
  document.documentElement.dataset.theme = theme;
  document.documentElement.style.colorScheme = theme;
}
