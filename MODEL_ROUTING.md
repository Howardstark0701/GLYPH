# Model routing — free tier via OmniRoute

Switch manually with `/model <id>` (one command per message — Claude Code
does not auto-switch models based on prompt content).

## Quick picks by task

| Task type | Model | Why |
|---|---|---|
| Default / daily driver | `oc/big-pickle` | 200K ctx, tool calling + thinking, built for coding agents |
| Big refactor / many files at once | `oc/deepseek-v4-flash-free` | 1M context, won't truncate large repos |
| Architecture / design decisions | `aug/glm-5.2` | 1M ctx, thinking with effort tiers, good at tradeoffs |
| Quick iterative edits | `aug/kimi-k2.7` | 131K ctx, fast, coding-tuned |
| big-pickle rate-limited | `auto/coding:free` | OmniRoute auto-picks a free fallback |
| Need real Claude reasoning (rare, hard bugs) | `tllm/CLAUDE_4_6_SONNET` | Free proxy, flaky but genuinely Claude |

## Per-project defaults

**GLYPH** (Rust/Axum, Astro, SQLx, D3)
- Default: `oc/big-pickle`
- Multi-file sub-page work: `oc/deepseek-v4-flash-free`

**PRETO** (FastAPI, React, Streamlit)
- Default: `oc/big-pickle`
- Phase 8 ingestion pivot / architecture calls: `aug/glm-5.2`

**Sherza** (Next.js, FastAPI, Supabase)
- Default: `oc/big-pickle`
- Legal/privacy policy review, subtle bugs: bring to Claude directly (not a free-tier task)

## Switching mid-session

```
/model oc/big-pickle
```
Then wait for confirmation before sending your next prompt. Don't stack
multiple `/model` lines in one message — it'll error.

## When to stop using free models

Debugging sessions like the Starlette middleware 500 bug or Sherza's
trial-race-condition — free models tend to loop on the wrong fix. Worth
burning real Claude API credit or bringing it to a chat session instead
of grinding a free model for hours.
