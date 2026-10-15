import { expect, test } from "vitest";
import { matchingPageCommands } from "./page-commands";

test("CMD-6 only the implemented page commands match, and require an active page", () => {
  expect(
    matchingPageCommands("", true).map((command) => command.command),
  ).toEqual(["newPage", "trashPage"]);
  expect(
    matchingPageCommands("trash", true).map((command) => command.label),
  ).toEqual(["Move to trash"]);
  expect(
    matchingPageCommands("new", true).map((command) => command.label),
  ).toEqual(["New page"]);
  expect(matchingPageCommands("", false)).toEqual([]);
  expect(matchingPageCommands("theme", true)).toEqual([]);
});
