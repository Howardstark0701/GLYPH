# GLYPH Project Progress Checklist

## Overview
**Project Goal:** Extract decision history from GitHub repositories using AI intelligence pipeline
**Current Status:** Phases 1–7 complete — rich-ingestion backend, honest frontend data plumbing, Stitch design pass, deployed (Render + Vercel)
**Last Updated:** August 7, 2026

## Latest Session (2026-08-07) — runtime smoke test + full-stack polish

- 🐛 **Migration fix** — `002_rich_ingestion.sql` used unquoted `user` as a column
  name (reserved word in PostgreSQL); the schema **failed to apply on any fresh
  database**. Quoted as `"user"` (INSERTs already quoted it). Verified end-to-end:
  backend boots against Docker Postgres 16, all migrations apply cleanly.
- ✅ **Runtime smoke test** — seeded a repo + 4 intent nodes via psql and
  validated every GET endpoint against the live server:
  `/status`, `/intent`, `/graph`, `/decisions`, `/rejections`, `/contributors`,
  `/debates` (200s with correct shapes); `/summary` returns 401 without a NIM key;
  missing keys → 401; unknown id → 404.
- ✅ **Contributors page** — now fully re-renders real data (cards, registry
  stats, influence histogram, pattern breakdown) from `/contributors`. Previously
  it fetched but never rendered ("full re-render reserved for v2").
- ✅ **Debates page** — now hydrates thread id/topic/status and agreement/
  contention metrics from `/debates` instead of leaving static seed text.
- 🆕 **Report export feature** — dashboard **EXPORT** button downloads a bundled
  JSON report + Markdown brief (status, narrative, decisions, debates,
  rejections, contributors, graph). Client-side; BYOK keys never persist.
- ✅ **Verified** — `cargo test` 11/11 green, `npm run build` clean, all 6 routes
  render 200 through the dev server. Committed as `157e4ac`.

## Task Board (last working session)

- ✅ **Task 1 — Backend: rich ingestion** — migration `002_rich_ingestion.sql` (unique constraints, `repos.error_message` + `repos.stage`, PR review/PR comment/issue comment tables); shared `gh_get` helper with 403/429 Retry-After backoff; full PR reviews / PR inline comments / issue comment thread ingestion; `analyze.rs` rewritten with host-validated URL parsing, staged pipeline (`ingesting_commits → … → extracting_intent`), cancel flags.
- ✅ **Task 2 — Backend: reliability** — retry/backoff on GitHub 403/429; host-validated `parse_github_url`; resilient `parse_insights` (bare → fenced → prose → object wrapper); configurable `NIM_MODEL` + consolidated `chat()`.
- ✅ **Task 3 — Backend: terminate endpoint + router cleanup** — `POST /api/repo/:id/terminate` via in-memory `AtomicBool` cancellation; router consolidated to single `api::routes()`; dead code removed (db module, legacy wrappers, graph edges).
- ✅ **Task 4 — Backend: tests + cargo green** — `cargo check --all-targets` clean (0 warnings); `cargo test` 11/11 passed.
- ✅ **Task 5 — Frontend functional** — `[id].astro`: real `/terminate` wiring, real status-polled stream, honest graph states (`ANALYSIS_FAILED / TERMINATED / NO_INTENT / SYNCING`), removed fake demo data; landing syslog trimmed to boot-only events; shared `src/scripts/repo-view.ts` wired into all 5 subpages (real jobs stream real stages, honest empty/error states); 5 dead stub components deleted.
- ✅ **Task 6 — UI/UX design pass** — `global.css` rebuilt as a design-token system; flattening `!important` reset removed from `index.astro` + `[id].astro` + all 5 subpages (radius/shadows/transitions restored); ambient hero glow + `:focus-within` input state; dashboard chrome polish (nav active state + TERMINAL item, graph-panel inset frame + faint grid, live-stream pulse, heartbeat no longer fights real `/status`).
- ⬜ **Task 7 — Build verify + docs + hygiene + commit** — *in progress at last session*; builds pass, docs updated, commits pending.

## Phase Completion Status

### ✅ **Phase 1: Rust Project Setup + GitHub Ingestion** - **95% COMPLETE**
- [x] Cargo.toml with all dependencies
- [x] Project folder structure
- [x] Async GitHub API client (reqwest)
- [x] Ingestion layer modules: commits, issues, pull_requests
- [x] Rate limit awareness
- [ ] Full pagination handling (partial)
- [ ] Error recovery for failed API calls

### ✅ **Phase 2: PostgreSQL Schema + SQLx Integration** - **90% COMPLETE**
- [x] Database migration file (001_initial.sql)
- [x] All table schemas: repos, commits, pull_requests, issues, intent_nodes
- [x] SQLx models with proper typing
- [x] Database connection pool
- [x] Indexes for performance
- [ ] Advanced query optimization
- [ ] Bulk insert operations

