import React from "react";
import ReactDOM from "react-dom/client";
// Three roles, three faces. Bai Jamjuree is semi-condensed, which matters for a
// Thai interface: Thai titles run long and still have to fit one line.
import "@fontsource/bai-jamjuree/500.css";
import "@fontsource/bai-jamjuree/600.css";
import "@fontsource/bai-jamjuree/700.css";
import "@fontsource/ibm-plex-sans-thai/400.css";
import "@fontsource/ibm-plex-sans-thai/500.css";
import "@fontsource/ibm-plex-sans-thai/600.css";
// Mono is for paths and counts — identifiers, not prose.
import "@fontsource/ibm-plex-mono/400.css";
import "./styles.css";
import { App } from "./App";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
