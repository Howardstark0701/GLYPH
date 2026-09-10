// ─────────────────────────────────────────────────────────────
// GLYPH OS — shared client helpers for /repo/:id/* views
//
// These views are server-rendered with static seed data (a polished
// demo for slug-based URLs), then hydrated from the real API when the
// job id is a UUID. This module owns the honest parts of that:
//   · uuid/slug job detection
//   · real status polling for the live feeds (no fabricated events)
//   · honest empty + error states for real jobs
// ─────────────────────────────────────────────────────────────

export interface RepoStatus {
  status:        string;
  stage:         string | null;
  error_message: string | null;
  analyzed_at:   string | null;
  /** Which repository this job analysed. The URL only carries the job id. */
  owner?:        string | null;
  name?:         string | null;
  github_url?:   string | null;
}

const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

/**
 * Resolve the backend API base for the running environment:
 *   1. PUBLIC_API_BASE_URL env override (set it in Vercel if you want to pin it)
 *   2. window.__GLYPH_API__ (injected at runtime if ever needed)
 *   3. localhost:8000 when running on a local dev host
 *   4. the live Render backend as the production default
 * Without this, the deployed site fell back to http://localhost:8000 and every
 * API call hit the user's own machine — graphs/SVG never hydrated.
 */
export function apiBase(): string {
  const env = (import.meta as any).env?.PUBLIC_API_BASE_URL;
  if (env) return env;
  if (typeof window !== 'undefined' && (window as any).__GLYPH_API__) {
    return (window as any).__GLYPH_API__;
  }
  if (typeof window !== 'undefined') {
    const host = (window as any).location?.hostname ?? '';
    if (host === 'localhost' || host === '127.0.0.1') return 'http://localhost:8000';
  }
  return 'https://glyph-api-qh9i.onrender.com';
}

/** Job id from the URL path (/repo/:id/...), or null for slug/demo URLs. */
export function getJobId(): string | null {
  const id = window.location.pathname.split('/').filter(Boolean)[1] ?? '';
  return UUID_RE.test(id) ? id : null;
}

export function nowTs(): string {
  const d = new Date();
  const p = (n: number) => String(n).padStart(2, '0');
  return `[${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}]`;
}

/**
 * Why this is a tagged result rather than `RepoStatus | null`:
 *
 * Callers poll this on an interval, and the two ways it can fail need
 * opposite handling. A 404 means the job id does not exist — retrying can
 * never succeed, so the poller must stop and say so. A network error or a
 * 5xx may well clear on the next tick, so the poller should retry, but only
 * a bounded number of times. Collapsing both into `null` produced a loop
 * that hammered a dead endpoint every 3s for the life of the tab while
 * showing the viewer nothing at all.
 */
export type StatusResult =
  | { kind: 'ok'; status: RepoStatus }
  | { kind: 'gone' }
  | { kind: 'unreachable' };

export async function fetchStatusResult(jobId: string): Promise<StatusResult> {
  try {
    const r = await apiFetch(`${apiBase()}/api/repo/${jobId}/status`);
    if (r.status === 404) return { kind: 'gone' };
    if (!r.ok) return { kind: 'unreachable' };
    return { kind: 'ok', status: (await r.json()) as RepoStatus };
  } catch {
    return { kind: 'unreachable' };
  }
}

export async function fetchStatus(jobId: string): Promise<RepoStatus | null> {
  const r = await fetchStatusResult(jobId);
  return r.kind === 'ok' ? r.status : null;
}

const STAGE_LABEL: Record<string, string> = {
  queued:                'QUEUED',
  ingesting_commits:     'INGESTING_COMMITS',
  ingesting_pull_requests: 'INGESTING_PULL_REQUESTS',
  ingesting_issues:      'INGESTING_ISSUES',
  ingesting_review_threads: 'INGESTING_REVIEW_THREADS',
  extracting_intent:     'EXTRACTING_INTENT',
  idle:                  'ANALYSIS_IDLE',
};

export function stageLabel(stage: string | null): string {
  if (!stage) return 'PENDING';
  return STAGE_LABEL[stage] ?? stage.toUpperCase().replace(/_/g, ' ');
}