### ✅ **Phase 3: NVIDIA NIM Pipeline** - **70% COMPLETE**
- [x] NIM client implementation
- [x] Prompt engineering module
- [x] Structured JSON parsing for insights
- [ ] Full event batching system
- [ ] Confidence scoring refinement
- [ ] Multi-pass analysis for complex repos
- [ ] Fallback mechanisms for NIM failures

### ✅ **Phase 4: Axum REST API** - **85% COMPLETE**
- [x] All core endpoints defined (8 routes)
- [x] BYOK credential extraction from headers
- [x] Async job processing with tokio spawn
- [x] Error handling with AppError enum
- [x] CORS configuration for frontend
- [ ] Rate limiting middleware
- [ ] Request validation and sanitization
- [ ] Comprehensive API documentation
- [ ] Swagger/OpenAPI spec generation

### ✅ **Phase 5: Astro.js UI Foundation** - **80% COMPLETE**
- [x] Landing page with BYOK credential flow
- [x] Terminal-style dark UI aesthetic
- [x] Repository dashboard layout
- [x] Live system logs and status indicators
- [x] SVG scroll-trace visualization
- [ ] Full API integration for analysis triggering
- [ ] Loading states and progress tracking
- [ ] Error handling in UI
- [ ] Responsive design refinements

### 🔄 **Phase 6: D3.js Decision Graph + Timeline** - **40% COMPLETE**
- [ ] D3.js force-directed graph implementation
- [ ] Interactive node zoom/pan
- [ ] Decision timeline visualization
- [ ] Debate explorer component
- [ ] Contributor cards with reasoning profiles
- [ ] Rejection vault display
- [ ] Graph pagination and filtering

### ✅ **Phase 7: Polish + Deploy** — **COMPLETE**
- [x] Vercel config (`vercel.json` + `astro.config.mjs` with `output: 'server'` + Vercel serverless adapter for dynamic routes)
- [x] Shuttle.rs replaced with Render (Shuttle shut down April 2026)
- [x] Multi-stage `Dockerfile` for backend (Rust → debian:bookworm-slim)
- [x] `render.yaml` with web service + managed PostgreSQL
- [x] `main.rs` rewritten as plain `#[tokio::main]` (no Shuttle macros)
- [x] `Cargo.toml` cleaned of dead shuttle-* dependencies
- [x] CORS reads `FRONTEND_URL` from env var
- [x] `.gitignore` covers `.env`, `dist/`, `target/`
- [x] Render account created + service deployed → `https://glyph-api-u495.onrender.com`
- [x] Vercel project created + `PUBLIC_API_BASE_URL` env var set
- [x] Frontend deployed → `https://glyph-pua2jbu7s-glyph-tango.vercel.app`
- [x] `FRONTEND_URL` updated in Render env vars after Vercel deploy

## Current Implementation Details

### ✅ **Working Components**
1. **Backend Server** - Fully functional Axum server with PostgreSQL
2. **Database Schema** - All tables created with proper relationships
3. **GitHub Ingestion** - Fetches commits, PRs, issues from API
4. **NIM Integration** - Basic AI analysis pipeline
5. **Frontend Landing** - Complete BYOK credential flow with scroll visualization
6. **Dashboard Layout** - Terminal-style UI with navigation panels

### 🔧 **Partially Implemented**
1. **D3.js Visualization** - Placeholder structure exists, needs actual graph logic
2. **Live Data Updates** - UI has stream panel but needs real data integration
3. **Error Recovery** - Basic error handling, needs comprehensive strategy
4. **Performance** - Works for small repos, needs optimization for large ones

### ❌ **Not Started / Missing**
1. **Production Deployment** - Local development only
2. **Advanced Features**:
   - Multi-repository comparison
   - Export functionality (JSON, PDF)
   - Advanced filtering and search
   - User preferences/saved analyses
   - API key management UI
3. **Testing Suite** - Unit tests, integration tests
4. **Documentation** - User guides, API documentation

## Immediate Next Steps (Priority Order)

### **HIGH PRIORITY** - Core functionality
1. ✅ **D3.js graph visualization** — real graph renders from `/api/repo/:id/graph` (force simulation, type-colored nodes, drag); honest empty/error/pending states instead of fake data
2. ✅ **Connect frontend to backend API** — landing → `/api/analyze` → `/repo/:id` dashboard; subpages hydrate from `/decisions`, `/summary`, etc.
3. ✅ **Implement live data streaming** — `/status` polling every 3s with real stage labels; heartbeat reflects real status
4. ✅ **Add comprehensive error handling** — `error_message` surfaced to UI; honest `ANALYSIS_FAILED` / `TERMINATED` / `NO_INTENT` states

