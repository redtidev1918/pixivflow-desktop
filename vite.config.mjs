import { defineConfig } from "vite";

// base: "./" is the Fix for the white-screen bug — without it Vite emits
// absolute `/assets/...` URLs that fail to resolve under the Tauri custom
// protocol (webview loads blank). Relative base keeps production builds working.
export default defineConfig({
  root: "src/frontend",
  base: "./",
  plugins: [],
  build: {
    outDir: "../../dist",
    emptyOutDir: true,
  },
  server: {
    port: 1420,
    strictPort: true,
  },
});
