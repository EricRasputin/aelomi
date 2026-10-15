import type { MouseEvent } from "react";
import { commands } from "../ipc";

export function allowedUrl(value: string): boolean {
  try {
    return ["http:", "https:", "mailto:"].includes(new URL(value).protocol);
  } catch {
    return false;
  }
}

export function openPageLink(event: MouseEvent<HTMLElement>) {
  const target = event.target;
  const link = target instanceof Element ? target.closest("a[href]") : null;
  if (!link || !event.currentTarget.contains(link)) return;
  event.preventDefault();
  event.stopPropagation();
  const url = link.getAttribute("href");
  if (
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
