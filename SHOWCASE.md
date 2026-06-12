┌──────────────────────────────────────────────────────────────────┐
│                        GLYPH_OS                                 │
│                   CAPABILITY BRIEF // v0.1.0-ALPHA              │
│              CLASSIFICATION: PUBLIC // DISTRIBUTION: UNLIMITED  │
└──────────────────────────────────────────────────────────────────┘

**Repository:** `https://github.com/Howardstark0701/GLYPH`
**Stack:** Rust, Axum, PostgreSQL, NVIDIA NIM, Astro.js, D3.js
**Deployment:** Shuttle.rs / Vercel / Railway

---

## 1 // What Is GLYPH

GLYPH is an open-source intelligence backend that takes any GitHub
repository URL and reconstructs the **decision history** of that
codebase — what was debated, what was rejected, what architectural
choices were made and why.

It turns thousands of commits, pull requests, issues, and review
threads into a structured, queryable intelligence document powered
by AI.

    Git log tells you what changed. GLYPH tells you *why*.

---

## 2 // State-of-the-Art Differentiation

| Dimension | Existing Tools | GLYPH |
|---|---|---|
| Data extracted | Diffs, stats, contributor counts | Decision nodes, debate threads, rejection records, architectural intent |
| Output type | Dashboards & metrics | Structured queryable intelligence graph |
| AI reasoning | None (or generic LLM wrappers) | Purpose-built NIM pipeline for cognitive archaeology |
| Credential model | Server stores API keys | BYOK — keys never touch the database |
| Language | Python/Node.js | Rust — can process 10k+ commit repos efficiently |

**No existing tool extracts reasoning and intent from git history.**
GitStats, git-of-theseus, and similar tools only surface *what*
changed — never *why* it changed, what was argued, or what was
rejected along the way. GLYPH fills that gap as a dedicated
intelligence infrastructure layer.

---

## 3 // Build Velocity — Vibe-Coded in ~72 Hours

The entire project was built across **3 sessions** (~72 hours total)
by a solo developer using AI-assisted vibe coding:

| Session | Deliverables |
|---|---|
| Session 1 | Full Rust backend scaffold (ingestion, processing, intelligence modules, all 9 API endpoints, SQLx models + migrations, unified error types) |
| Session 2 | Astro.js frontend (dark dashboard, D3.js graph shell, BYOK modal, repo input, syslog terminal) |
| Session 3 | Landing page polish (glitch animations, BYOK trace flow, data-packet animation, bug fixing + deployment) |

The architecture was evolved through rapid iteration with AI agents
— opencode managed boilerplate and file generation while kiro drove
architectural decisions and complex logic.

---

## 4 // Bug Resolution — Human + AI Pair Debugging

A single CSS bug consumed **24+ hours** of sustained investigation:

**The problem:** Two animations on the landing page (GLYPH logo glitch
and data-packet flow) were correctly wired — keyframes defined,
animation shorthand set, HTML structure correct — yet both were dead
in the live build. DevTools showed `animation-duration: 0s`.

**The root cause:** The project had a global CSS animation whitelist
that used `animation-duration: unset !important` to "exempt" elements
from animation suppression. But `unset` on the non-inherited
`animation-duration` property resolves to `initial` — which is `0s`.
The whitelist was itself the killer.

**The fix:** Explicit `animation-duration: 2s !important` /
`4s !important` overrides placed *after* the whitelist rule in the
cascade. Same specificity, both `!important` — later declaration wins.

This was debugged collaboratively: AI agents traced the CSS cascade
layer by layer while the developer controlled hypothesis, tested fixes,
and verified the built output. Full war story is documented in
`BUGHUNT.md` in the repository.

---

## 5 // Precursor — PRETO

GLYPH evolved from an earlier project called **PRETO** — an open-source
PR review intelligence tool that analyzed GitHub pull requests via LLM
to surface review quality, sentiment, and hidden issues.

PRETO proved the core thesis: **code review threads contain invaluable
decision context that is lost after the PR is merged.** GLYPH extends
this to the entire repository — not just PRs, but commits, issues,
review threads, and their interconnections.

Both projects share the same architectural DNA: Rust + Axum backend,
BYOK auth model, and a conviction that developer tools should surface
*intelligence*, not just metrics.

---

## 6 // System Architecture

```
User → Vercel (Astro.js frontend)
          ↓ API calls (X-Github-Token / X-Nim-Api-Key headers)
     Shuttle.rs (Axum backend)
          ↓ queries
     Railway (PostgreSQL)
          ↓ AI calls
     NVIDIA NIM API
          ↓ data fetch
     GitHub REST API
```

**The BYOK flow:** User enters their own GitHub PAT + NVIDIA NIM API
key in the frontend. Keys travel as per-request HTTP headers, are
never written to the database, never logged, and are discarded when
the request completes. Zero secrets on the server.

---

## 7 // Current Progress — Phase 1 Complete

- [x] Rust project scaffold with Axum + Shuttle.rs
- [x] GitHub ingestion pipeline (commits, PRs, issues)
- [x] BYOK credential model (frontend modal + backend header extraction)
- [x] All 9 REST API endpoints (analyze, status, intent, debates,
      decisions, rejections, contributors, graph, summary)
- [x] PostgreSQL schema with full migrations
- [x] Astro.js dark dashboard with D3.js graph shell
- [x] Landing page with glitch animations, BYOK trace, syslog terminal
- [x] Deployment: render.com (backend) + Vercel (frontend)

**Next phases:**
- Phase 3: NVIDIA NIM intelligence pipeline (prompt engineering +
  structured extraction from event chains)
- Phase 4: Interactive D3.js decision graph + timeline
- Phase 5: Multi-repo analysis + cross-repo comparison
- Phase 6: Production polish, docs, and launch

---

## 8 // Closing

> "GLYPH is an open-source intelligence backend built in Rust that
> uses AI to reconstruct the decision history of any GitHub repository
> — surfacing debates, rejections, and architectural reasoning that
> git history obscures."

Built by **Howardstark0701** — solo developer, vibe-coded in ~72
hours with AI-assisted pair programming, 24+ hours of human + AI
collaborative debugging, and a conviction that the next generation
of developer tools should be intelligence infrastructure.

**Repository:** `https://github.com/Howardstark0701/GLYPH`

---

*GLYPH_OS // SHOWCASE_BRIEF // 2026-06-11
END OF TRANSMISSION*
