# GLYPH

> Extract the *why* layer hidden inside any GitHub repository.

GLYPH is an open-source intelligence backend that takes a GitHub repository URL and reconstructs the **decision history** of that codebase — what was debated, what was rejected, what architectural choices were made and why.

**Stack:** Rust, Axum, PostgreSQL, NVIDIA NIM, Astro.js, D3.js  
**Deployment:** Render (backend + PostgreSQL), Vercel (frontend)

## Features

- **Deep ingestion** — commits, pull requests, issues, PR review threads and
  issue-comment debates are pulled from the GitHub API (with retry/backoff on
  `403`/`429`). Full pagination scales to 10,000-commit repos
  (`MAX_COMMITS`, `MAX_LIST_PAGES`).
- **Multi-pass NIM intent extraction** — the decision history is cut into
  evenly-spaced windows spanning the *whole* commit history, each extracted
  by NVIDIA NIM, then a consolidation pass merges cross-window duplicates. If
  NIM fails, a deterministic fallback extracts real insights from commit
  messages + PR state so a job never completes empty. Each insight is a
  decision, debate, or rejection with contributors, source references, and a
  confidence score.
- **Multi-repo comparison** — analyze two repositories and diff their decision
  histories: counts, intent-type distribution, mean confidence, contributor
  overlap, and deterministic comparison notes.
- **Live analysis dashboard** — real status polling, stage-by-stage progress,
  honest `failed` / `terminated` / empty states, a D3 force-directed graph,
  and a D3 chronological decision timeline.
- **BYOK (Bring Your Own Key)** — your GitHub PAT and NIM API key live in
  `sessionStorage` only and travel as per-request headers; the server never
  stores or logs them. See [backend/README.md](./backend/README.md).
- **Abuse protection** — fixed-window per-client rate limiting (`429 +
  Retry-After`), a 64 KB body cap, and strict GitHub-URL validation.
- **Report export** — on any completed analysis, the **EXPORT** button downloads
  a bundled JSON report plus a human-readable Markdown brief covering the
  status, narrative summary, decisions, debates, rejections, contributors, and
  graph topology — all client-side, no keys shared beyond the normal headers.

See [GLYPH.md](./GLYPH.md) for the full specification and [backend/README.md](./backend/README.md) for backend deployment + BYOK security notes.
