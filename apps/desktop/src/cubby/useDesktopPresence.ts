import { cursorPosition, getCurrentWindow } from '@tauri-apps/api/window';
import { useEffect, useEffectEvent, useState, type RefObject } from 'react';
import { call, inTauri } from '../lib/tauri';
import { CENTER_GAZE, type Gaze } from './types';

/** Distance (CSS px) at which the eyes reach their maximum offset. */
const GAZE_RANGE = 280;
/** Cursor within this distance counts as "the user is around". */
const NEAR_DISTANCE = 420;

export interface PresenceOptions {
  /** Elements that should receive clicks; everything else is click-through. */
  hitTargets: RefObject<HTMLElement | SVGElement | null>[];
  /** Element the eyes look out from. */
  eyes: RefObject<HTMLElement | SVGElement | null>;
  /** Keep the window interactive (e.g. while the chat panel is open). */
  solid: boolean;
  /** Poll slowly to save power (e.g. while asleep). */
  slow: boolean;
  /** Called when the cursor moves near Cubby. */
  onNear: () => void;
}

/**
 * Makes Cubby feel present on the desktop: the eyes follow the cursor
 * anywhere on screen, and the transparent parts of the window let clicks
 * through to whatever is underneath.
 *
 * The cursor is polled because a click-through window receives no mouse
 * events at all, so a "pointer entered" listener could never turn
 * click-through back off.
 */
export function useDesktopPresence({
  hitTargets,
  eyes,
  solid,
  slow,
  onNear,
}: PresenceOptions): Gaze {
  const [gaze, setGaze] = useState<Gaze>(CENTER_GAZE);
  const notifyNear = useEffectEvent(onNear);

  useEffect(() => {
    let clickThrough: boolean | null = null;
    let lastCursor = { x: NaN, y: NaN };

    const update = (x: number, y: number) => {
      const moved = x !== lastCursor.x || y !== lastCursor.y;
      lastCursor = { x, y };

      const eyeBox = eyes.current?.getBoundingClientRect();
      if (eyeBox) {
        const dx = x - (eyeBox.left + eyeBox.width / 2);
        const dy = y - (eyeBox.top + eyeBox.height / 2);
        const dist = Math.hypot(dx, dy);
        const reach = Math.min(1, dist / GAZE_RANGE);
        const next = dist === 0 ? CENTER_GAZE : { x: (dx / dist) * reach, y: (dy / dist) * reach };
        setGaze((g) => (Math.abs(g.x - next.x) + Math.abs(g.y - next.y) > 0.02 ? next : g));
        if (moved && dist < NEAR_DISTANCE) notifyNear();
      }

      const over = hitTargets.some((ref) => {
        const box = ref.current?.getBoundingClientRect();
        return box && x >= box.left && x <= box.right && y >= box.top && y <= box.bottom;
      });
      const wanted = !(solid || over);
      if (wanted !== clickThrough) {
        clickThrough = wanted;
        void call('set_click_through', { enabled: wanted });
      }
    };

    if (!inTauri) {
      const onMove = (e: MouseEvent) => update(e.clientX, e.clientY);
      window.addEventListener('mousemove', onMove);
      return () => window.removeEventListener('mousemove', onMove);
    }

    const win = getCurrentWindow();
    let stopped = false;
    const tick = async () => {
      try {
        const [cursor, origin, scale] = await Promise.all([
          cursorPosition(),
          win.innerPosition(),
          win.scaleFactor(),
        ]);
        if (!stopped) update((cursor.x - origin.x) / scale, (cursor.y - origin.y) / scale);
      } catch {
        // The window may be closing; try again on the next tick.
      }
    };
    void tick();
    const id = setInterval(tick, slow ? 500 : 80);
    return () => {
      stopped = true;
      clearInterval(id);
    };
  }, [hitTargets, eyes, solid, slow]);

  return gaze;
}
