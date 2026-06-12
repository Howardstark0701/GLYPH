# BUGHUNT // ANIMATION-DURATION: 0s

> 24+ hours of sustained combat against a single CSS property.
> Final outcome: GLYPH logo glitch and data-packet flow now operational.

---

## The Target

Two animated elements on the GLYPH landing page:

| Element | Animation | Expected Duration |
|---|---|---|
| `.hero-title` | `high-intensity-glitch` (glitch distortion) | 2s |
| `.data-packet-continuous` | `flow-continuous` (diamond dot riding the BYOK trace line) | 4s |

Both were correctly wired in the component CSS — keyframes defined, animation shorthand set, HTML structure correct. Yet on the live build, both were dead. CSS `animation-name` resolved correctly. `animation-duration`: **`0s`**.

---

## The Kill Chain

### Layer 1 — The Global Eradicator

```css
*, *::before, *::after {
  border-radius: 0 !important;
  box-shadow: none !important;
  text-shadow: none !important;
}
```

Innocent. No animation kill here. But the trap was laid elsewhere.

### Layer 2 — The Whitelist Paradox

```css
.hero-title,
.hero-title::before,
.hero-title::after,
.data-packet-continuous,
... {
  animation-duration: unset !important;
  transition: none !important;
  filter: unset !important;
}
```

This block was designed as the **animation whitelist** — intended to exempt specific elements from a global `animation-duration: 0s` suppressant. But the mechanism was fatally flawed:

| CSS Value | Resolves To | Effect |
|---|---|---|
| `animation-duration: unset` | `initial` = `0s` (non-inherited property) | ❌ Still kills animation |

The whitelist was itself the killer. Every class added to the whitelist was simultaneously granted "exemption" and handed a death sentence. `animation-duration: unset !important` — same specificity as the component's `animation` shorthand, but `!important` won. Duration collapsed to `0s`.

### Layer 3 — The Cascade War

The `.hero-title` in the Astro component's scoped `<style>` block set:

```css
.hero-title {
  animation: high-intensity-glitch 2s infinite steps(1);
}
```

But `global.css` loaded the whitelist AFTER the component CSS in the build. The whitelist's `!important` overrode the shorthand's `2s`. DevTools showed:

| Property | Value | Source |
|---|---|---|
| `animation-name` | `high-intensity-glitch` | Component (wins) |
| `animation-duration` | `0s` | global.css whitelist (wins, `!important`) |
| `animation-timing-function` | `steps(1)` | Component (wins) |
| `animation-iteration-count` | `infinite` | Component (wins) |

Name, timing, iteration — all correct. Duration: **zero**. The animation computed but never played.

---

## The Fix

Two `!important` overrides placed **after** the whitelist rule in `global.css`:

```css
.hero-title {
  animation-duration: 2s !important;
}

.data-packet-continuous {
  animation-duration: 4s !important;
}
```

By positioning them after the whitelist block, the cascade order becomes:

1. Whitelist: `animation-duration: unset !important` → 0s
2. Override: `animation-duration: 2s !important` → 2s ⬅ wins

Same specificity, both `!important` — the later declaration wins. This is the CSS cascade rule that swallowed 24 hours.

---

## Key Technical Lesson

**`animation-duration: unset` does not "allow" an animation to play.** For non-inherited properties (including `animation-duration`), `unset` is `initial`, and the initial value is `0s`. An animation whitelist must explicitly set the intended duration, or — better — avoid `!important` altogether and rely on cascade order.

The correct pattern for an animation exemption system:

```css
/* ❌ Broken — unset kills the duration */
.animated-element {
  animation-duration: unset !important;
}

/* ✅ Working — explicit duration after the shorthand */
.component-style {
  animation: my-anim 2s infinite;
}
.global-override {
  animation-duration: 2s !important;  /* overrides any global 0s kill */
}
```

Or, preferred: do not use `!important` on `animation-duration` globally. Let the cascade work naturally.

---

## Trophy

Two animations now function as specified:

- `.hero-title` — `high-intensity-glitch` @ 2s, `steps(1)`, infinite
- `.data-packet-continuous` — `flow-continuous` @ 4s, linear, infinite

Confirmed in the built CSS at `.vercel/output/static/_astro/index.*.css`.

---

## File Manifest

| File | Role |
|---|---|
| `frontend/src/styles/global.css` | Whitelist + duration overrides |
| `frontend/src/pages/index.astro` | Component CSS with animation shorthand |
| `frontend/.vercel/output/static/_astro/index.*.css` | Verified build output |

---

*Filed 2026-06-11. The cascade does not forgive. We learn. We override. We move.*
