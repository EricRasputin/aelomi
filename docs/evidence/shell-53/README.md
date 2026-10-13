# SHELL-10 warning theme relation

Issue #53 asked for proof that the save-failure banner follows the active theme. The proof must not pin palette values. PR #50 replaced exact RGB pins with a readability check that each theme run applies independently. That check could not see a missing dark warning palette. The shipped light banner is readable on its own, so a dark run that painted it still passed.

`tests/playwright/save-failure.spec.ts` now reads the banner in its own theme, switches `page.emulateMedia` to the other color scheme, and reads the banner again. It asserts that the painted text backgrounds differ, then restores the original scheme. Each themed test makes this comparison itself, so no results are shared between tests. The exact copy, wrapping, overflow, contrast, dialog, focus, outside click, Escape, and quit decision checks are unchanged.

## Variants

Every variant uses one `vite build --mode mock` of the source at `f1c86fb`. Only the built stylesheet changes. No source file was modified.

| Variant | Change to the built CSS |
| --- | --- |
| `shipped` | None |
| `no-dark-warning` | Removes only `--warning-background:#2b251b;--warning-border:#57472b;--warning-text:#e5c585;` from the dark `@media` block. The dark `--danger-text`, `--overlay`, and app colors remain. |
| `teal-hue` | Appends light `#e9f6f4` background, `#b4ddd6` border, and `#0b5a52` text. Dark is `#13292a`, `#2c5052`, and `#8fd8cf`. |
| `inverted` | Appends the shipped dark warning palette to light mode and the shipped light warning palette to dark mode |

A WebKit probe read these computed colors for each variant. The root is `document.documentElement`. Contrast is text against banner.

| Variant, theme | Banner | Text | Root | Contrast |
| --- | --- | --- | --- | --- |
| shipped, light | 255, 248, 235 | 118, 80, 11 | 250, 250, 250 | 6.80 |
| shipped, dark | 43, 37, 27 | 229, 197, 133 | 24, 24, 27 | 9.15 |
| no-dark-warning, light | 255, 248, 235 | 118, 80, 11 | 250, 250, 250 | 6.80 |
| no-dark-warning, dark | 255, 248, 235 | 118, 80, 11 | 24, 24, 27 | 6.80 |
| teal-hue, light | 233, 246, 244 | 11, 90, 82 | 250, 250, 250 | 7.29 |
| teal-hue, dark | 19, 41, 42 | 143, 216, 207 | 24, 24, 27 | 9.35 |
| inverted, light | 43, 37, 27 | 229, 197, 133 | 250, 250, 250 | 9.15 |
| inverted, dark | 255, 248, 235 | 118, 80, 11 | 24, 24, 27 | 6.80 |

## Results

The runner runs the base spec from `f1c86fb` and the working-tree spec against every variant. Before the change, the working tree equaled `f1c86fb`, and the missing dark warning palette passed every test. The runner already expects the fixed spec to fail on that variant, so `as-expected=NO` below records the false pass:

```text
before shipped          exit=0  3 passed (3.3s)       expected pass                               as-expected=yes
before no-dark-warning  exit=0  3 passed (2.1s)       expected pass                               as-expected=yes
before teal-hue         exit=0  3 passed (2.1s)       expected pass                               as-expected=yes
before inverted         exit=0  3 passed (2.1s)       expected pass                               as-expected=yes
after  shipped          exit=0  3 passed (2.1s)       expected pass                               as-expected=yes
after  no-dark-warning  exit=0  3 passed (2.1s)       expected fail at 'changes with the theme'   as-expected=NO
after  teal-hue         exit=0  3 passed (2.1s)       expected pass                               as-expected=yes
after  inverted         exit=0  3 passed (2.1s)       expected pass                               as-expected=yes
result: UNEXPECTED outcome
```

With the 9-line spec change, only the negative control fails. Both themed tests fail at the new assertion, `save-failure.spec.ts:134`. The quit-confirmation test still passes, and no contrast assertion fails:

