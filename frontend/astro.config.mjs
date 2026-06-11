import { defineConfig } from 'astro/config';
import tailwind from '@astrojs/tailwind';

export default defineConfig({
  // 'static' output: all pages pre-rendered at build time (correct for this app —
  // all data fetching happens client-side via fetch(), no SSR needed)
  output: 'static',
  integrations: [tailwind()],
  vite: {
    define: {
      // Bakes PUBLIC_API_BASE_URL into the JS bundle at build time.
      // Set this env var in Vercel dashboard → Project → Settings → Environment Variables
      'import.meta.env.PUBLIC_API_BASE_URL': JSON.stringify(
        process.env.PUBLIC_API_BASE_URL ?? 'http://localhost:8000'
      ),
    },
  },
});
