// @ts-check

/**
 * @param {import("../src/ipc").Theme} choice
 * @param {"light" | "dark"} appearance
 * @returns {"light" | "dark"}
 */
function appliedTheme(choice, appearance) {
  if (choice === "Light") return "light";
  if (choice === "Dark") return "dark";
  return appearance;
}

/** @param {"light" | "dark"} theme */
function applyTheme(theme) {
  document.documentElement.dataset.theme = theme;
  document.documentElement.classList.toggle("dark", theme === "dark");
  document.documentElement.style.colorScheme = theme;
}

window.aelomiTheme = { resolve: appliedTheme, apply: applyTheme };
applyTheme(
  appliedTheme(
    window.__aelomiInitialTheme ?? "System",
    window.matchMedia("(prefers-color-scheme: dark)").matches
      ? "dark"
      : "light",
  ),
);
