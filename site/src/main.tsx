import { StrictMode } from "react";
import { createRoot, hydrateRoot } from "react-dom/client";
import { App } from "./App";
import "./styles.css";

const root = document.getElementById("root")!;
const app = (
  <StrictMode>
    <App />
  </StrictMode>
);

// The build prerenders the page into #root; hydrate it. In development it starts empty.
if (root.firstElementChild) hydrateRoot(root, app);
else createRoot(root).render(app);
