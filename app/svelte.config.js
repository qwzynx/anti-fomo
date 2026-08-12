import adapter from "@sveltejs/adapter-static";
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

/** @type {import('@sveltejs/kit').Config} */
const config = {
  preprocess: vitePreprocess(),
  kit: {
    // Tauri serves the built files from a custom protocol, so there is no
    // Node server. Everything is prerendered into a static SPA shell.
    adapter: adapter({ fallback: "index.html" }),
  },
};

export default config;
