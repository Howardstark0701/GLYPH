# GLYPH
> Extract the *why* layer hidden inside any GitHub repository.

---

## What is GLYPH?

GLYPH is an open-source intelligence backend that takes a GitHub repository URL and reconstructs the **decision history** of that codebase — what was debated, what was rejected, what architectural choices were made and why. It turns thousands of commits, pull requests, issues, and review threads into a structured, queryable intelligence document powered by AI.

Think of it as: **"Git log tells you what changed. GLYPH tells you why."**

---

## The Core Problem It Solves

When a developer joins a new codebase, the institutional reasoning behind every architectural decision is buried across thousands of GitHub threads. Why was this library chosen over that one? Why was this feature removed? What was debated before the current structure was settled on?

GLYPH surfaces all of that as a clean API with an extraordinary visual dashboard.

---

## Vision

- Intelligence infrastructure, not a developer tool
- Open source, built to be featured on GitHub, LinkedIn, and resume
- Palantir-style data platform aesthetics — dark, dense, information-rich
- Target users: developers onboarding to new codebases, OSS researchers, engineering teams

---

## Full Tech Stack

| Layer | Technology |
|---|---|
| Language | Rust |
| Web Framework | Axum |
| Async Runtime | Tokio |
| HTTP Client | reqwest |
| Database | PostgreSQL |
| ORM/Query | SQLx |
| AI Layer | NVIDIA NIM (LLM API) |
| Serialization | Serde / Serde JSON |
| Frontend Framework | Astro.js + TypeScript |
| Styling | Tailwind CSS |
| Graph Visualization | D3.js |
| Backend Deployment | Render (Docker) |
| Frontend Deployment | Vercel |
| Database Hosting | Render PostgreSQL (free tier) |

---

## System Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        GLYPH                              │
│                                                             │
│  ┌─────────────┐   ┌──────────────┐   ┌─────────────────┐  │
│  │  INGESTION  │──▶│  PROCESSING  │──▶│  INTELLIGENCE   │  │
│  │    LAYER    │   │    LAYER     │   │     LAYER       │  │
│  └─────────────┘   └──────────────┘   └─────────────────┘  │
│         │                 │                    │            │
│         ▼                 ▼                    ▼            │
│  ┌─────────────┐   ┌──────────────┐   ┌─────────────────┐  │
│  │  GitHub API │   │  PostgreSQL  │   │   NVIDIA NIM    │  │
│  │  (reqwest)  │   │   (SQLx)     │   │   (LLM API)     │  │
│  └─────────────┘   └──────────────┘   └─────────────────┘  │
│                                                │            │
│                                       ┌─────────────────┐  │
│                                       │   REST API      │  │
│                                       │   (Axum)        │  │
│                                       └─────────────────┘  │
│                                                │            │
│                                       ┌─────────────────┐  │
│                                       │   GLYPH UI    │  │
│                                       │  (Astro.js +    │  │
│                                       │   D3.js)        │  │
│                                       └─────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

---

## Layer-by-Layer Breakdown

### 1. Ingestion Layer
**Tech:** Rust + reqwest + tokio (async)

Pulls raw data from the GitHub REST API for a single target repository:
- All commits (message, author, timestamp, files changed)
- All pull requests (title, description, state, linked issues)
- All issues (title, body, labels, comments)
- Code review threads (inline comments, approval/rejection history)
- Branch names and merge patterns

All requests are async and rate-limit aware. GitHub token authentication supported.

---

### 2. Processing Layer
**Tech:** Rust + SQLx + PostgreSQL

Cleans, structures, and stores raw GitHub data:
- Normalizes all timestamps to UTC
- Links commits → PRs → issues into unified event chains
- Builds a chronological event graph per repo
- Deduplicates overlapping references
- Caches analysis results to avoid redundant API calls

---

### 3. Intelligence Layer
**Tech:** Rust → NVIDIA NIM (LLM)

The core AI brain. Feeds structured event chains into NIM and extracts:
- **Decision nodes** — what was decided, when, and by whom
- **Debate threads** — what was argued, what position lost
- **Rejection records** — what was tried, built, or proposed then abandoned
- **Architectural intent** — why the codebase structure is what it is
- **Contributor reasoning profiles** — who drove which kinds of decisions
- **Repo narrative** — a human-readable story of the project's evolution

---

