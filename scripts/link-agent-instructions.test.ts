import { execFileSync } from "node:child_process";
import {
  existsSync,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { expect, test } from "vitest";

const script = resolve(import.meta.dirname, "link-agent-instructions.sh");

function setup() {
  const home = mkdtempSync(join(tmpdir(), "link-home-"));
  const repo = mkdtempSync(join(tmpdir(), "link-repo-"));
  mkdirSync(join(home, ".agents"));
  const src = join(home, ".agents", "aelomi.md");
  writeFileSync(src, "# private instructions\n");
  execFileSync("git", ["init", "-q"], { cwd: repo });
  writeFileSync(join(repo, ".gitignore"), "/CLAUDE.md\n/AGENTS.md\n");
  const run = () =>
    execFileSync("sh", [script], {
      cwd: repo,
      env: { ...process.env, HOME: home },
      encoding: "utf8",
    });
  return { repo, src, run };
}

const isLinkTo = (path: string, target: string) =>
  lstatSync(path).isSymbolicLink() && readlinkSync(path) === target;

test("links both names, then a second run changes nothing", () => {
  const { repo, src, run } = setup();
  expect(run()).toContain("Linked CLAUDE.md");
  expect(isLinkTo(join(repo, "CLAUDE.md"), src)).toBe(true);
  expect(isLinkTo(join(repo, "AGENTS.md"), src)).toBe(true);
  expect(run()).toBe("");
});

test("replaces an identical copy and a stale link, but keeps an edited copy", () => {
  const { repo, src, run } = setup();
  writeFileSync(join(repo, "CLAUDE.md"), readFileSync(src));
  execFileSync("ln", ["-s", "/nowhere", join(repo, "AGENTS.md")]);
  run();
  expect(isLinkTo(join(repo, "CLAUDE.md"), src)).toBe(true);
  expect(isLinkTo(join(repo, "AGENTS.md"), src)).toBe(true);

  const edited = setup();
  writeFileSync(join(edited.repo, "CLAUDE.md"), "my local edits\n");
  expect(edited.run()).toContain("CLAUDE.md is a regular file that differs");
  expect(readFileSync(join(edited.repo, "CLAUDE.md"), "utf8")).toBe(
    "my local edits\n",
  );
});

test("leaves a tracked copy alone", () => {
  const { repo, run } = setup();
  writeFileSync(join(repo, ".gitignore"), "");
  writeFileSync(join(repo, "CLAUDE.md"), "tracked\n");
  execFileSync("git", ["add", "CLAUDE.md"], { cwd: repo });
  run();
  expect(lstatSync(join(repo, "CLAUDE.md")).isSymbolicLink()).toBe(false);
});

test("says so when the machine has no private instructions", () => {
  const { repo, src, run } = setup();
  execFileSync("rm", [src]);
  expect(run()).toContain("No agent instructions on this machine");
  expect(existsSync(join(repo, "CLAUDE.md"))).toBe(false);
});
