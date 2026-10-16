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

test("CMD-3 command matching stops at prefix and word starts", () => {
  expect(matchingPageCommands("moth", true)).toEqual([]);
  expect(matchingPageCommands("tash", true)).toEqual([]);
  expect(matchingPageCommands("to", true).map((c) => c.command)).toEqual([
    "trashPage",
  ]);
  expect(matchingPageCommands("new p", false).map((c) => c.command)).toEqual([
    "newPage",
  ]);
});
