#!/usr/bin/osascript -l JavaScript
ObjC.import("CoreGraphics");

const USAGE = `Usage: ax.js <command> <pid> [args]
  tree PID                  print the window's web content as an accessibility tree
  status PID                print the text of the role="status" region
  content PID               print the status or alert dialog text
  wait-status PID TEXT [S]  wait up to S seconds (default 10) until the status reads TEXT
  wait-dialog PID TITLE [S] wait up to S seconds for an alert dialog with TITLE
  press PID NAME            send AXPress to the first element named NAME
  focus PID NAME            focus the first element named NAME
  keys PID TEXT [MODS]      bring the app to the front and type TEXT; MODS is a comma list of command,shift,option,control
  window PID                print the CGWindowID and System Events bounds of the app window
  menus PID                 print the native menu bar titles`;

function run(argv) {
  const [command, pidArg, ...rest] = argv;
  const pid = Number(pidArg);
  if (!command || !Number.isInteger(pid)) throw new Error(USAGE);
  const se = Application("System Events");
  const procs = se.processes.whose({ unixId: pid });
  if (procs.length === 0) throw new Error(`No process with PID ${pid}.`);
  const proc = procs[0];

  switch (command) {
    case "tree":
      return tree(webArea(proc));
    case "status":
      return statusText(proc);
    case "content":
      return textOf(region(webArea(proc), ["AXApplicationStatus", "AXApplicationAlertDialog"]));
    case "wait-status":
      return waitStatus(proc, rest[0], Number(rest[1] ?? 10));
    case "wait-dialog":
      return waitDialog(proc, rest[0], Number(rest[1] ?? 10));
    case "press":
      findByName(webArea(proc), rest[0]).actions.whose({ name: "AXPress" })[0].perform();
      return `pressed ${JSON.stringify(rest[0])}`;
    case "focus": {
      const element = findByName(webArea(proc), rest[0]);
      element.focused = true;
      return `focused ${JSON.stringify(rest[0])}: ${element.focused()}`;
    }
    case "keys":
      return keys(se, proc, rest[0], rest[1]);
    case "window":
      return windowInfo(proc, pid);
    case "menus":
      return proc.menuBars[0].menuBarItems.name().join("\n");
    default:
      throw new Error(USAGE);
  }
}

// Accessibility lists only windows on the current Space, and another app can take the
// front between two commands.
function ready(proc, { front = false, seconds = 5 } = {}) {
  const deadline = Date.now() + seconds * 1000;
  while (Date.now() < deadline) {
    if (proc.windows.length > 0 && (!front || proc.frontmost())) return;
    proc.frontmost = true;
    delay(0.1);
  }
  throw new Error(`The app has no window in the accessibility tree after ${seconds} s. Is it minimized?`);
}

function webArea(proc) {
  const deadline = Date.now() + 5000;
  while (Date.now() < deadline) {
    ready(proc);
    const queue = [proc.windows[0]];
    while (queue.length > 0) {
      const element = queue.shift();
      if (get(element, "role") === "AXWebArea") return element;
      queue.push(...children(element));
    }
    delay(0.2);
  }
  throw new Error("The window has no web content yet.");
}

function get(element, property) {
  try {
    return element[property]();
  } catch (_) {
    return null;
  }
}

function label(element) {
  return get(element, "name") || get(element, "description") || "";
}

// uiElements() returns references rooted at the process name, which reach the wrong
// app when two Aelomi processes run. Index specifiers stay rooted at the PID.
function children(element) {
  try {
    return Array.from({ length: element.uiElements.length }, (_, i) => element.uiElements[i]);
  } catch (_) {
    return [];
  }
}