### **MEDIUM PRIORITY** - Polish and reliability
5. ⬜ **Implement rate limiting** - Protect API from abuse
6. ⬜ **Add loading states and progress bars** - Better user experience
7. ⬜ **Optimize database queries** - Performance for large repos
8. ✅ **Write basic tests** — 11 backend unit tests (URL parsing, NIM JSON parsing, fence stripping) green

### **LOW PRIORITY** - Advanced features
9. ✅ **Production deployment setup** — Render (plain Axum + PostgreSQL) + Vercel; Shuttle retired
10. ⬜ **Multi-repo comparison** - Advanced analysis feature
11. ⬜ **Export functionality** - Share analysis results
12. ⬜ **User authentication** - Optional enhancement

## Known Issues / Technical Debt

1. **Frontend API Integration** - Landing page form doesn't actually trigger analysis
2. **Graph Visualization** - Placeholder SVG without actual D3.js logic
3. **Error Recovery** - Network failures may leave jobs stuck in "processing"
4. **Memory Management** - Large repos could exhaust memory
5. **Security** - Need input validation and sanitization
6. **Performance** - No caching of API responses
7. **Testing** - No automated tests

## Bug Fixes Applied (June 11, 2026)

- ✅ **Bug 1 — sessionStorage writes in index.astro**: `confirmBtn` now writes keys to `sessionStorage` on confirm; keys are restored from `sessionStorage` on page load so the indicator is correct on refresh.
- ✅ **Bug 2 — broken CSS link in index.astro**: Removed broken static `<link>` to `global.css`; CSS is imported via Astro frontmatter `import` instead, which works correctly on Vercel.
- ✅ **Bug 3 — API_BASE hardcoded in subpages**: All 5 subpages (decisions, summary, contributors, rejections, debates) now use `(window as any).__GLYPH_API__ ?? (import.meta as any).env?.PUBLIC_API_BASE_URL ?? 'http://localhost:8000'`.
- ✅ **Bug 4 — API_BASE hardcoded in `[id].astro` execute-fetch**: The `initExecuteFetch` function now uses `(import.meta as any).env?.PUBLIC_API_BASE_URL ?? 'http://localhost:8000'`.
- ✅ **Bug 5 — slug extraction from DOM scraping in `[id].astro`**: Fixed slug extraction to use URL path segments directly instead of fragile DOM text scraping.
- ✅ **Bug 6 — second hardcoded API_BASE in debates.astro `hydrateDebates`**: Removed inner `const API_BASE` override so the function uses the outer scoped env-aware variable.

## Completed Files Review

### ✅ **Backend (Rust)**
- `backend/Cargo.toml` - Complete dependency setup
- `backend/src/main.rs` - Axum server with Shuttle.rs integration
- `backend/src/db/models.rs` - All SQLx models defined
- `backend/src/api/*.rs` - All route handlers implemented
- `backend/src/ingestion/*.rs` - GitHub API clients
- `backend/src/intelligence/*.rs` - NIM integration
- `backend/migrations/001_initial.sql` - Full database schema

### ✅ **Frontend (Astro.js)**
- `frontend/package.json` - Complete dependency setup
- `frontend/src/pages/index.astro` - Full landing page with BYOK flow
- `frontend/src/pages/repo/[id].astro` - Dashboard layout
- `frontend/src/components/*.astro` - UI component stubs
- `frontend/src/styles/global.css` - Base styling

## Dependencies Status

### ✅ **Installed and Working**
- Rust: Axum, Tokio, SQLx, reqwest, serde
- Frontend: Astro.js, Tailwind CSS, D3.js, TypeScript

### ⚠️ **Configuration Required**
- NVIDIA NIM API key (user-provided)
- GitHub Personal Access Token (user-provided)
- PostgreSQL database connection (for deployment)

## Progress Metrics
- **Codebase:** ~80% complete
- **Core Features:** ~70% implemented
- **UI/UX:** ~75% complete
- **Backend API:** ~85% complete
- **Database:** ~90% complete
- **AI Integration:** ~70% complete
- **Deployment:** ~10% complete

## Estimated Completion Time
- **Minimum Viable Product:** 2-3 weeks (complete D3.js, API integration)
- **Full Feature Complete:** 4-6 weeks (all phases, polish, deployment)
- **Production Ready:** 6-8 weeks (testing, optimization, documentation)

## Success Criteria
- [ ] User can enter GitHub repo URL and get analysis
- [ ] D3.js graph shows decision nodes and relationships
- [ ] All API endpoints return correct data
- [ ] BYOK credential flow works end-to-end
- [ ] System handles repositories up to 10,000 commits
- [ ] Dashboard updates in real-time during analysis
- [ ] Project deployed to Shuttle.rs + Vercel
- [ ] Basic documentation available

---

*Last updated by analyzing codebase structure and implementation status on June 11, 2026.*