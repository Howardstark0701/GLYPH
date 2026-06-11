import { defineConfig } from 'astro/config';
import tailwind from '@astrojs/tailwind';
import vercel from '@astrojs/vercel/serverless';

export default defineConfig({
  output: 'server',
  adapter: vercel(),
  integrations: [tailwind()],
  vite: {
    define: {
      'import.meta.env.PUBLIC_API_BASE_URL': JSON.stringify(
        process.env.PUBLIC_API_BASE_URL ?? 'http://localhost:8000'
      ),
    },
  },
});
