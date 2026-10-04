import type { Mood } from './types';

/** What the app is doing on the user's behalf, shown through Cubby's mood. */
export type Activity = 'thinking' | 'working' | 'needs-you';

export interface MoodInputs {
  /** Being dragged across the screen. */
  held: boolean;
  /** Ongoing work; takes priority over passing reactions. */
  activity: Activity | null;
  /** A short-lived reaction (happy after a click, curious on hover, …). */
  reaction: Mood | null;
  asleep: boolean;
}

/** Picks the single mood to display; higher-priority states win. */
export function resolveMood({ held, activity, reaction, asleep }: MoodInputs): Mood {
  if (held) return 'held';
  if (activity === 'thinking') return 'thinking';
  if (activity === 'working') return 'working';
  if (activity === 'needs-you') return 'curious';
  if (reaction) return reaction;
  if (asleep) return 'sleeping';
  return 'idle';
}

/** Moods during which the eyes are open and can blink. */
export function canBlink(mood: Mood): boolean {
  return (
    mood === 'idle' ||
    mood === 'curious' ||
    mood === 'thinking' ||
    mood === 'working' ||
    mood === 'held'
  );
}

export interface IdleAction {
  reaction: Mood | null;
  /** Temporary gaze override, or null to keep following the cursor. */
  look: { x: number; y: number } | null;
  durationMs: number;
}

/**
 * A small random "bit of life" while idle: glancing around, a curious tilt
 * or a happy wiggle. `random` is injectable for tests.
 */
export function pickIdleAction(random: () => number = Math.random): IdleAction {
  const r = random();
  if (r < 0.35)
    return { reaction: null, look: { x: random() < 0.5 ? -1 : 1, y: -0.2 }, durationMs: 1400 };
  if (r < 0.6) return { reaction: 'curious', look: null, durationMs: 2200 };
  if (r < 0.85) return { reaction: 'happy', look: null, durationMs: 1600 };
  return { reaction: null, look: { x: 0, y: 1 }, durationMs: 1200 };
}