```text
before shipped          exit=0  3 passed (2.6s)       expected pass                               as-expected=yes
before no-dark-warning  exit=0  3 passed (2.4s)       expected pass                               as-expected=yes
before teal-hue         exit=0  3 passed (2.1s)       expected pass                               as-expected=yes
before inverted         exit=0  3 passed (2.1s)       expected pass                               as-expected=yes
after  shipped          exit=0  3 passed (2.1s)       expected pass                               as-expected=yes
after  no-dark-warning  exit=1  2 failed, 1 passed (3.1s) expected fail at 'changes with the theme'   as-expected=yes
      ✘  1 [webkit] › tests/playwright/save-failure.spec.ts:91:3 › SHELL-10 light warnings wrap and preserve the quit decision after recovery (438ms)
      ✘  2 [webkit] › tests/playwright/save-failure.spec.ts:91:3 › SHELL-10 dark warnings wrap and preserve the quit decision after recovery (445ms)
        Error: light warning background changes with the theme
        Expected: not [[255, 248, 235], [255, 248, 235], [255, 248, 235]]
        Error: dark warning background changes with the theme
        Expected: not [[255, 248, 235], [255, 248, 235], [255, 248, 235]]
after  teal-hue         exit=0  3 passed (2.1s)       expected pass                               as-expected=yes
after  inverted         exit=0  3 passed (2.1s)       expected pass                               as-expected=yes
result: all outcomes as expected
```

The working-tree spec also passed 30 of 30 runs against `shipped` with `--repeat-each 10`. `pnpm lint`, `pnpm typecheck`, and `pnpm test` passed with 90 Vitest tests.

## Why the backgrounds must differ

The relation compares the two themes' painted banner backgrounds. It does not compare each banner with its app background. The PR #50 review also suggested requiring the banner on the same side of mid-gray as the page. That rule catches `no-dark-warning`, where the dark run has banner luminance 0.944 on root 0.009. It also rejects `inverted`, which is readable at 9.15 and 6.80. In that variant, the light run has banner luminance 0.019 on root 0.956. The review named `document.body`, but the body background here is transparent `rgba(0, 0, 0, 0)`. The root element paints the app background.

The relation has these limits:

- A palette that keeps one banner background in both themes fails. Issue #53 treats a banner that ignores the theme as the defect to detect, so changing that design also means changing this test.
- The relation proves only that the banner changes with the theme. The existing 4.5:1 check in each theme proves readability.
- A missing dark `--warning-border` override alone is not detected. The border carries no text. Banner box visibility belongs to issue #54.
- The contrast check, not this relation, covers a partial loss. If only the dark background override is lost, the dark text sits on the light banner at 1.57:1. If only the dark text override is lost, the light text sits on the dark banner at 2.11:1. These ratios are computed from the shipped values with the `tests/contrast.ts` formula. They were not run as variants.

## Rerun

The quoted results, mutation details, and palette values above are the permanent evidence record. The local runners and raw logs below are temporary. They are not committed and may disappear when `/tmp` is cleared.

The runner is local and uncommitted at `/tmp/aelomi-issue53-proof/`. It uses port 4533, and the shipped Playwright config is unchanged. `playwright.config.mjs` serves one variant with `pnpm exec vite preview --mode mock --host 127.0.0.1 --port 4533 --strictPort --outDir <variant>`. `variants.mjs` writes the variants and fails unless it removes exactly 3 dark declarations. `run.sh` builds once with `pnpm exec vite build --mode mock --outDir /tmp/aelomi-issue53-proof/dist/build --emptyOutDir`. It extracts the base tests with `git archive f1c86fb tests package.json`, runs the 8 combinations, and exits nonzero on any unexpected outcome. `probe.sh` prints the colors in the table above.

```bash
/tmp/aelomi-issue53-proof/run.sh
/tmp/aelomi-issue53-proof/probe.sh
```

Environment: macOS 27.0.1 on arm64, Node 24.13.1, pnpm 12.11.2, and Playwright 1.63.0 with WebKit.