### 4. API Layer
**Tech:** Axum (Rust)

Clean REST endpoints served from Shuttle.rs:

```
POST /analyze                    → trigger full repo analysis (async job)
GET  /repo/:id/status            → check analysis job status
GET  /repo/:id/intent            → full extracted intent map
GET  /repo/:id/debates           → all debate threads extracted
GET  /repo/:id/decisions         → chronological decision timeline
GET  /repo/:id/rejections        → abandoned ideas and why
GET  /repo/:id/contributors      → per-contributor reasoning profiles
GET  /repo/:id/graph             → full decision graph as JSON (for D3)
GET  /repo/:id/summary           → AI-generated repo narrative story
```

All responses return structured JSON. Analysis jobs are async — trigger via POST, poll status, then fetch results.

---

### 5. UI Layer
**Tech:** Astro.js + TypeScript + Tailwind CSS + D3.js

Dark intelligence dashboard aesthetic — think war room, not SaaS tool.

Key UI components:
- **Repo input screen** — enter GitHub URL, trigger analysis with animated progress
- **Decision timeline** — animated horizontal timeline of key decisions
- **Interactive graph** — D3.js force graph of decision nodes, debate edges, rejection forks
- **Debate explorer** — expandable threads showing what was argued and what won
- **Contributor cards** — AI-extracted reasoning style per contributor
- **Repo story** — full AI narrative of the project's evolution, rendered as a readable document
- **Rejection vault** — the graveyard of abandoned ideas

Color palette: near-black background, electric blue/cyan accents, amber highlights for conflicts, red for rejections.

### ✦ Scroll-trace BYOK explainer (landing page hero section)

One of the signature UI moments on the landing page. A full-height dark section where a **thin glowing SVG path traces the BYOK credential flow as the user scrolls down**. The line is pre-drawn but invisible — `stroke-dashoffset` animates from full to zero driven by scroll position. A glowing dot rides `getPointAtLength()` along the curve in real time.

**How it works:**
- SVG path is drawn in two layers — a blurred ambient glow layer (`feGaussianBlur` filter) under a thin bright core line
- `stroke-dasharray` + `stroke-dashoffset` technique draws the line progressively as scroll progress increases
- A moving dot follows the exact curve position using `SVGPathElement.getPointAtLength()`
- Node cards (step 1–5) fade in and float up exactly when the line reaches their point in the flow
- Scroll listener hooks via `IntersectionObserver` — only active when the section is in view

**The 5 nodes it traces:**
1. You → enter GitHub PAT + NIM key in the setup screen
2. GLYPH UI → keys held in Astro component state, memory only
3. GLYPH API → keys travel as `X-Github-Token` / `X-Nim-Api-Key` request headers
4. GitHub API + NIM → your rate limit, your quota, no shared pool
5. Request ends → keys discarded, never written to DB, never logged

**Implementation notes for Astro.js:**
- Build as a standalone Astro component: `BYOKTrace.astro`
- Dark section with `background: #08090d`, full viewport height per scroll step
- SVG is `position: sticky; top: 0; height: 100vh` inside a tall scroll container (~2200px)
- Node cards are `position: absolute` at staggered `top` values, fade in on scroll trigger
- Scroll listener uses `passive: true` for performance
- Wrap in `IntersectionObserver` to pause animation when off-screen

---

## Database Schema (PostgreSQL)

```sql
-- Repos being analyzed
repos (
  id UUID PRIMARY KEY,
  github_url TEXT NOT NULL,
  owner TEXT,
  name TEXT,
  analyzed_at TIMESTAMP,
  status TEXT  -- pending | processing | complete | failed
)

-- Raw ingested commits
commits (
  id UUID PRIMARY KEY,
  repo_id UUID REFERENCES repos(id),
  sha TEXT,
  message TEXT,
  author TEXT,
  timestamp TIMESTAMP,
  files_changed JSONB
)

-- Raw ingested pull requests
pull_requests (
  id UUID PRIMARY KEY,
  repo_id UUID REFERENCES repos(id),
  number INT,
  title TEXT,
  body TEXT,
  state TEXT,
  author TEXT,
  created_at TIMESTAMP,
  merged_at TIMESTAMP
)

-- Raw ingested issues
issues (
  id UUID PRIMARY KEY,
  repo_id UUID REFERENCES repos(id),
  number INT,
  title TEXT,
  body TEXT,
  state TEXT,
  author TEXT,
  created_at TIMESTAMP,
  comments JSONB
)

-- AI-extracted intelligence
intent_nodes (
  id UUID PRIMARY KEY,
  repo_id UUID REFERENCES repos(id),
  node_type TEXT,  -- decision | debate | rejection | architectural
  title TEXT,
  summary TEXT,
  reasoning TEXT,
  contributors JSONB,
  source_refs JSONB,  -- linked commit/PR/issue IDs
  timestamp TIMESTAMP,
  confidence FLOAT
)
```

