// Counts study time on a lesson or item page. Time only counts while the window is shown and focused and the
// learner has touched the mouse, keyboard or scroll wheel in the last two minutes, so a page left open overnight
// adds nothing. Time is sent about once a minute and when the page closes.

import { api } from "#lib/api.ts";
import { localDay } from "#lib/insights.ts";
import { prefs } from "#lib/prefs.svelte.ts";

const TICK_MS = 15_000;
const IDLE_MS = 120_000;
const SEND_EVERY_SECS = 60;

export function trackStudy(itemId: string): () => void {
  let lastInput = Date.now();
  let pending = 0;
  const input = () => (lastInput = Date.now());
  const events = ["pointermove", "pointerdown", "keydown", "wheel", "scroll"] as const;
  for (const e of events) window.addEventListener(e, input, { passive: true, capture: true });

  const send = () => {
    if (pending <= 0) return;
    const now = new Date();
    const seconds = Math.round(pending);
    pending = 0;
    api.recordStudy(itemId, localDay(now), now.getHours(), seconds).catch(() => {});
  };

  const tick = setInterval(() => {
    if (!prefs.trackStudy) {
      pending = 0;
      return;
    }
    if (!document.hidden && document.hasFocus() && Date.now() - lastInput < IDLE_MS) pending += TICK_MS / 1000;
    if (pending >= SEND_EVERY_SECS) send();
  }, TICK_MS);

  const hidden = () => {
    if (document.hidden) send();
  };
  document.addEventListener("visibilitychange", hidden);
  window.addEventListener("pagehide", send);

  return () => {
    clearInterval(tick);
    send();
    for (const e of events) window.removeEventListener(e, input, { capture: true });
    document.removeEventListener("visibilitychange", hidden);
    window.removeEventListener("pagehide", send);
  };
}
