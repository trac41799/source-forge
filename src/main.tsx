import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { BrowserRouter } from "react-router-dom";
import { installWebPreviewShim } from "./lib/ipc/webPreviewShim";
import App from "./App";
import "./styles/globals.css";

installWebPreviewShim();

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <BrowserRouter>
      <App />
    </BrowserRouter>
  </StrictMode>
);