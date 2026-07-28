import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Dev server proxies API + WebSocket to a locally running samong-server.
export default defineConfig({
  plugins: [react()],
  server: {
    proxy: {
      "/api": "http://127.0.0.1:3000",
      "/ws": { target: "ws://127.0.0.1:3000", ws: true },
    },
  },
});
