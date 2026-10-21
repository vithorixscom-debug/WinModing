import { resolve } from "node:path";
import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import svgLoader from "vite-svg-loader";

const root = resolve(__dirname);

export default defineConfig({
  resolve: {
    alias: [
      { find: "@", replacement: resolve(root, "src") },
      { find: "@modrinth/assets", replacement: resolve(root, "packages/assets") },
      { find: "@modrinth/ui", replacement: resolve(root, "packages/ui") },
      { find: "@modrinth/utils", replacement: resolve(root, "packages/utils") },
    ],
  },
  plugins: [
    vue(),
    svgLoader({
      svgoConfig: {
        plugins: [
          {
            name: "preset-default",
            params: { overrides: { removeViewBox: false } },
          },
        ],
      },
    }),
  ],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  envPrefix: ["VITE_", "TAURI_"],
  test: {
    environment: "node",
    include: ["src/helpers/*.test.js"],
  },
  build: {
    target: "chrome105",
    minify: !process.env.TAURI_DEBUG ? "esbuild" : false,
    sourcemap: !!process.env.TAURI_DEBUG,
  },
});