---

## Project Phase Plan

| Phase | Goal | Key Deliverables |
|---|---|---|
| **Phase 1** | Rust project setup + GitHub ingestion | Cargo.toml, folder structure, async GitHub scraper |
| **Phase 2** | PostgreSQL schema + SQLx integration | Migrations, models, DB connection pool |
| **Phase 3** | NVIDIA NIM pipeline | Prompt engineering, intent extraction, structured JSON output |
| **Phase 4** | Axum REST API | All endpoints wired, JSON responses, error handling |
| **Phase 5** | Astro.js UI foundation | Dark dashboard layout, repo input, analysis trigger |
| **Phase 6** | D3.js decision graph + timeline | Interactive graph, animated timeline, debate explorer |
| **Phase 7** | Polish + deploy | Shuttle.rs backend, Vercel frontend, Railway DB |

---

## Folder Structure

```
glyph/
├── backend/                  # Rust Axum backend
│   ├── Cargo.toml
│   ├── src/
│   │   ├── main.rs           # Axum server entry + Shuttle annotation
│   │   ├── ingestion/        # GitHub API scrapers
│   │   │   ├── mod.rs
│   │   │   ├── commits.rs
│   │   │   ├── pull_requests.rs
│   │   │   └── issues.rs
│   │   ├── processing/       # Data cleaning + event graph builder
│   │   │   ├── mod.rs
│   │   │   └── linker.rs
│   │   ├── intelligence/     # NVIDIA NIM integration
│   │   │   ├── mod.rs
│   │   │   ├── client.rs
│   │   │   └── prompts.rs
│   │   ├── api/              # Axum route handlers
│   │   │   ├── mod.rs
│   │   │   ├── analyze.rs
│   │   │   └── repo.rs
│   │   ├── db/               # SQLx models + queries
│   │   │   ├── mod.rs
│   │   │   ├── models.rs
│   │   │   └── queries.rs
│   │   └── errors.rs         # Unified error types
│   └── migrations/           # PostgreSQL migration files
│
├── frontend/                 # Astro.js frontend
│   ├── astro.config.mjs
│   ├── package.json
│   ├── src/
│   │   ├── pages/
│   │   │   ├── index.astro   # Landing + repo input
│   │   │   └── repo/
│   │   │       └── [id].astro  # Analysis dashboard
│   │   ├── components/
│   │   │   ├── RepoInput.astro
│   │   │   ├── DecisionGraph.astro   # D3.js graph
│   │   │   ├── Timeline.astro
│   │   │   ├── DebateExplorer.astro
│   │   │   └── ContributorCard.astro
│   │   └── styles/
│   │       └── global.css
│   └── public/
│
├── GLYPH.md                # This file
└── README.md
```

---

## Key Dependencies

### Rust (Cargo.toml)
```toml
[dependencies]
axum = "0.7"
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", features = ["json"] }
sqlx = { version = "0.7", features = ["postgres", "runtime-tokio", "uuid", "chrono"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
uuid = { version = "1", features = ["v4"] }
chrono = { version = "0.4", features = ["serde"] }
shuttle-axum = "0.48"
shuttle-runtime = "0.48"
shuttle-shared-db = { version = "0.48", features = ["postgres"] }
```

### Frontend (package.json)
```json
{
  "dependencies": {
    "astro": "^4.0.0",
    "@astrojs/tailwind": "^5.0.0",
    "d3": "^7.0.0",
    "tailwindcss": "^3.0.0",
    "typescript": "^5.0.0"
  }
}
```

---

## Credentials Model — BYOK (Bring Your Own Keys)

GLYPH does **not** store or provide any API keys. Users supply their own credentials on first use. This keeps costs off the maintainer, rate limits per-user, and makes the platform fully transparent.

