# GLYPH Backend

Rust + Axum service that ingests a GitHub repository (commits, PRs, issues, review
threads and comment debates) and runs an NVIDIA NIM LLM pass to reconstruct the
codebase's **decision history** — what was decided, what was debated, what was
rejected, and why.

## Stack

- **Axum 0.7** (Tokio) HTTP server
- **sqlx** + PostgreSQL — migrations auto-run at startup
- **reqwest** async GitHub + NIM HTTP clients
- **NVIDIA NIM** LLM for intent extraction (`/v1/chat/completions`)

## Local development

```bash
# 1. PostgreSQL must be reachable (local, Docker, or managed)
createdb glyph

# 2. Run
DATABASE_URL=postgres://localhost/glyph cargo run
```

The server listens on `0.0.0.0:8000` (override with `PORT`). Migrations run
automatically on first boot.

## Environment variables

| Variable | Required | Default | Purpose |
|---|---|---|---|
| `DATABASE_URL` | ✅ | — | PostgreSQL connection string. Migrations run automatically on boot. |
| `PORT` | ❌ | `8000` | HTTP listen port (Render injects this). |
| `FRONTEND_URL` | ❌ | — | Allowed CORS origin (e.g. the Vercel frontend). Localhost dev origins are always allowed. |
| `NIM_MODEL` | ❌ | `nvidia/llama-3.1-nemotron-70b-instruct` | NIM model id used for intent extraction. |
| `NIM_BASE_URL` | ❌ | `https://integrate.api.nvidia.com/v1` | NVIDIA NIM chat-completions endpoint. |
| `RUST_LOG` | ❌ | `glyph=info,tower_http=info` | Tracing filter. |

## API

| Route | Method | Purpose |
|---|---|---|
| `/health` | GET | Liveness probe → `OK` |
| `/api/analyze` | POST | Start analysis. Body `{ "repo_url": "https://github.com/owner/name" }` → `{ "job_id": "<uuid>" }`. |
| `/api/repo/:id/status` | GET | Job status: `queued / processing / complete / failed / terminated`, plus `stage` and `error_message`. |
| `/api/repo/:id/terminate` | POST | Flips the cancellation flag; the job stops at its next checkpoint and records `terminated`. |
| `/api/repo/:id/graph` | GET | Extracted intent graph (`nodes`, `links`, `meta`). |
| `/api/repo/:id/summary` | GET | NIM-generated narrative summary. |
| `/api/repo/:id/decisions` | GET | Decision nodes. |

## BYOK security model

The backend is **stateless with respect to credentials** — GitHub PATs and NIM API
keys are never stored, logged, or persisted:

- The frontend holds keys in `sessionStorage` only (cleared on refresh/session end).
- Keys travel **per-request** as `X-Github-Token` and `X-Nim-Api-Key` headers.
- The backend reads them from the request headers for the duration of that request
  and discards them. Nothing is written to disk or the database.
- Never paste keys into URLs, and never commit them to `.env` files.

## Deployment (Render)

`render.yaml` (repo root) provisions:

- a **web service** `glyph-api`, built from the multi-stage `Dockerfile`
  (Rust → `debian:bookworm-slim`),
- a **managed PostgreSQL** database `glyph-db` whose connection string is injected
  into the web service as `DATABASE_URL`.

Steps:

1. Push the repo to GitHub.
2. Render → New → Blueprint → select the repo (Render reads `render.yaml`).
3. After the frontend is live, set `FRONTEND_URL` to the deployed Vercel URL.
4. Set `NIM_MODEL` / `NIM_BASE_URL` if you need non-defaults.
5. Deploy — migrations run automatically on first boot.

> GLYPH previously ran on Shuttle.rs. Shuttle shut down in April 2026; the backend
> is now plain Axum on Render. The old `railway`/`shuttle` references are gone.
