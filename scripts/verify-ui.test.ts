import { execFileSync } from "node:child_process";
import { resolve } from "node:path";
import { expect, test } from "vitest";

test("verification UI lock and frontmost CLI contracts", {
  timeout: 30_000,
}, () => {
  const output = execFileSync(
    "bash",
    [resolve(".claude/skills/verify-aelomi/scripts/ui-lock.test.sh")],
    { encoding: "utf8", timeout: 20_000 },
  );
  expect(output).toContain(
    "PASS: concurrent stale-lock contenders leave exactly one owner",
  );
  expect(output).toContain(
    "PASS: frontmost accepts only the run PID and refuses Apple event errors",
  );
});