function tree(root) {
  const lines = [];
  const visit = (element, depth) => {
    const subrole = get(element, "subrole");
    const role = subrole ? `${get(element, "role")}/${subrole}` : get(element, "role");
    const value = get(element, "value");
    const name = label(element);
    let line = `${"  ".repeat(depth)}${role}`;
    if (name) line += ` ${JSON.stringify(name)}`;
    if (value !== null && value !== "" && value !== name) line += ` = ${JSON.stringify(String(value))}`;
    lines.push(line);
    for (const child of children(element)) visit(child, depth + 1);
  };
  visit(root, 0);
  return lines.join("\n");
}

function textOf(root) {
  const parts = [];
  const visit = (element) => {
    if (get(element, "role") === "AXStaticText") parts.push(String(get(element, "value") ?? ""));
    for (const child of children(element)) visit(child);
  };
  visit(root);
  return parts.join("");
}

function region(web, subroles) {
  const queue = [web];
  while (queue.length > 0) {
    const element = queue.shift();
    if (subroles.includes(get(element, "subrole"))) return element;
    queue.push(...children(element));
  }
  throw new Error(`No ${subroles.join(" or ")} region in the web content.`);
}

function statusText(proc) {
  return textOf(region(webArea(proc), ["AXApplicationStatus"]));
}

function waitDialog(proc, expected, seconds) {
  if (expected === undefined) throw new Error(USAGE);
  const deadline = Date.now() + seconds * 1000;
  let actual = "";
  while (Date.now() < deadline) {
    try {
      const dialog = region(webArea(proc), ["AXApplicationAlertDialog"]);
      actual = label(dialog);
      if (actual === expected) return textOf(dialog);
    } catch (_) {
      actual = "";
    }
    delay(0.2);
  }
  throw new Error(`Dialog did not read ${JSON.stringify(expected)} within ${seconds} s. Last: ${JSON.stringify(actual)}`);
}

function waitStatus(proc, expected, seconds) {
  if (expected === undefined) throw new Error(USAGE);
  const deadline = Date.now() + seconds * 1000;
  let actual = "";
  while (Date.now() < deadline) {
    try {
      actual = statusText(proc);
    } catch (_) {
      actual = "";
    }
    if (actual === expected) return actual;
    delay(0.2);
  }
  throw new Error(`Status did not read ${JSON.stringify(expected)} within ${seconds} s. Last: ${JSON.stringify(actual)}`);
}

function findByName(web, name) {
  if (name === undefined) throw new Error(USAGE);
  const named = [];
  const queue = [web];
  while (queue.length > 0) {
    const element = queue.shift();
    const role = get(element, "role");
    const elementLabel = label(element);
    if (role !== "AXStaticText" && role !== "AXWebArea" && elementLabel) {
      if (elementLabel === name) return element;
      named.push(`${role} ${JSON.stringify(elementLabel)}`);
    }
    queue.push(...children(element));
  }
  throw new Error(`No element named ${JSON.stringify(name)}. Named elements:\n${named.join("\n")}`);
}

function keys(se, proc, text, mods) {
  if (text === undefined) throw new Error(USAGE);
  const using = (mods ? mods.split(",") : []).map((mod) => `${mod.trim()} down`);
  ready(proc, { front: true });
  se.keystroke(text, { using });
  return `typed ${JSON.stringify(text)}${using.length ? ` with ${using.join(", ")}` : ""}`;
}

function windowInfo(proc, pid) {
  ready(proc);
  const windows = ObjC.deepUnwrap(
    ObjC.castRefToObject($.CGWindowListCopyWindowInfo($.kCGWindowListOptionOnScreenOnly, $.kCGNullWindowID)),
  ).filter((info) => info.kCGWindowOwnerPID === pid && info.kCGWindowLayer === 0);
  if (windows.length === 0) throw new Error(`No on-screen window for PID ${pid}. Is it minimized?`);
  const [x, y] = proc.windows[0].position();
  const [w, h] = proc.windows[0].size();
  return `id=${windows[0].kCGWindowNumber} title=${JSON.stringify(proc.windows[0].name())} x=${x} y=${y} w=${w} h=${h}`;
}
