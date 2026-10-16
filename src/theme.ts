import type { Theme } from "./ipc";

export type AppliedTheme = "light" | "dark";

declare global {
  interface Window {
    __aelomiInitialTheme?: Theme;
    aelomiTheme: {
      resolve: (choice: Theme, appearance: AppliedTheme) => AppliedTheme;
      apply: (theme: AppliedTheme) => void;
    };
  }
}

export function selectedTheme(choice: Theme) {
  return typeof choice === "string" ? choice : "System";
}
