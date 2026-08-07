import { defineConfig } from 'astro/config';
import tailwind from '@astrojs/tailwind';
import vercel from '@astrojs/vercel/serverless';

export default defineConfig({
  output: 'server',
  adapter: vercel(),
  integrations: [tailwind()],
  // NOTE: PUBLIC_API_BASE_URL is left to Astro/Vite's native PUBLIC_* env
  // handling. A manual vite.define here previously baked 'http://localhost:8000'
  // into every bundle when the env var was unset, which is what made the
  // deployed site call the user's own machine. apiBase() in repo-view.ts owns
  // the runtime fallback now.
});
