/** @type {import('tailwindcss').Config} */
module.exports = {
  content: ['./src/**/*.{astro,html,js,jsx,ts,tsx}'],
  theme: {
    extend: {
      colors: {
        glyph: {
          // Original palette (kept for other pages)
          dark: '#08090d',
          panel: '#0d1117',
          accent: '#22d3ee',
          conflict: '#f59e0b',
          reject: '#ef4444',
          // Terminal OS palette
          bg: '#131313',
          border: '#262626',
          red: '#cc0000',
          text: '#e5e2e1',
          muted: '#a1a1a1',
          hero: '#ffb4a8',
          rust: '#5e3f3a',
        },
      },
      fontFamily: {
        mono: ['"JetBrains Mono"', 'monospace'],
        sans: ['"Karla"', 'sans-serif'],
        display: ['"Space Grotesk"', 'sans-serif'],
      },
      fontSize: {
        'data-sm': ['0.6875rem', { lineHeight: '1rem', letterSpacing: '0.04em' }],
        'label-caps': ['0.6875rem', { lineHeight: '1rem', letterSpacing: '0.1em' }],
      },
      borderRadius: {
        DEFAULT: '0px',
        none: '0px',
        sm: '0px',
        md: '0px',
        lg: '0px',
        xl: '0px',
        '2xl': '0px',
        full: '0px',
      },
      boxShadow: {
        none: 'none',
      },
    },
  },
  plugins: [],
};
