import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { readFileSync } from "node:fs";

const pkg = JSON.parse(
  readFileSync(new URL("./package.json", import.meta.url), "utf-8")
);

const appVersionPlugin = {
  name: "sayl-app-version",
  enforce: "pre",
  transform(code, id) {
    if ((id.includes("/src/") || id.includes("\\src\\")) && code.includes("__APP_VERSION__")) {
      return {
        code: code.replaceAll("__APP_VERSION__", JSON.stringify(pkg.version)),
        map: null,
      };
    }
  },
};

export default defineConfig({
  plugins: [appVersionPlugin, svelte()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
    target: process.env.SAYL_SKIP_MINIFY === "1" ? "esnext" : "es2021",
    minify: process.env.SAYL_SKIP_MINIFY === "1" ? false : (!process.env.TAURI_DEBUG ? "esbuild" : false),
    sourcemap: !!process.env.TAURI_DEBUG,
  },
  test: {
    globals: true,
    environment: "node",
    exclude: ["**/node_modules/**", "src-tauri/**"],
  },
});
