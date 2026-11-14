/// <reference types="vitest/config" />
import { sveltekit } from "@sveltejs/kit/vite";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "vite";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [tailwindcss(), sveltekit()],

  // Unit tests for the pure modules (search, filters, sorting). Nothing here
  // needs a DOM or a Tauri runtime.
  test: {
    include: ["src/**/*.test.ts"],
    environment: "node",
  },

  // Tauri expects a fixed port and surfaces Rust errors instead of hiding them.
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: {
      // src-tauri changes are handled by the Rust toolchain, not Vite.
      ignored: ["**/src-tauri/**"],
    },
  },
});
