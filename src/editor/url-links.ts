import type { MouseEvent } from "react";
import { commands } from "../ipc";

const pointerStarts = new WeakMap<HTMLElement, { x: number; y: number }>();

export function recordPageLinkPointer(
  event: Pick<MouseEvent<HTMLElement>, "currentTarget" | "clientX" | "clientY">,
) {
  pointerStarts.set(event.currentTarget, {
    x: event.clientX,
    y: event.clientY,
  });
}

export function allowedUrl(value: string): boolean {
  try {
    return ["http:", "https:", "mailto:"].includes(new URL(value).protocol);
  } catch {
    return false;
  }
}

export function openPageLink(
  event: Pick<
    MouseEvent<HTMLElement>,
    | "target"
    | "currentTarget"
    | "preventDefault"
    | "type"
    | "button"
    | "detail"
    | "clientX"
    | "clientY"
    | "shiftKey"
    | "altKey"
    | "ctrlKey"
    | "metaKey"
  >,
) {
  const start = pointerStarts.get(event.currentTarget);
  pointerStarts.delete(event.currentTarget);
  const target = event.target;
  const link = target instanceof Element ? target.closest("a[href]") : null;
  if (!link || !event.currentTarget.contains(link)) return;
  event.preventDefault();
  const url = link.getAttribute("href");
  if (
    event.type === "contextmenu" ||
    event.shiftKey ||
    event.altKey ||
    event.ctrlKey ||
    event.metaKey ||
    event.detail > 1 ||
    (event.detail !== 0 &&
      (!start ||
        Math.hypot(event.clientX - start.x, event.clientY - start.y) > 4)) ||
    !url ||
    !allowedUrl(url) ||
    (event.type === "auxclick" && event.button !== 1)
  )
    return;
  void commands.openUrl(url).then(
    (reply) => {
      if (reply.status === "error")
        console.error("URL opener failed", reply.error);
    },
    (error: unknown) => console.error("URL opener failed", error),
  );
}
