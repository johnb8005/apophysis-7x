import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import path from "node:path";

// GitHub Pages serves project sites from /<repo>/, so the base path must match
// the repository name in CI. Locally it stays "/" so `bun dev` works normally.
const base = process.env.GITHUB_ACTIONS ? "/apophysis-7x/" : "/";

export default defineConfig({
  base,
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: { "@": path.resolve(import.meta.dirname, "./src") },
  },
  // The generated .wasm is a real asset, not something to inline.
  assetsInclude: ["**/*.wasm"],
  build: {
    target: "es2022",
  },
});
