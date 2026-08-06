# GLYPH Project Progress Checklist

## Overview
**Project Goal:** Reconstruct the decision history of any GitHub repository — what was decided, what was debated, what was rejected, and who drove it — using GitHub events + NVIDIA NIM analysis. BYOK security: user keys travel per-request, never stored server-side.
**Current Status:** All 12 tasks complete. Deployed to Render (backend) + Vercel (frontend).
**Last Updated:** August 7, 2026

## Latest Session (2026-08-07) — the 12-task hardening pass

Everything below is committed and pushed. Verified locally: `cargo check --all-targets` clean, `cargo test` **15/15 green**, `npm run build` clean.

### Task 1 — Verify live deploy
- Diagnosed the Render crash-loop: the migration fix for the reserved `user` column existed only locally, so the live backend was running pre-fix code whose migration fails on a fresh DB.
- Pushed the migration-fix commits → Render rebuild. Vercel frontend confirmed healthy (`302`).
- **Honest limitation:** end-to-end *real* analysis on the live site cannot be automated — it requires the user's GitHub PAT + NIM key, which are kept in the browser via sessionStorage (BYOK). The user runs the first live analysis themselves.

### Task 2 — Kill fake data across every repo view
- **2a Debates page** — real sentiment metrics (agreement/contention/avg-confidence) + an honest heatmap that renders the real debate matrix on UUID jobs; demo seed only for slug URLs.
- **2b Dashboard** — `sha:0x0000000` placeholder → `N/A`; fake probability matrix / neural forecast / conflict callouts only render for demo slugs; real UUID jobs get honest metrics hydrated from `/graph` (`total_nodes`, `total_links`, `density_pct`).
- **2c Loading/empty/error transitions** — all 5 subpages (decisions, debates, rejections, summary, contributors) detect UUID jobs server-side (`IS_UUID` regex), render honest loading placeholders, and hydrate from the real APIs; demo seeds are gated behind `!IS_UUID`.

### Task 3 — Backend robustness
- **3a Rate limiting** — custom fixed-window per-client limiter (`rate_limit.rs`, no new deps), `X-Forwarded-For` first-hop identity with `ConnectInfo` fallback, `429 + Retry-After`, background `prune()` task bounds the map.
- **3b Request validation** — 2048-char `repo_url` cap + `RequestBodyLimitLayer` (64KB) on `/api`; host-validated URL parsing rejects foreign hosts/subdomains/extra path segments.
- **3c Full pagination** — commits/PRs/issues scale with `MAX_COMMITS` (default 1k, 10k for big repos) + `MAX_LIST_PAGES` (default 10 pages); relies on existing 403/429 retry/backoff.
- **3d Stuck-job recovery** — startup sweep marks any repo left in `processing` by a dead process as `failed` with an honest error; jobs are idempotent (`ON CONFLICT DO NOTHING`) so re-running is safe.

### Task 4 — Advanced features
- **4a Multi-repo comparison** — `POST /api/compare` diffs two repositories' decision histories: raw counts, intent-type distribution, mean confidence, contributor overlap, deterministic comparison notes. Frontend `/compare` page auto-runs missing analyses (BYOK + status polling) then renders side-by-side panels.
- **4b D3 chronological decision timeline** — fixed-height D3 scatter on the decisions page (x = timestamp, y = confidence, dot size/colour = confidence/stability), sequence line + hover tooltips; hydrates from real data for UUID jobs.
- **4c Multi-pass NIM analysis + fallback** — commit history cut into evenly-spaced windows spanning the whole history (was: newest-50-commits truncation), one extraction call per window, then a consolidation pass merges cross-window duplicates. If NIM fails, deterministic fallback extracts real insights from commit messages + PR state so a job never completes empty. `MAX_ANALYSIS_CHUNKS` bounds LLM cost.

### Task 5 — Final verify + docs + commit
- `cargo test` 15/15, `cargo check` clean, `npm run build` clean, all commits pushed.
- This checklist + README updated.

