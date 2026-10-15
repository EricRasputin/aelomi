import { execFileSync } from "node:child_process";
import { expect, test } from "vitest";

test("agent instructions stay out of the repo", () => {
  const tracked = execFileSync("git", ["ls-files", "CLAUDE.md", "AGENTS.md"], {
    encoding: "utf8",
  });
  expect(tracked).toBe("");
});
