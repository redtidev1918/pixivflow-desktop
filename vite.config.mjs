import { defineConfig } from "vite";

// F1 frontend: a plain status shell — vanilla JS calling Tauri commands via
// invoke. Frontend lives under src/frontend; build lands in <repo>/dist so
// tauri.conf.json frontendDist ("../dist") resolves correctly.
export default defineConfig({
  root: "src/frontend",
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  build: {
    outDir: "../../dist",
    emptyOutDir: true,
  },
});