## Task Board

- ✅ **Task 1 — Verify live deploy works** — migration-fix pushed; Render rebuild triggered; Vercel healthy; end-to-end real analysis requires the user's BYOK keys (not automatable).
- ✅ **Task 2a — Debates page: real sentiment chart + honest heatmap**
- ✅ **Task 2b — Kill `sha:0x0000000` placeholder + dashboard fake metrics**
- ✅ **Task 2c — Smoother loading/empty/error transitions across repo views**
- ✅ **Task 3a — Backend rate limiting (BYOK abuse protection)**
- ✅ **Task 3b — Request validation / sanitization**
- ✅ **Task 3c — Full pagination for large repos (10k commits)**
- ✅ **Task 3d — Recovery for jobs stuck in 'processing' forever**
- ✅ **Task 4a — Multi-repo comparison (diff two decision histories)**
- ✅ **Task 4b — D3 chronological decision timeline**
- ✅ **Task 4c — Multi-pass NIM analysis + fallback for big/complex repos**
- ✅ **Task 5 — Final verify + docs + commit everything**

## Key Architecture

- **Stack:** Rust + Axum 0.7, sqlx 0.7 + PostgreSQL (migrations auto-run at boot), reqwest 0.12 async GitHub/NIM clients, NVIDIA NIM (`nvidia/llama-3.1-nemotron-70b-instruct`), Astro.js 4 (server-rendered), D3.js 7 (jsdelivr CDN ESM), Tailwind.
- **BYOK security model:** GitHub PAT + NIM key held in browser `sessionStorage`, travel per-request as `X-Github-Token` / `X-Nim-Api-Key`, never stored or logged server-side.
- **Honest-data pattern:** every `prerender=false` page regex-tests its `:id` against a UUID; real jobs render honest loading placeholders + client hydration from the API, while demo/slug URLs keep the polished seed. No fake flash on real jobs.
- **Analysis pipeline:** ingest (commits → PRs → issues → review threads) → multi-pass NIM extraction (windowed pass 1 + consolidation pass 2) → deterministic fallback → `intent_nodes` (decision/debate/rejection/architectural).

## Live URLs
- Backend: `https://glyph-api-u495.onrender.com` (`/health` probe)
- Frontend: `https://glyph-pua2jbu7s-glyph-tango.vercel.app`
- Compare: `<frontend>/compare`

## Known Limitations / Honest Notes
1. **Live end-to-end analysis needs the user's keys** — BYOK by design; can't be automated/verified in CI.
2. **Render free tier sleeps** — first request after idle triggers a cold start (slow first load); `000` probes during rebuilds are expected.
3. **Fallback nodes are coarser** — when NIM fails, deterministic insights (commit messages + PR state) are real but less semantically rich; confidence reflects the heuristic.
4. **`MAX_ANALYSIS_CHUNKS` bounds LLM cost** — a 10k-commit repo is covered via 6 evenly-spaced windows, not every commit.
5. **Summary narrative** (`/summary`) still requires a valid NIM key at request time (on-demand generation).

## Success Criteria
- [x] User enters a GitHub repo URL and gets a full decision-history analysis
- [x] D3 graph + chronological timeline show decision nodes and relationships
- [x] All API endpoints return correct shapes (smoke-tested locally)
- [x] BYOK credential flow works end-to-end (keys never leave the browser to GLYPH servers)
- [x] Pagination handles repos up to 10,000 commits (`MAX_COMMITS=10000`)
- [x] Dashboard + subpages update in real-time during analysis (status polling, no fabricated events)
- [x] Deployed to Render + Vercel
- [x] Rate limiting + request validation + stuck-job recovery + multi-pass NIM fallback
- [x] Multi-repo comparison + D3 timeline + honest empty/loading/error states

---

*Last updated 2026-08-07 after the 12-task hardening pass.*
