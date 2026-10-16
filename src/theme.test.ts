// @vitest-environment happy-dom
import { readFileSync } from "node:fs";
import { runInNewContext } from "node:vm";
import { beforeEach, describe, expect, it } from "vitest";
import type { Theme } from "./ipc";
import { selectedTheme } from "./theme";

beforeEach(() => {
  delete window.__aelomiInitialTheme;
  runInNewContext(readFileSync("public/theme.js", "utf8"), {
    window,
    document,
  });
});

describe("SHELL-5 applied theme", () => {
  const cases: [Theme, "light" | "dark", "light" | "dark"][] = [
    ["Light", "light", "light"],
    ["Light", "dark", "light"],
    ["Dark", "light", "dark"],
    ["Dark", "dark", "dark"],
    ["System", "light", "light"],
    ["System", "dark", "dark"],
    [{ Unknown: "Sepia" }, "light", "light"],
    [{ Unknown: "Sepia" }, "dark", "dark"],
  ];
  it.each(cases)(
    "resolves %j with %s appearance to %s",
    (choice, appearance, expected) => {
      expect(window.aelomiTheme.resolve(choice, appearance)).toBe(expected);
    },
  );
  it("applies the stored Dark choice before the app mounts", () => {
    window.__aelomiInitialTheme = "Dark";
    runInNewContext(readFileSync("public/theme.js", "utf8"), {
      window,
      document,
    });
    expect(document.documentElement.dataset.theme).toBe("dark");
    expect(document.documentElement.style.colorScheme).toBe("dark");
    expect(document.documentElement.className).toBe("dark");
    window.aelomiTheme.apply("light");
    expect(document.documentElement.dataset.theme).toBe("light");
    expect(document.documentElement.className).toBe("");
  });
  it("shows System for an unknown choice without changing its value", () => {
    const choice = { Unknown: "Sepia" };
    expect(selectedTheme(choice)).toBe("System");
    expect(choice).toEqual({ Unknown: "Sepia" });
    expect(selectedTheme("Dark")).toBe("Dark");
    expect(selectedTheme("Light")).toBe("Light");
    expect(selectedTheme("System")).toBe("System");
  });
});
