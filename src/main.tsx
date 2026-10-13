import { RouterProvider } from "@tanstack/react-router";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { router } from "./router";
import { connectStorageEvents } from "./stores/storage-events";
import "./styles.css";

const root = document.getElementById("root");
if (!root) throw new Error("Missing app root");
const storageEvents = connectStorageEvents();
void storageEvents.ready.catch((error: unknown) => {
  console.error("Storage event connection failed", error);
});
import.meta.hot?.dispose(storageEvents.dispose);
createRoot(root).render(
  <StrictMode>
    <RouterProvider router={router} />
  </StrictMode>,
);
