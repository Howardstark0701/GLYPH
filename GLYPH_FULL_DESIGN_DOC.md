# GLYPH — Complete Design & Planning Document for Claude

> Generated 2026-06-30. Contains all project planning, UI design specs, Stitch design system, and current codebase state.

---

## Table of Contents

1. [Project Overview](#1-project-overview)
2. [Tech Stack](#2-tech-stack)
3. [Architecture](#3-architecture)
4. [Stitch Design System (Canonical Source of Truth)](#4-stitch-design-system-canonical-source-of-truth)
5. [Stitch Screen Mockups](#5-stitch-screen-mockups)
6. [Current UI Implementation](#6-current-ui-implementation)
7. [BYOK Credential Flow](#7-byok-credential-flow)
8. [Database Schema](#8-database-schema)
9. [Project Progress](#9-project-progress)
10. [Known Issues](#10-known-issues)
11. [Deployment](#11-deployment)

---

# 1. Project Overview

**GLYPH** is an open-source intelligence backend that takes a GitHub repository URL and reconstructs the **decision history** of that codebase — what was debated, rejected, and which architectural choices were made and why.

**Tagline:** "Git log tells you *what* changed. GLYPH tells you *why*."

**Target Users:** Developers onboarding to new codebases, OSS researchers, engineering teams.

**Key Insight:** When a developer joins a new codebase, the institutional reasoning behind every architectural decision is buried across thousands of GitHub threads. GLYPH surfaces all of that as a clean API with an extraordinary visual dashboard.

**Vision:** Intelligence infrastructure, not a developer tool. Open source, built to be featured on GitHub/LinkedIn/resume. Palantir-style data platform aesthetics — dark, dense, information-rich.

---

# 2. Tech Stack

| Layer | Technology |
|---|---|
| **Language** | Rust |
| **Web Framework** | Axum 0.7 |
| **Async Runtime** | Tokio (full) |
| **HTTP Client** | reqwest 0.12 (json) |
| **Database** | PostgreSQL |
| **ORM/Query** | SQLx 0.7 (postgres, uuid, chrono) |
| **AI Layer** | NVIDIA NIM (LLM API via build.nvidia.com) |
| **Serialization** | Serde / Serde JSON |
| **Frontend Framework** | Astro.js 4.x + TypeScript |
| **Styling** | Tailwind CSS 3.x **+** heavy inline `<style>` blocks |
| **Graph Visualization** | D3.js 7.x |
| **Auth Model** | BYOK (Bring Your Own Keys) — keys in headers, never persisted |
| **Backend Deployment** | Render (Docker) |
| **Frontend Deployment** | Vercel |
| **Database Hosting** | Render PostgreSQL (free tier) |

---

# 3. Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        GLYPH                                │
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
│                                       │   GLYPH UI      │  │
│                                       │  (Astro.js +    │  │
│                                       │   D3.js)        │  │
│                                       └─────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

## Layer Breakdown

### 1. Ingestion Layer (Rust + reqwest + tokio)
Pulls raw data from GitHub REST API:
- All commits (message, author, timestamp, files changed)
- All pull requests (title, description, state, linked issues)
- All issues (title, body, labels, comments)
- Code review threads (inline comments, approval/rejection history)
- Branch names and merge patterns

### 2. Processing Layer (Rust + SQLx + PostgreSQL)
Cleans, structures, stores:
- Normalizes timestamps to UTC
- Links commits → PRs → issues into unified event chains
- Chronological event graph per repo
- Deduplicates overlapping references
- Caches analysis results

### 3. Intelligence Layer (Rust → NVIDIA NIM LLM)
Feeds structured event chains into NIM and extracts:
- **Decision nodes** — what was decided, when, by whom
- **Debate threads** — what was argued, what position lost
- **Rejection records** — what was tried then abandoned
- **Architectural intent** — why structure is what it is
- **Contributor reasoning profiles** — who drove which decisions
- **Repo narrative** — human-readable story of evolution

### 4. API Layer (Axum)
```
POST /analyze                    → trigger full analysis (async job)
GET  /repo/:id/status            → check job status
GET  /repo/:id/intent            → full extracted intent map
GET  /repo/:id/debates           → all debate threads
GET  /repo/:id/decisions         → chronological decision timeline
GET  /repo/:id/rejections        → abandoned ideas and why
GET  /repo/:id/contributors      → per-contributor reasoning profiles
GET  /repo/:id/graph             → full decision graph as JSON (for D3)
GET  /repo/:id/summary           → AI-generated repo narrative story
```

### 5. UI Layer (Astro.js + TypeScript + Tailwind CSS + D3.js)
Dark intelligence dashboard aesthetic — think war room, not SaaS tool.

---

# 4. Stitch Design System (Canonical Source of Truth)

**Stitch Project:** "Gothamic Intelligence Dashboard" (`projects/14635260504603434813`)

> This is the canonical design source. All UI decisions should reference this.

## 4a. Design Markdown Spec

```markdown
---
name: Gothamic Intelligence
colors:
  surface: '#131314'
  surface-dim: '#131314'
  surface-bright: '#3a393a'
  surface-container-lowest: '#0e0e0f'
  surface-container-low: '#1c1b1c'
  surface-container: '#201f20'
  surface-container-high: '#2a2a2b'
  surface-container-highest: '#353436'
  on-surface: '#e5e2e3'
  on-surface-variant: '#bacac4'
  inverse-surface: '#e5e2e3'
  inverse-on-surface: '#313031'
  outline: '#84948f'
  outline-variant: '#3b4a45'
  surface-tint: '#27dfbe'
  primary: '#46f1cf'
  on-primary: '#00382e'
  primary-container: '#00d4b4'
  on-primary-container: '#005648'
  inverse-primary: '#006b5a'
  secondary: '#c8c6c8'
  on-secondary: '#303032'
  secondary-container: '#474649'
  on-secondary-container: '#b7b4b7'
  tertiary: '#ffcfa7'
  on-tertiary: '#4c2700'
  tertiary-container: '#ffa957'
  on-tertiary-container: '#733e00'
  error: '#ffb4ab'
  on-error: '#690005'
  error-container: '#93000a'
  on-error-container: '#ffdad6'
  primary-fixed: '#55fcda'
  primary-fixed-dim: '#27dfbe'
  on-primary-fixed: '#00201a'
  on-primary-fixed-variant: '#005143'
  secondary-fixed: '#e4e2e4'
  secondary-fixed-dim: '#c8c6c8'
  on-secondary-fixed: '#1b1b1d'
  on-secondary-fixed-variant: '#474649'
  tertiary-fixed: '#ffdcc1'
  tertiary-fixed-dim: '#ffb877'
  on-tertiary-fixed: '#2e1500'
  on-tertiary-fixed-variant: '#6c3a00'
  background: '#131314'
  on-background: '#e5e2e3'
  surface-variant: '#353436'
  border-muted: '#2d2d30'
  text-dim: '#a1a1a3'
  data-critical: '#ff453a'
  data-warning: '#ffd60a'
  panel-bg: '#111113'
typography:
  display-lg:
    fontFamily: Space Grotesk
    fontSize: 48px
    fontWeight: '700'
    lineHeight: 56px
    letterSpacing: -0.02em
  headline-lg:
    fontFamily: Space Grotesk
    fontSize: 32px
    fontWeight: '600'
    lineHeight: 40px
  headline-md:
    fontFamily: Space Grotesk
    fontSize: 24px
    fontWeight: '600'
    lineHeight: 32px
  headline-sm:
    fontFamily: Space Grotesk
    fontSize: 18px
    fontWeight: '500'
    lineHeight: 24px
  body-lg:
    fontFamily: JetBrains Mono
    fontSize: 16px
    fontWeight: '400'
    lineHeight: 24px
  body-md:
    fontFamily: JetBrains Mono
    fontSize: 14px
    fontWeight: '400'
    lineHeight: 20px
  label-md:
    fontFamily: JetBrains Mono
    fontSize: 12px
    fontWeight: '500'
    lineHeight: 16px
    letterSpacing: 0.05em
  label-sm:
    fontFamily: JetBrains Mono
    fontSize: 10px
    fontWeight: '500'
    lineHeight: 14px
  headline-lg-mobile:
    fontFamily: Space Grotesk
    fontSize: 24px
    fontWeight: '600'
    lineHeight: 30px
spacing:
  unit: 4px
  gutter: 12px
  margin-page: 24px
  panel-padding: 16px
  stack-xs: 4px
  stack-sm: 8px
  stack-md: 16px
---
```

## 4b. Brand & Style Guidelines

**Personality:** Authoritative, precise, uncompromising. Mission-critical intelligence analysis. High-density data visualization with a "military-grade" technical aesthetic.

**Style:** Hybrid of **Minimalism** and **Technical Brutalism**. Panel-based architecture with sharp geometry, high-contrast monochrome surfaces, surgical use of a single accent color. Should feel like a high-end command-and-control interface.

**Background textures:** Low-opacity grid overlays (1px lines every 20px) or faint horizontal scanlines within data visualization containers.

## 4c. Color System

The palette is anchored by a deep monochrome foundation for low-light environments:

| Category | Color | Hex | Usage |
|---|---|---|---|
| **Canvas** | Near-black | `#0a0a0b` | Main page background (darkest layer) |
| **Surface** | Dark gray | `#131314` | Panel backgrounds, navbar, sidebar |
| **Panel BG** | Darker | `#111113` | Inner panel backgrounds |
| **Container Low** | Dark gray | `#1c1b1c` | Hover states, elevated surfaces |
| **Container** | Medium | `#201f20` | Elevated containers |
| **Container High** | Medium gray | `#2a2a2b` | Higher elevation surfaces |
| **Container Highest** | Light gray | `#353436` | Surface variant |
| **Border** | Muted | `#2d2d30` | Panel borders, separators |
| **Outline** | Dim | `#84948f` | Secondary text, outlines |
| **Outline Variant** | Dark teal | `#3b4a45` | Tertiary text, muted outlines |
| **Primary** | Bright teal | `#46f1cf` | Highlighted values, active text |
| **Primary Container** | Teal | `#00d4b4` | Accent color, buttons, active borders |
| **On Surface** | Off-white | `#e5e2e3` | Primary text color |
| **On Surface Variant** | Light teal | `#bacac4` | Secondary text |
| **Text Dim** | Gray | `#a1a1a3` | Muted text, labels |
| **Data Critical** | Red | `#ff453a` | Errors, critical alerts, rejection links |
| **Data Warning** | Yellow | `#ffd60a` | Warnings, conflicts |
| **Error** | Light red | `#ffb4ab` | Error text |
| **Error Container** | Dark red | `#93000a` | Error backgrounds |

**Key principle:** Primary teal (`#00d4b4`) reserved strictly for interactive states, progress indicators, and primary action buttons. Red used sparingly for critical alerts only.

## 4d. Typography System

| Token | Font | Size | Weight | Line Ht | Letter-Spacing | Usage |
|---|---|---|---|---|---|---|
| display-lg | Space Grotesk | 48px | 700 | 56px | -0.02em | Hero / large display |
| headline-lg | Space Grotesk | 32px | 600 | 40px | — | Section headers |
| headline-md | Space Grotesk | 24px | 600 | 32px | — | Panel titles |
| headline-sm | Space Grotesk | 18px | 500 | 24px | — | Sub-headers |
| body-lg | JetBrains Mono | 16px | 400 | 24px | — | Large body/data |
| body-md | JetBrains Mono | 14px | 400 | 20px | — | Default body/data |
| label-md | JetBrains Mono | 12px | 500 | 16px | 0.05em | Labels, metadata |
| label-sm | JetBrains Mono | 10px | 500 | 14px | — | Small labels, timestamps |

- **Space Grotesk:** Narrative/Structural elements — headings, UI landmarks
- **JetBrains Mono:** Data/Functional elements — body copy, tables, code, labels
- Formatting: Uppercase for labels and small headings (military document headers)

## 4e. Layout & Spacing

- **Grid:** 12-column grid with narrow gutters (12px)
- **Density:** High information density. Vertical spacing uses stack increments: `stack-xs` (4px), `stack-sm` (8px), `stack-md` (16px)
- **Panel padding:** 16px
- **Margin page:** 24px
- **Sidebar:** Fixed at 240px or 320px on desktop
- **Mobile:** 12-column grid collapses to single-column vertical stack

## 4f. Elevation & Depth

Depth through **Tonal Layering** and **Crisp Outlines** — no shadows:
- Base layer: `#0a0a0b` (canvas)
- Panels/cards: `#131314` / `#1a1a1c`
- Every panel must have 1px solid border using `#2d2d30` (border-muted)
- Active panels: border swaps to teal (`#00d4b4`) with faint glow: `0px 0px 4px rgba(0, 212, 180, 0.3)`
- Overlays/modals: darker background than panels below, with 1px border

## 4g. Shapes

**Strictly Sharp (0px roundedness).** Every UI element — buttons, input fields, panels, dropdowns — must have 90-degree corners. No pill shapes. Exception: small square status indicators.

## 4h. Component Specs

| Component | Style |
|---|---|
| **Primary Button** | Solid teal (`#00d4b4`) bg, black (`#00382e`) text, sharp edges |
| **Secondary Button** | 1px teal border, teal text, no fill, sharp edges |
| **Input Field** | Dark fill (`#111113`), 1px `#2d2d30` border, teal on focus, label above in `label-sm` JetBrains Mono |
| **Chips/Tags** | Rectangular, 1px border, 10% opacity background |
| **Data Tables** | No vertical column lines, subtle horizontal dividers, header rows lighter bg, JetBrains Mono for all cells |
| **Cards/Panels** | Header with Space Grotesk title + optional metadata (e.g., "REF_ID: 882-X") in top-right |
| **Scrollbars** | Custom thin, `#2d2d30` color |
| **Terminal Component** | Pure black bg, teal text |

---

# 5. Stitch Screen Mockups

The following screens exist in the Stitch project as designed mockups (screenshots + generated HTML):

## Analysis Dashboard Screens
| Title | Notes |
|---|---|
| GLYPH: Analysis Dashboard (Charcoal Protocol) × 2 | Two versions at different refinement stages |

## Sidebar Screens
| Title | Notes |
|---|---|
| GLYPH: Analysis Dashboard | Alternative layout with decision timeline + topology viz |
| GLYPH: Analysis Dashboard (Crimson Protocol) | Red-accent variant |
| GLYPH: Analysis Dashboard (Charcoal Protocol) × 2 | Teal-accent variant (canonical) |

## Terminal / Entry Screens
| Title | Notes |
|---|---|
| GLYPH: Terminal Entry (Charcoal Protocol) × 2 | Terminal input/output design |
| GLYPH: Terminal Entry (Glitch Nodes) | Glitch effect terminal |
| GLYPH: Terminal Entry (Crimson Protocol) | Red-accent terminal |
| GLYPH: Terminal Entry (Animated Pipeline) | Animated terminal pipeline |
| GLYPH: Terminal Entry (Updated Pipeline) | Updated terminal pipeline |
| GLYPH: Intelligence Terminal Entry | Intelligence-focused terminal |
| GLYPH: Intelligence Terminal Entry (War Room) × 2 | War room terminal design |
| GLYPH: Vibe Coding Terminal Entry | Alternative terminal style |
| GLYPH: Terminal Entry (Enhanced Glitch & Fixed Flow) | Enhanced glitch fix |

## Debate Explorer Screens
| Title | Notes |
|---|---|
| GLYPH: Debate Explorer | Main debate explorer |
| GLYPH: Debate Explorer (Clean & Translucent) | Clean variant |
| GLYPH: Debate Explorer (Charcoal Protocol) × 2 | Teal variant |
| GLYPH: Animated Debate Explorer (Charcoal Protocol) | Animated version |
| GLYPH: Animated Debate Explorer (Fixed Growth Animation) | Fixed animation |

## Other Screens
| Title | Notes |
|---|---|
| GLYPH: Rejection Vault (Charcoal Protocol) × 2 | Rejection graveyard (teal) |
| GLYPH: Rejection Vault (Crimson Protocol) | Rejection graveyard (red) |
| GLYPH: Rejection Vault | Base version |
| GLYPH: Decision Timeline (Charcoal Protocol) | Timeline (teal) |
| GLYPH: Repository Intelligence Summary (Charcoal Protocol) | Repo summary (teal) |
| GLYPH: Contributor Intelligence (Charcoal Protocol) | Contributors (teal) |
| GLYPH: Contributor Intelligence (Terminal Layout) | Contributors (terminal) |
| GLYPH: Intelligence Terminal Entry | Terminal entry |

---

# 6. Current UI Implementation

## 6a. File Structure

```
frontend/
├── astro.config.mjs          # Astro + Tailwind + Vercel adapter
├── tailwind.config.cjs        # Custom palette, fonts, zero radii
├── package.json
├── src/
│   ├── pages/
│   │   ├── index.astro        # Landing page (~1140 lines, single-file)
│   │   └── repo/
│   │       ├── [id].astro     # Analysis dashboard (~1250 lines, single-file)
│   │       └── [id]/
│   │           ├── summary.astro
│   │           ├── decisions.astro
│   │           ├── debates.astro
│   │           ├── rejections.astro
│   │           └── contributors.astro
│   ├── components/
│   │   ├── RepoInput.astro
│   │   ├── DecisionGraph.astro
│   │   ├── Timeline.astro
│   │   ├── DebateExplorer.astro
│   │   └── ContributorCard.astro
│   └── styles/
│       └── global.css         # Tailwind directives + global resets
```

## 6b. Current Dashboard Layout (`[id].astro`)

The dashboard is a single-file component with CSS + HTML + JS all inline:

**Structure:**
```
navbar (fixed, 48px)
  └── GLYPH brand (left) | Search bar | Action icons (right)

layout (below navbar)
  ├── sidebar (172px)
  │   ├── nav items: RECON, VCS_LOG, DECRYPT, INDEX, NETWORK
  │   └── footer: EXECUTE_FETCH, SYS_LOG, TERMINATE
  └── main-area
      ├── status bar: system state | thread context | uptime | heartbeat
      ├── content-grid
      │   ├── graph-panel (left, 1fr) — MAP_OVERLAY_01 // CONFLICT_CLUSTERS
      │   │   ├── SVG container for D3 force graph
      │   │   ├── Static conflict callout overlays
      │   │   └── Pagination dots
      │   ├── intel-panel (right, 260px) — PROBABILITY_MATRIX_X + NEURAL_FORECAST
      │   └── stream-panel (bottom, spans both) — LIVE_RECON_STREAM
      ├── extra panels: disk utilization | encryption | operators
      └── bottom bar: system status | sec layer | location | timestamp
```

**Current color scheme** (after recent fixes): Uses red (`#cc0000`) as primary accent throughout — navbar active states, button fills, graph edges, callout borders, metric bars, critical text.

**Typography:** JetBrains Mono for UI, body, data. Space Grotesk for headings. Karla for forecast body text. All 0px border-radius.

**Key UI elements:**
- Navbar: grid layout with brand | search | actions
- Sidebar: 172px fixed, nav items with SVG icons + text
- Graph panel: empty SVG container for D3, overlay with conflict callout boxes
- Intel panel: 3 metric bars with labels + percent, neural forecast text + override button
- Stream panel: scrolling log with trace/critical lines, auto-appended JS
- Extra panels: disk utilization bar, encryption status, operator chips
- Bottom status bar: SYSTEM, SEC_LAYER, LOC, live timestamp

## 6c. JS Functionality

All client-side JavaScript is inline `<script>` blocks:

1. **Uptime counter** — `requestAnimationFrame` tick from page load, updates navbar and status bar
2. **Heartbeat toggle** — flips between HEARTBEAT_STABLE / HEARTBEAT_OK every 2s
3. **Timestamp** — shows live datetime in bottom bar, updates every 1s
4. **Sidebar nav** — toggles active class + shows/hides `data-section-content` panels
5. **Execute fetch** — POST to `/api/analyze`, polls status every 4s, updates button text
6. **Override button** — POST to `/api/repo/:id/override`
7. **Live stream** — appends a random trace/critical line every 1.8s, caps at 40 lines
8. **D3 graph** (ES module) — force-directed graph with nodes + links, drag support, fallback data

---

# 7. BYOK Credential Flow

GLYPH does **not** store or provide any API keys. Users supply their own credentials on first use.

**What the user provides:**
| Credential | Source | Purpose |
|---|---|---|
| GitHub PAT | github.com → Settings → Developer Settings | Fetching commits, PRs, issues |
| NVIDIA NIM API Key | build.nvidia.com → Get API Key | Running AI intelligence layer |

**Flow:**
1. User enters keys in landing page "Setup Keys" modal
2. Keys stored in `sessionStorage` (cleared on tab close)
3. Every API call passes keys as request headers: `X-Github-Token` + `X-Nim-Api-Key`
4. Backend receives keys per-request, never writes to DB
5. Keys used for GitHub API + NIM calls for that session only
6. Keys discarded after request completes

**Security principles:**
- Never persisted — in-memory per request only, never written to PostgreSQL
- Never logged — explicitly excluded from any request logging
- User-owned rate limits — GitHub's 5000 req/hr and NIM quotas apply to user's own account

---

# 8. Database Schema

```sql
repos (
  id UUID PRIMARY KEY,
  github_url TEXT NOT NULL,
  owner TEXT,
  name TEXT,
  analyzed_at TIMESTAMP,
  status TEXT  -- pending | processing | complete | failed
)

commits (
  id UUID PRIMARY KEY,
  repo_id UUID REFERENCES repos(id),
  sha TEXT,
  message TEXT,
  author TEXT,
  timestamp TIMESTAMP,
  files_changed JSONB
)

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

# 9. Project Progress

## Phase Completion

| Phase | Status | Notes |
|---|---|---|
| **1: Rust + GitHub Ingestion** | 95% | Async GitHub client, rate-limit aware. Missing: full pagination, error recovery |
| **2: PostgreSQL + SQLx** | 90% | All tables + migrations + indexes. Missing: query optimization |
| **3: NVIDIA NIM Pipeline** | 70% | Client + prompts + JSON parsing. Missing: batching, confidence scoring, fallbacks |
| **4: Axum REST API** | 85% | All 8 endpoints, BYOK headers, CORS. Missing: rate limiting, validation, docs |
| **5: Astro.js UI** | 80% | Landing + dashboard + stream. Missing: API integration, loading states, error handling |
| **6: D3.js Graph** | 40% | Force layout + fallback data. Missing: zoom/pan, timeline, debate explorer, contributor cards |
| **7: Polish + Deploy** | Done | Vercel + Render + PostgreSQL deployed |

## What's Working
- Backend server with PostgreSQL
- GitHub ingestion (commits, PRs, issues)
- NIM integration (basic)
- Frontend landing page with BYOK flow + scroll visualization
- Dashboard layout with navigation, stream, graph shell
- Live Vercel + Render deployment

## What's Partially Done
- D3.js visualization (placeholder structure, force layout scaffold)
- Live data updates (stream panel disconnected from real API)
- Error recovery

## What's Missing
- Multi-repo comparison
- Export functionality (JSON, PDF)
- Advanced filtering/search
- User preferences/saved analyses
- API key management UI
- Testing suite (unit + integration)
- Documentation

---

# 10. Known Issues

1. **Frontend API Integration** — Landing page form doesn't actually trigger analysis end-to-end
2. **Graph Visualization** — D3.js has force layout shell but minimal real interactivity
3. **Error Recovery** — Network failures may leave jobs stuck in "processing"
4. **Memory Management** — Large repos could exhaust memory
5. **Security** — Input validation and sanitization needed
6. **Performance** — No caching of API responses
7. **Testing** — No automated tests
8. **CSS bug history:** The global whitelist uses `animation-duration: unset !important` which resolves to `0s` (initial value for non-inherited properties). Hero glitch + data-packet animations needed explicit `!important` overrides after the whitelist to work. Documented in `BUGHUNT.md`.

---

# 11. Deployment

```
User → Vercel (Astro frontend)
          ↓ API calls
     Render (Axum backend via Docker)
          ↓ queries
     Render PostgreSQL
          ↓ AI calls
     NVIDIA NIM API
          ↓ data fetch
     GitHub REST API
```

**URLs:**
- Frontend: `https://glyph-pua2jbu7s-glyph-tango.vercel.app`
- Backend API: `https://glyph-api-u495.onrender.com`

**Environment Variables:**
```env
# Backend
DATABASE_URL=postgresql://...  # Render PostgreSQL
NIM_BASE_URL=https://integrate.api.nvidia.com/v1
FRONTEND_URL=https://glyph-pua2jbu7s-glyph-tango.vercel.app

# Frontend
PUBLIC_API_BASE_URL=https://glyph-api-u495.onrender.com
```

**Stitch MCP Config** (local `opencode.json`):
```json
{
  "mcp": {
    "stitch": {
      "type": "remote",
      "url": "https://stitch.googleapis.com/mcp",
      "enabled": true,
      "headers": {
        "X-Goog-Api-Key": "${STITCH_API_KEY}"
      }
    }
  }
}
```

---

*End of document. Generated from GLYPH.md, PROGRESS_CHECKLIST.md, SHOWCASE.md, BUGHUNT.md, Stitch design system output, and frontend code analysis.*
