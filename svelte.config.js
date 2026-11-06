import adapter from "@sveltejs/adapter-static";
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

/** @type {import('@sveltejs/kit').Config} */
const config = {
  preprocess: vitePreprocess(),
  kit: {
    // Tauri serves static files, so the build is a single-page app with index.html as fallback.
    adapter: adapter({ fallback: "index.html" }),
  },
};

export default config;
