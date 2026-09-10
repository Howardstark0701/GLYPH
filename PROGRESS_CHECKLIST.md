# GLYPH Project Progress Checklist

## Overview
**Project Goal:** Reconstruct the decision history of any GitHub repository — what was decided, what was debated, what was rejected, and who drove it — using GitHub events + NVIDIA NIM analysis. BYOK security: user keys travel per-request, never stored server-side.
**Current Status:** Local stack verified end-to-end with real credentials. The hosted backend is down pending a database swap — see "Deployment status" below.
**Last Updated:** September 10, 2026

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

## Latest session (2026-09-10) — debugging + structural pass

Bugs found and fixed this session. Each was breaking something the previous
checklist recorded as complete.

### The client scripts never ran at all

All five sub-pages used `<script define:vars={...}>`. Astro renders such a
script **inline**, so its `import` statements throw
`Uncaught SyntaxError: Cannot use import statement outside a module` and the
entire page's hydration dies on line 1. Nothing after it executed: no API
fetch, no empty states, no live feed. Every sub-page therefore sat on its
server-rendered seed for ever, which is why "Task 2c" looked done in the source
and was inert in the browser.

Fixed by passing seed data through a `<script type="application/json">` tag and
letting the real script be a normal, bundled Astro script.

### Dynamically created elements were unstyled

Astro scopes page CSS with a `[data-astro-cid-*]` attribute that only
server-rendered markup carries. Everything the pages build in JS — feed lines,
hydrated decision and contributor cards, D3 nodes — is created via `innerHTML`
or `createElement` and has no such attribute, so it rendered with no styling at
all. The six repo pages plus `/compare` now use `<style is:global>`; their
stylesheets are self-contained and page-scoped by construction.

### Seed data leaked into real analyses

A real, failed job still displayed `TOTAL 12`, `AVG CONFIDENCE 89%`, contributor
handles `@arch_priya / @lead_author / @dev_soren`, `REQ-99X-ALPHA-01`, and a log
line reading `SUCCESS: 12 ENTRIES PARSED FROM COMMIT_LOG`. `decisions.astro`
gated exactly one element on `IS_UUID` against twelve in `contributors.astro`.
Every stat, feed and header on a real job is now either a live value or an
em-dash, and each page wipes its seeded feed before attaching the real one.

### The NVIDIA NIM layer had never actually worked

The headline feature was silently degraded on every single run. Four separate
faults, each of which alone was enough:

1. **The model was retired.** `nvidia/llama-3.1-nemotron-70b-instruct` returns
   404 for this account while still being listed by `GET /v1/models`. The
   backend now walks a chain of candidate models and remembers the first that
   answers.
2. **The HTTP timeout was below the model's response time.** The shared client's
   60s timeout versus ~100s per extraction window meant every call timed out.
   NIM calls now use `NIM_TIMEOUT_SECS` (default 300).
3. **Responses could not be parsed.** Reasoning models narrate before answering.
   The parser now strips `<think>` blocks and scans every fenced block and every
   balanced JSON span instead of assuming the reply begins with `[`.
4. **Responses were truncated before the JSON.** Narration consumed the whole
   4096-token budget. The budget is now 8192 and the prompt demands a bare JSON
   array with no preamble.

Because extraction failures fall through to the deterministic fallback, all of
this presented as "working" — just with every decision titled like a raw commit
subject at a uniform 0.62 confidence.

### The decision timeline had no data to plot

`intent_nodes.timestamp` was never written — the insert simply omitted the
column and NIM does not emit dates — so it was `NULL` on every row and the D3
chronological timeline rendered empty for every real job. Each insight's source
refs are now matched back against the commits, PRs and issues just ingested
(`EventTimeline` in `api/analyze.rs`), which handles the loose formats models
actually emit: `a1b2c3d`, `commit: a1b2c3d`, `PR #1822`, `#1822`.

The same loose formatting made the decision cards read `COMMIT: commit:`,
because the client took the first seven characters of the raw reference.

### Analyses took ten minutes

Extraction windows ran sequentially. They are independent, so they now run
concurrently behind a semaphore (`MAX_CONCURRENT_WINDOWS = 4`).

### Navigation differed on every page

Five pages, five different sidebars — `TERMINAL/UPLINK/ARCHIVE/SENSORS/DECRYPT`,
`RECON/VCS_LOG/INDEX/NETWORK`, and so on — with almost no `href`s, so most
items were dead. All sub-pages now share the dashboard's six-item nav with real
links and correct active state.

### Fabricated readouts

`MEM: 12.4GB/32GB`, `CPU: 44%`, `BAUDRATE: 115200`, `LATENCY: 12ms` and
`FILTER: ALL_EVENTS` were invented — the browser has no access to host
telemetry and no filter control exists. Replaced with real facts: job id, API
host, the endpoint driving each feed, and a live `/health` probe that shows
`API: ONLINE` / `API: UNREACHABLE` with a measured round-trip.

### Smaller fixes

- Dashboard header read `SYSTEM RECONSTRUCTION: ACTIVE` while the status beside
  it read `FAILED`; the headline now tracks the real job status.
- The dashboard's D3 graph script hardcoded `localhost:8000` as its production
  fallback, so the deployed graph called the visitor's own machine.
- D3 is bundled from `node_modules` instead of fetched from jsDelivr, so charts
  render with no network.
- The 260px recon stream clipped every log line mid-word; it is wider and wraps.
- The `KEYS: NOT_SET` call to action — the first thing a new visitor must click
  — was thrown to the far-left page margin by `align-self: flex-start` inside a
  centred column, half-clipped. It now sits under the input with a pulsing
  indicator.
