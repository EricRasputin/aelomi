import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expect, test } from "vitest";

const summary = `editor-empty: full key-to-paint p95 20 ms
  app-owned p95 7 ms
  split check: over 405 samples from 4 loads
  load average 9.34
palette results p50 15 ms p95 25 ms (ci budget 87 ms) over 55 keys, load average 8.09
scroll frames 294, rendering work p50 2 ms p95 15 ms (budget < 35) max 115 ms, longest interval 55 ms (budget 83)
  previous frame's work 2 ms, then 53 ms to the next stamp; intervals over 33 ms 3
  load average 7.46`;

function collect(log: string) {
  const dir = mkdtempSync(join(tmpdir(), "aelomi-ci-collector-"));
  const output = join(dir, "output.json");
  writeFileSync(join(dir, "log"), log);
  writeFileSync(
    join(dir, "gh"),
    `#!/usr/bin/env node
import { readFileSync } from 'node:fs';
const path = process.argv[3];
const run = { id: 1, event: 'pull_request', head_branch: 'feature', head_sha: 'head', run_attempt: 3 };
if (path.includes('/logs')) process.stdout.write(readFileSync(${JSON.stringify(join(dir, "log"))}));
else if (path.includes('/jobs?')) {
  const parts = path.split('/');
  const attempt = Number(parts[parts.indexOf('attempts') + 1]);
  const started_at = ['2026-10-15T21:29:00Z', '2026-10-15T21:29:00.000Z', '2026-10-15T21:30:05Z'][attempt - 1];
  process.stdout.write(JSON.stringify([{ jobs: [{ name: 'keystroke', id: attempt, head_sha: 'head', started_at, status: 'completed', conclusion: 'success' }] }]));
} else process.stdout.write(JSON.stringify([{ workflow_runs: [run] }]));
`,
    { mode: 0o755 },
  );
  return {
    run: () =>
      execFileSync(
        process.execPath,
        [
          "scripts/collect-keystroke-ci.mjs",
          "2026-10-13T00:00:00Z",
          "2026-10-15T21:30Z",
          output,
        ],
        {
          env: { ...process.env, PATH: `${dir}:${process.env.PATH}` },
          encoding: "utf8",
          stdio: "pipe",
        },
      ),
    read: () => JSON.parse(readFileSync(output, "utf8")),
    clean: () => rmSync(dir, { recursive: true, force: true }),
  };
}

test("the collector extracts steady and tail metrics, marks carried jobs, and compares parsed dates", () => {
  const fixture = collect(summary);
  try {
    expect(fixture.run()).toContain("2 attempts, 1 carried rows");
    const { rows } = fixture.read();
    expect(rows).toHaveLength(2);
    expect(rows[0].palette).toEqual({
      p50: 15,
      p95: 25,
      keys: 55,
      load1: 8.09,
    });
    expect(rows[0].scroll).toEqual({
      p95: 15,
      workMax: 115,
      longest: 55,
      frames: 294,
      load1: 7.46,
      previousWork: 2,
      betweenFrames: 53,
      over33: 3,
    });
    expect(rows[0].typing).toEqual({
      "editor-empty": { p95: 7, samples: 405, load1: 9.34 },
    });
    expect(rows[1].carriedFrom).toBe(1);
  } finally {
    fixture.clean();
  }
});

test.each([
  ["palette", "CMD-4 and CMD-5 the palette shows results within one frame"],
  [
    "scroll",
    "EDIT-14 the 5,000-block editor page scrolls smoothly for 5 seconds",
  ],
  ["editor-empty", "EDIT-14 keystroke-to-paint on the editor-empty surface"],
  ["palette", summary.replace("p50 15 ms", "p50 unreadable ms")],
  [
    "scroll",
    summary.replace(
      "intervals over 33 ms 3",
      "intervals over 33 ms unreadable",
    ),
  ],
])("the collector rejects an unparseable %s summary", (name, log) => {
  const fixture = collect(log);
  try {
    expect(fixture.run).toThrow(`Unparseable ${name} perf summary`);
    expect(
      fixture
        .read()
        .rows.map((row: { summaryErrors: string[] }) => row.summaryErrors),
    ).toEqual([[name], [name]]);
  } finally {
    fixture.clean();
  }
});