/** Honest status line HTML for a live feed. Override via render(). */
export function statusLineHtml(s: RepoStatus): string {
  const ts = nowTs();
  if (s.status === 'failed' || s.status === 'terminated') {
    const tag = s.status === 'terminated' ? 'TERMINATED' : 'ANALYSIS_FAILED';
    const msg = s.error_message ?? '';
    return `<span style="color:#5e3f3a;">${ts}</span> <span style="color:#cc0000;font-weight:700;">[STATE]</span> <span style="color:#cc0000;font-weight:700;">${tag}</span>${msg ? ` <span style="color:#5e3f3a;">— ${msg}</span>` : ''}`;
  }
  if (s.status === 'idle') {
    return `<span style="color:#5e3f3a;">${ts}</span> <span style="color:#22d3ee;font-weight:700;">[STATE]</span> <span style="color:#22d3ee;font-weight:700;">ANALYSIS_IDLE</span> <span style="color:#5e3f3a;">— awaiting inspection</span>`;
  }
  return `<span style="color:#5e3f3a;">${ts}</span> <span style="color:#ffb4a8;font-weight:700;">[STATUS]</span> <span style="color:#e5e2e1;">${stageLabel(s.stage)}</span>`;
}

/**
 * Fill in which repository this job actually analysed.
 *
 * The URL of a real job carries only its UUID, so the server-rendered pages
 * fall back to "unknown/unknown" in their titles and headers — a dashboard
 * that cannot name the repository it just analysed reads as broken. GET
 * /status now returns owner/name, so one call fixes every such spot.
 *
 * Rewrites the document title and the text of any [data-repo-slug] and
 * [data-repo-context] element. No-op in demo (slug) mode, where the URL
 * already names the repository.
 */
export async function hydrateRepoIdentity(): Promise<void> {
  const jobId = getJobId();
  if (!jobId) return;

  const res = await fetchStatusResult(jobId);
  if (res.kind !== 'ok') return;

  const owner = res.status.owner;
  const name  = res.status.name;
  if (!owner || !name) return;

  const slug = `${owner}/${name}`;

  // Replace exactly what the server rendered. The placeholder is not a fixed
  // string — it is "unknown/<job-uuid>" — so each page publishes the slug it
  // rendered in <meta name="glyph-repo-slug">, and that is what gets swapped.
  const seeded = document
    .querySelector('meta[name="glyph-repo-slug"]')
    ?.getAttribute('content');
  if (seeded && document.title.includes(seeded)) {
    document.title = document.title.replace(seeded, slug);
  }

  document.querySelectorAll('[data-repo-slug]').forEach((el) => {
    el.textContent = slug;
  });
  document.querySelectorAll('[data-repo-context]').forEach((el) => {
    el.textContent = `THREAD CONTEXT: ${owner}_${name}_ANALYSIS`.toUpperCase();
  });
}

/**
 * Normalise a source reference for display.
 *
 * Models emit these loosely — "a1b2c3d", "commit: a1b2c3d", "PR #1822",
 * "issue #17", "pull request #3655". Taking a fixed-length prefix produced
 * labels like "COMMIT: commit:" and "0xPR #98", so pull out the identifier
 * that is actually in there.
 */