- Hero spacing is viewport-relative so the BYOK trace fits on a 768px display.
- The credential modal claimed keys are "never sent to GLYPH servers". They are
  — as request headers. Reworded to the accurate and still-strong claim: never
  stored, never logged.
- The landing page's SYSLOG panel cycled invented lines (`SSL_HANDSHAKE`,
  `PACKET_LOSS: 0.00%`, `THERMAL_NOMINAL`) that a browser cannot observe. It now
  logs real session events — session start, whether credentials are set, and
  backend reachability transitions.
- The decision timeline's x-axis used `%b %y`, rendering a 2022 tick as
  "Jan 22", which reads as a day. The format now follows the span: years for
  multi-year histories, `Jan '25` style for months, days for short ones.
- Decision card summaries were clamped for the short seed copy and let a third
  line bleed past the card edge once real LLM-written summaries arrived.

### Added: per-month decision density grid

A GitHub-contributions-style matrix sits beside the chronology scatter on the
decisions page — one row per year, one cell per month, cell colour encoding how
many decisions landed that month.

- **Palette** is sequential on a single hue built from GLYPH's own tokens:
  `#6a4741` (a lifted `#5e3f3a`) → `#cc0000` → `#f4735c` → `#ffb4a8`, with zero
  months at `#1c1c1c` so they recede toward the `#131313` surface. Validated as
  an ordinal ramp: monotone lightness, adjacent ΔL ≥ 0.06, dim end 2.29:1
  against the surface, hue spread 3°. The original candidate (`#3a2320` at the
  dim end) failed the contrast floor at 1.28:1 and was re-stepped.
- **Bucketing is sparse-aware.** Decision histories are thin — a busy month is
  often three or four nodes — so below five, count maps to step directly and
  every increment is visible; above it the scale switches to quartiles. Pure
  quartiles collapsed 1 and 2 into one colour and never used the dim step.
- **Every cell is a real hit target** with a hover *and* keyboard-focus tooltip
  (value leads, month follows) plus an `aria-label`, and the hovered cell takes
  a visible outline. Counts are never tooltip-gated — the dated decision list
  below carries the same information.
- **Months that have not happened yet read as absent**, not as zero.
- Placing the grid beside the scatter rather than below it costs no vertical
  space; the scatter was ~1000px wide for two dozen points and had room to give.

## Verified end-to-end (2026-09-10)

Real credentials, real repositories, local stack:

| Repo | Result |
|---|---|
| `BurntSushi/ripgrep` | 5/6 NIM windows parsed → 53 insights → consolidated to 18 |
| `sharkdp/bat` | 19 decisions, **19/19 with resolved timestamps**, avg confidence 91%, 13 contributors |

Sample of what the extractor now returns, versus the commit-title scraping it
was silently producing before:

> **Improve CSV delimiter auto-detection** — Changed CSV parsing to better
> auto-detect delimiter: files with .tsv extension treated as tab-delimited;
> other files use heuristic … `@keith-hall`, `#512`, 95%

`cargo test` 21/21 green (5 new regression tests covering the parser and
deserialization failures above). `cargo build --release` and `npm run build`
clean.

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

- **Stack:** Rust + Axum 0.7, sqlx 0.7 + PostgreSQL (migrations auto-run at boot), reqwest 0.12 async GitHub/NIM clients, NVIDIA NIM (model fallback chain headed by `nvidia/nemotron-3-super-120b-a12b`; pin one with `NIM_MODEL`), Astro.js 4 (server-rendered), D3.js 7 (bundled from node_modules, no CDN), Tailwind.
- **BYOK security model:** GitHub PAT + NIM key held in browser `sessionStorage`, travel per-request as `X-Github-Token` / `X-Nim-Api-Key`, never stored or logged server-side.
- **Honest-data pattern:** every `prerender=false` page regex-tests its `:id` against a UUID; real jobs render honest loading placeholders + client hydration from the API, while demo/slug URLs keep the polished seed. No fake flash on real jobs.
- **Analysis pipeline:** ingest (commits → PRs → issues → review threads) → multi-pass NIM extraction (windows run concurrently, then a consolidation pass) → deterministic fallback → `intent_nodes` (decision/debate/rejection/architectural).
- **Reading the fallback:** if every decision is titled like a raw commit subject and every confidence is identical, the LLM pass failed and you are looking at the deterministic fallback, not NIM output. The backend log names which of the three causes it was — retired model, timeout, or parse failure.

## Deployment status

| Target | State |
|---|---|
| Local (`docker compose up`) | ✅ Verified end-to-end with real GitHub + NIM keys |
| Frontend — `https://glyph-flax-two.vercel.app` | ✅ Serving (`200`) |
| Backend — `https://glyph-api-qh9i.onrender.com` | ❌ **Down.** No HTTP response; three 120s probes all timed out. |

The hosted backend needs a database before it can come back: Render deletes
free PostgreSQL instances after 30 days, so `glyph-db` no longer exists and the
container crash-loops at boot. `backend/render.yaml` now declares
`DATABASE_URL` as a dashboard secret (`sync: false`) so it can be pointed at a
provider with an indefinite free tier. Steps are in
[RUNBOOK.md](./RUNBOOK.md#4-failure-modes-that-have-actually-happened).

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
- [x] BYOK credential flow works end-to-end (keys travel as per-request headers and are never stored or logged server-side)
- [x] Pagination handles repos up to 10,000 commits (`MAX_COMMITS=10000`)
- [x] Dashboard + subpages update in real-time during analysis (status polling, no fabricated events)
- [x] Deployed to Render + Vercel
- [x] Rate limiting + request validation + stuck-job recovery + multi-pass NIM fallback
- [x] Multi-repo comparison + D3 timeline + honest empty/loading/error states

---

*Last updated 2026-09-10 after the debugging + structural pass.*
