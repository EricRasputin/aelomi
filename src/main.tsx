import { RouterProvider } from "@tanstack/react-router";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { connectQuitDrain } from "./editor/quit-drain";
import { router } from "./router";
import { connectIndexEvents } from "./stores/index-status";
import { connectPalette } from "./stores/palette";
import { connectStorageEvents } from "./stores/storage-events";
import { connectStorageStartup } from "./stores/storage-startup";
import "./styles.css";

const root = document.getElementById("root");
if (!root) throw new Error("Missing app root");
const stopDrain = connectQuitDrain();
const storageEvents = connectStorageEvents();
const storageStartup = connectStorageStartup();
void storageEvents.ready.catch((error: unknown) => {
  console.error("Storage event connection failed", error);
});
const indexEvents = connectIndexEvents();
void indexEvents.ready.catch((error: unknown) => {
  console.error("Index event connection failed", error);
});
const stopPalette = connectPalette();
import.meta.hot?.dispose(() => {
  storageEvents.dispose();
  storageStartup.dispose();
  indexEvents.dispose();
  stopPalette();
  stopDrain();
});
createRoot(root).render(
  <StrictMode>
    <RouterProvider router={router} />
  </StrictMode>,
);
