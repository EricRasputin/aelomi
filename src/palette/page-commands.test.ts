import { expect, test } from "vitest";
import { matchingPageCommands } from "./page-commands";

test("CMD-2 CMD-3 commands require a non-empty query", () => {
  expect(matchingPageCommands("", true)).toEqual([]);
  expect(matchingPageCommands("   ", true)).toEqual([]);
  expect(
    matchingPageCommands("trash", true).map((command) => command.label),
  ).toEqual(["Move to trash"]);
});

test("CMD-6 only Move to trash requires an active page", () => {
  expect(
    matchingPageCommands("new", false).map((command) => command.label),
  ).toEqual(["New page"]);
  expect(matchingPageCommands("trash", false)).toEqual([]);
  expect(
    matchingPageCommands("new", true).map((command) => command.label),
  ).toEqual(["New page"]);
  expect(matchingPageCommands("theme", true)).toEqual([]);
});
