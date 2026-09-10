# GLYPH — Demo Runbook

Everything needed to bring GLYPH up and show it working, plus the failure
modes that have actually bitten this project and what to do about each.

---

## 1. What you need before you start

| Thing | Where to get it | Notes |
|---|---|---|
| **GitHub PAT** | github.com → Settings → Developer settings → Personal access tokens | `public_repo` scope is enough. An unauthenticated request is capped at 60/hour, which is not enough to ingest a repo. |
| **NVIDIA NIM API key** | build.nvidia.com → your profile → API keys | Starts `nvapi-`. |
| **Docker Desktop** | Already installed on this machine | Must be *running* — the tray icon, not just installed. |

Both keys are entered in the browser (BYOK). They are held in `sessionStorage`
and sent as per-request headers; the server never stores or logs them.

---

## 2. Start the stack (laptop — the primary demo path)

```bash
# Terminal 1 — database + backend
cd ~/GLYPH
docker compose up --build          # http://localhost:8000

# Terminal 2 — frontend
cd ~/GLYPH/frontend
npm run dev                        # http://localhost:4321
```

Confirm the backend before you touch the UI:

```bash
curl http://localhost:8000/health   # -> OK
```

The landing page's status bar shows `API: ONLINE` and a real round-trip time.
If it reads `API: UNREACHABLE`, the frontend is fine and the backend is not —
check Terminal 1.

### Running the backend without Docker

Useful when Docker Desktop is slow to start:

```bash
docker start glyph-postgres         # or `docker compose up db`
cd ~/GLYPH/backend
DATABASE_URL=postgres://glyph:glyph@localhost:55432/glyph cargo run --release
```

Migrations run automatically at boot against an empty database.

---

## 3. The demo path

1. Open `http://localhost:4321`.
2. Click **KEYS: NOT_SET — CLICK TO CONFIGURE** (pulsing, under the URL box).
   Paste the GitHub PAT and the NIM key.
3. Paste a repository URL and submit.
4. You land on the dashboard. The header state, the recon stream, and the graph
   are all driven by the real job — nothing on that screen is pre-recorded.
5. Walk the left nav: DECISIONS → DEBATES → REJECTIONS → CONTRIBUTORS → SUMMARY.

### Pick the repo deliberately

Analysis time scales with repository size. **Rehearse with the repo you plan to
show.**

| Repo size | Roughly |
|---|---|
| Small (< 300 commits) | ~1 minute |
| Medium (ripgrep-sized) | ~3 minutes |
| Very large | Several minutes; bounded by `MAX_ANALYSIS_CHUNKS` |

Most of that is the LLM extraction pass, not GitHub ingestion. If you need a
guaranteed-fast demo, **run the analysis before you present** and open the
finished job's URL — completed jobs load instantly from the database.

`/repo/demo` is a static, clearly-fictional showcase page. It is not a real
analysis and it does not pretend to be one. Do not present it as output.

---

## 4. Failure modes that have actually happened

### The hosted backend is unreachable

`https://glyph-api-qh9i.onrender.com` stopped answering entirely — TLS
connects, the request goes out, nothing comes back.

**Cause:** Render deletes free PostgreSQL instances after 30 days. Once the
database vanished, the backend could not connect at boot, so it crash-looped
and the health check never passed.

**Fix:** give it a database with an indefinite free tier.

