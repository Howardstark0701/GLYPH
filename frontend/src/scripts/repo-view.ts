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
}

const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

export function apiBase(): string {
  return (
    (window as any).__GLYPH_API__ ??
    (import.meta as any).env?.PUBLIC_API_BASE_URL ??
    'http://localhost:8000'
  );
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

export async function fetchStatus(jobId: string): Promise<RepoStatus | null> {
  try {
    const r = await fetch(`${apiBase()}/api/repo/${jobId}/status`);
    if (!r.ok) return null;
    return (await r.json()) as RepoStatus;
  } catch {
    return null;
  }
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
 * Poll the real status endpoint and append honest lines to a feed
 * container. Stops when the analysis reaches a terminal state.
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
    line.innerHTML = html;
    container.appendChild(line);
    container.scrollTop = container.scrollHeight;
    while (container.children.length > maxLines) {
      container.removeChild(container.firstChild!);
    }
  };

  const poll = async () => {
    const s = await fetchStatus(jobId);
    if (!s) return;

    const sig = `${s.status}::${s.stage ?? ''}`;
    if (terminalSeen || sig === lastSig) return;

    if (s.status === 'idle' || s.status === 'failed' || s.status === 'terminated') {
      terminalSeen = true;
      lastSig = sig;
      append(render(s));
      if (timer) window.clearInterval(timer);
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
