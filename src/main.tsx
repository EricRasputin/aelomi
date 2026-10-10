import { createRoot } from "react-dom/client";
import "./styles.css";

const root = document.getElementById("root");
if (!root) throw new Error("Missing app root");
createRoot(root).render(<main>Aelomi</main>);
