# GLYPH

> Extract the *why* layer hidden inside any GitHub repository.

GLYPH is an open-source intelligence backend that takes a GitHub repository URL and reconstructs the **decision history** of that codebase — what was debated, what was rejected, what architectural choices were made and why.

**Stack:** Rust, Axum, PostgreSQL, NVIDIA NIM, Astro.js, D3.js  
**Deployment:** Render (backend + PostgreSQL), Vercel (frontend)

See [GLYPH.md](./GLYPH.md) for the full specification and [backend/README.md](./backend/README.md) for backend deployment + BYOK security notes.
