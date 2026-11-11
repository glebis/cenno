// Builds the live widget gallery for the site from the app's own React views
// (src/a2ui/views.tsx), so the site never drifts from what the panel renders.
// Output is committed to site/public/widgets/ — the site build itself does not
// need the app source or React. Rebuild with `npm run widgets` in site/.
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "node:path";

const here = __dirname;
const appModules = path.resolve(here, "../../node_modules");

export default defineConfig({
  root: here,
  plugins: [react()],
  define: { "process.env.NODE_ENV": JSON.stringify("production") },
  resolve: {
    alias: [
      { find: "@tauri-apps/plugin-opener", replacement: path.join(here, "stubs/opener.ts") },
      { find: "@tauri-apps/api/core", replacement: path.join(here, "stubs/core.ts") },
      { find: /^react-dom(.*)$/, replacement: `${appModules}/react-dom$1` },
      { find: /^react-markdown$/, replacement: `${appModules}/react-markdown` },
      { find: /^react(\/.*)?$/, replacement: `${appModules}/react$1` },
    ],
  },
  build: {
    outDir: path.resolve(here, "../public/widgets"),
    emptyOutDir: false,
    cssCodeSplit: false,
    modulePreload: false,
    rollupOptions: {
      input: path.join(here, "main.tsx"),
      output: {
        entryFileNames: "widgets.js",
        assetFileNames: "widgets[extname]",
      },
    },
  },
});