### What the user provides on first visit:
| Credential | Where to get it | Why it's needed |
|---|---|---|
| GitHub Personal Access Token | github.com → Settings → Developer Settings → PAT | Fetching commits, PRs, issues |
| NVIDIA NIM API Key | build.nvidia.com → Get API Key | Running the AI intelligence layer |

### How credentials flow through the system:

```
User enters keys in UI
        ↓
Frontend sends keys in request headers (never stored in browser localStorage)
        ↓
Backend receives keys per-request (never written to DB)
        ↓
Backend uses keys for GitHub API + NIM calls for that session only
        ↓
Keys are discarded after request completes
```

### Key principles:
- **Never persisted** — keys are used in-memory per request only, never written to PostgreSQL
- **Never logged** — keys are explicitly excluded from any request logging middleware
- **User-owned rate limits** — GitHub's 5000 req/hr and NIM quotas apply to the user's own account
- **Transparent** — UI clearly explains what each key is used for before asking

### UI credential flow:
1. Landing page has a **"Setup Keys"** step before analysis
2. User enters GitHub PAT + NIM API key in a secure input form
3. Keys are held in frontend memory (React/Astro state) for the session only
4. Every API call to the GLYPH backend passes keys as request headers:
   - `X-Github-Token: ghp_...`
   - `X-Nim-Api-Key: nvapi-...`
5. On page refresh, keys are cleared — user re-enters next session

### Backend header extraction (Axum):
```rust
// Axum extracts credentials from headers per request
// No global state, no DB writes, no logging of key values
async fn analyze_repo(
    headers: HeaderMap,
    Json(payload): Json<AnalyzeRequest>,
) -> Result<Json<AnalyzeResponse>, AppError> {
    let github_token = headers
        .get("X-Github-Token")
        .and_then(|v| v.to_str().ok())
        .ok_or(AppError::MissingCredentials)?;

    let nim_api_key = headers
        .get("X-Nim-Api-Key")
        .and_then(|v| v.to_str().ok())
        .ok_or(AppError::MissingCredentials)?;

    // Pass downstream — never store
}
```

---

## Environment Variables

```env
# Backend (Shuttle.rs) — no API keys needed server-side
DATABASE_URL=postgresql://...      # Railway PostgreSQL connection string
NIM_BASE_URL=https://integrate.api.nvidia.com/v1  # NIM endpoint (public, not secret)

# Frontend (Vercel)
PUBLIC_API_BASE_URL=https://...    # Shuttle.rs backend URL
```

> Note: Unlike PRETO, GLYPH requires **zero secret API keys on the server**. The only server-side secret is the database connection string. All GitHub and NIM credentials come from the user at runtime.

---

## NVIDIA NIM Prompt Strategy

GLYPH uses NIM with structured prompting. Each call sends a batch of related GitHub events (commits + linked PR + issue thread) and asks for:

1. Was a **decision** made here? If so, what was decided?
2. Was anything **debated**? What positions were taken?
3. Was anything **rejected or abandoned**? What and why?
4. What does this tell us about **architectural intent**?

Output is always requested as structured JSON for deterministic parsing.

---

## Deployment Architecture

```
User → Vercel (Astro frontend)
          ↓ API calls
     Shuttle.rs (Axum backend)
          ↓ queries
     Railway (PostgreSQL)
          ↓ AI calls
     NVIDIA NIM API
          ↓ data fetch
     GitHub REST API
```

---

## What Makes GLYPH Unique

- No existing tool extracts **reasoning and intent** from git history — only diffs and stats
- Frames developer activity as **cognitive archaeology**, not metrics
- Built in Rust for performance — can process large repos (thousands of commits) efficiently
- The output is **intelligence**, not a dashboard — structured data that answers questions
- Directly addresses a real pain point: onboarding to unfamiliar codebases takes weeks because institutional knowledge is invisible

---

## Resume / Portfolio Positioning

> "GLYPH is an open-source intelligence backend built in Rust that uses AI to reconstruct the decision history of any GitHub repository — surfacing debates, rejections, and architectural reasoning that git history obscures. Stack: Rust, Axum, PostgreSQL, NVIDIA NIM, Astro.js, D3.js. Deployed on Shuttle.rs + Vercel."

---

*This document is the single source of truth for GLYPH. Update it as the project evolves.*