export function shortRef(ref: unknown): string {
  if (!ref) return 'N/A';
  const text = String(ref).trim();
  const sha = text.match(/[0-9a-f]{7,40}/i);
  if (sha) return sha[0].slice(0, 7).toLowerCase();
  const num = text.match(/#?(\d+)/);
  if (num) return '#' + num[1];
  return text.length > 12 ? text.slice(0, 12) + '\u2026' : text;
}

/** How long any single API call may take before it counts as unreachable. */
export const API_TIMEOUT_MS = 10000;

/**
 * fetch() with a deadline.
 *
 * A backend that is *down* is not the same as one that *refuses*. Render's
 * free tier, once its database expired, stopped answering entirely: the
 * request neither resolved nor rejected, it simply hung. Bare fetch() has no
 * default timeout, so every poller sat waiting for a reply that was never
 * coming — the retry counters never advanced, no error was ever surfaced, and
 * the page showed "SYNCING…" for as long as it stayed open. That is the exact
 * silent degradation this project treats as its worst failure mode.
 */
export function apiFetch(url: string, init: RequestInit = {}): Promise<Response> {
  return fetch(url, { ...init, signal: AbortSignal.timeout(API_TIMEOUT_MS) });
}

/** Consecutive unreachable ticks tolerated before the feed gives up. */
const MAX_STATUS_FAILURES = 5;

/**
 * Poll the real status endpoint and append honest lines to a feed
 * container. Stops when the analysis reaches a terminal state, when the job
 * turns out not to exist, or after MAX_STATUS_FAILURES consecutive failures
 * to reach the backend — and says which of those happened rather than
 * leaving an empty stream behind.
 * Returns a cancel function. No-op in demo (slug) mode.
 */
export function startStatusFeed(
  container: HTMLElement | null,
  render: (s: RepoStatus) => string = statusLineHtml,
  intervalMs = 3000,
  maxLines = 40,
): () => void {
  const jobId = getJobId();
  if (!jobId || !container) return () => {};

  let lastSig = '';
  let terminalSeen = false;
  let timer: number | undefined;

  const append = (html: string) => {
    const line = document.createElement('div');
    // Host pages style their own feed lines; carry the terminal type across
    // so a status line never renders in the page's proportional body font.
    line.className = 'stream-line glyph-feed-line';
    line.style.cssText =
      'font-family:JetBrains Mono,monospace;font-size:0.6rem;line-height:1.6;' +
      'letter-spacing:0.04em;overflow-wrap:anywhere;word-break:break-word;';
    line.innerHTML = html;
    container.appendChild(line);
    container.scrollTop = container.scrollHeight;
    while (container.children.length > maxLines) {
      container.removeChild(container.firstChild!);
    }
  };

  const stop = () => {
    if (timer) window.clearInterval(timer);
    timer = undefined;
  };

  const appendFailure = (msg: string) => {
    append(
      `<span style="color:#5e3f3a;">${nowTs()}</span> ` +
      `<span style="color:#cc0000;font-weight:700;">[ERROR]</span> ` +
      `<span style="color:#cc0000;">${msg}</span>`,
    );
  };

  let failures = 0;

  const poll = async () => {
    const res = await fetchStatusResult(jobId);

    if (res.kind === 'gone') {
      stop();
      appendFailure('STATUS_UNAVAILABLE — no analysis exists for this job id');
      return;
    }

    if (res.kind === 'unreachable') {
      failures += 1;
      if (failures >= MAX_STATUS_FAILURES) {
        stop();
        appendFailure(
          `STATUS_UNREACHABLE — backend did not answer ${MAX_STATUS_FAILURES} times; polling stopped`,
        );
      }
      return;
    }

    failures = 0;
    const s = res.status;

    const sig = `${s.status}::${s.stage ?? ''}`;
    if (terminalSeen || sig === lastSig) return;

    if (s.status === 'idle' || s.status === 'failed' || s.status === 'terminated') {
      terminalSeen = true;
      lastSig = sig;
      append(render(s));
      stop();
      return;
    }

    lastSig = sig;
    append(render(s));
  };

  timer = window.setInterval(poll, intervalMs);
  poll();
  return () => { if (timer) window.clearInterval(timer); };
}

/** Replace a list container with an honest empty state (real jobs only). */
export function showEmptyState(container: HTMLElement | null, code: string, msg: string): void {
  if (!container) return;
  container.innerHTML = '';
  const wrap = document.createElement('div');
  wrap.style.cssText =
    'padding:28px 20px;font-family:JetBrains Mono,monospace;font-size:0.6875rem;' +
    'color:#5e3f3a;border:1px dashed #2a2a2a;text-align:center;line-height:1.8;letter-spacing:0.04em;';
  wrap.innerHTML =
    `<div style="color:#cc0000;font-weight:700;margin-bottom:6px;">${code}</div>` +
    `<div>${msg}</div>`;
  container.appendChild(wrap);
}

/** Append an honest error line to a feed container (real jobs only). */
export function appendErrorLine(container: HTMLElement | null, msg: string): void {
  if (!container || !getJobId()) return;
  const line = document.createElement('div');
  line.innerHTML =
    `<span style="color:#5e3f3a;">${nowTs()}</span> <span style="color:#cc0000;font-weight:700;">[ERROR]</span> ` +
    `<span style="color:#cc0000;">${msg}</span>`;
  container.appendChild(line);
  container.scrollTop = container.scrollHeight;
}