1. Create a free Postgres at [neon.tech](https://neon.tech) (or Supabase).
2. Copy the connection string (`postgresql://…?sslmode=require`).
3. Render → `glyph-api` → Environment → add `DATABASE_URL` with that value.
4. Manual Deploy → Deploy latest commit.
5. Verify: `curl https://glyph-api-qh9i.onrender.com/health` → `OK`.

`backend/render.yaml` now declares `DATABASE_URL` as `sync: false` (a secret
you set in the dashboard) precisely so this cannot silently expire again.

**Also note:** Render free instances sleep after inactivity. The first request
after idle pays a cold start of ~50 seconds. Hit `/health` a few minutes before
demoing so the instance is awake.

### Every analysis returns bare commit titles instead of reasoning

Symptom: decisions all read like raw commit subjects (`ignore-0.4.33`,
`ci: fix binary discovery`) and every confidence is identical.

That is the deterministic fallback, which means the LLM pass failed. Three
distinct causes have produced it:

1. **The model was retired.** NVIDIA retires hosted models on a rolling basis,
   and a retired model still appears in `GET /v1/models` while
   `POST /chat/completions` answers 404. The backend now walks a chain of
   candidate models instead of trusting one hardcoded default.
2. **The HTTP timeout was shorter than the model.** The shared client's 60s
   timeout was below the ~100s a reasoning model spends on one window, so every
   call timed out. NIM calls now use their own budget (`NIM_TIMEOUT_SECS`,
   default 300).
3. **The response could not be parsed.** Reasoning models narrate before
   answering. The parser now strips `<think>` blocks and scans every fenced
   block and every balanced JSON span rather than assuming the reply starts
   with `[`.

To check which one you are hitting, read the backend log — each is logged
distinctly:

```
NIM model '<name>' unavailable, trying next: …    → retired model
NIM request failed: error sending request …       → timeout
Failed to parse NIM JSON response: …              → parse failure
```

### The UI shows `SYNCING_…` for ever and never loads data

The page's client script failed to start. Check the browser devtools **Network**
tab, not just the console — a failed ES module import produces no console error,
so the page just sits there looking like it is loading.

The usual culprit in development is a stale Vite dependency cache: after the d3
import changed, `node_modules/.vite/deps/d3.js` began returning **504**, which
killed the module and therefore all hydration, silently.

```bash
cd frontend
rm -rf node_modules/.vite
npm run dev
```

This affects `npm run dev` only — the production build bundles its dependencies
and is unaffected.

Also avoid running `npm run build` while `npm run dev` is watching: the build
writes into `.vercel/output`, which the dev server then picks up as a file
change, and the two chase each other.

### Rate limits

GitHub allows 5,000 requests/hour on a PAT. Ingesting a large repository with
review threads uses a meaningful slice of that. Check what is left:

```bash
curl -H "Authorization: Bearer $GH_PAT" https://api.github.com/rate_limit
```

---

## 5. Configuration reference

| Variable | Default | Purpose |
|---|---|---|
| `DATABASE_URL` | — (required) | PostgreSQL connection string. Migrations run at boot. |
| `PORT` | `8000` | HTTP listen port. |
| `FRONTEND_URLS` | — | Comma-separated allowed CORS origins. **Unset = allow any origin**, which is safe here because GLYPH holds no cookies or sessions. A wrong value is worse than none — it locks CORS to that list and blocks the real frontend. |
| `NIM_MODEL` | — | Pins one model. Leave unset to use the fallback chain. |
| `NIM_BASE_URL` | `https://integrate.api.nvidia.com/v1` | NIM endpoint. |
| `NIM_TIMEOUT_SECS` | `300` | Per-request budget for LLM calls. |
| `MAX_ANALYSIS_CHUNKS` | `6` | Extraction windows per job — bounds LLM cost and wall-clock time. |
| `MAX_COMMITS` | `1000` | Commit ingestion cap. |
| `PUBLIC_API_BASE_URL` | — | Frontend override for the API base. Unset falls back to localhost in dev and the Render URL in production. |

---

## 6. Pre-demo checklist

- [ ] Docker Desktop running
- [ ] `docker compose up` — backend healthy at `/health`
- [ ] `npm run dev` — frontend at `:4321`
- [ ] Landing page status bar reads **API: ONLINE**
- [ ] Keys configured in the browser (they live in `sessionStorage`, so they do
      **not** survive closing the tab — re-enter after a restart)
- [ ] GitHub rate limit has headroom
- [ ] One analysis already completed, its URL saved as a fallback
- [ ] Rehearsed once end-to-end on the exact repo you will show
