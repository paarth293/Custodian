import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { pickIdleAction } from './mood';
import type { Gaze, Mood } from './types';

/** Fall asleep after this long without any interaction or nearby cursor. */
export const SLEEP_AFTER_MS = 3 * 60 * 1000;

export interface CubbyLife {
  reaction: Mood | null;
  /** Temporary gaze override from an idle routine. */
  look: Gaze | null;
  asleep: boolean;
  /** Shows a reaction for `ms` milliseconds. */
  react: (mood: Mood, ms?: number) => void;
  /** Call on any user interaction or nearby cursor movement. */
  poke: () => void;
  sleep: () => void;
}

/**
 * Cubby's inner life: passing reactions, little idle routines while nothing
 * happens, and falling asleep when the user is away. `busy` pauses idle
 * routines and sleep while Cubby works. `greeting` is shown on start-up.
 */
export function useCubbyLife(busy: boolean, greeting: Mood | null = null): CubbyLife {
  const [reaction, setReaction] = useState<Mood | null>(greeting);
  const [look, setLook] = useState<Gaze | null>(null);
  const [asleep, setAsleep] = useState(false);
  const lastPoke = useRef(0);
  const asleepRef = useRef(false);
  const reactionTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  const react = useCallback((mood: Mood, ms = 1800) => {
    clearTimeout(reactionTimer.current);
    setReaction(mood);
    reactionTimer.current = setTimeout(() => setReaction(null), ms);
  }, []);

  const poke = useCallback(() => {
    lastPoke.current = Date.now();
    if (asleepRef.current) {
      asleepRef.current = false;
      setAsleep(false);
      react('happy', 1500);
    }
  }, [react]);

  const sleep = useCallback(() => {
    clearTimeout(reactionTimer.current);
    setReaction(null);
    lastPoke.current = 0;
    asleepRef.current = true;
    setAsleep(true);
  }, []);

  // Falling asleep.
  useEffect(() => {
    if (busy) return;
    const id = setInterval(() => {
      if (Date.now() - lastPoke.current > SLEEP_AFTER_MS) {
        asleepRef.current = true;
        setAsleep(true);
      }
    }, 5000);
    return () => clearInterval(id);
  }, [busy]);

  // Idle routines every 9–16 s.
  useEffect(() => {
    if (busy || asleep) return;
    let timer: ReturnType<typeof setTimeout>;
    let lookTimer: ReturnType<typeof setTimeout> | undefined;
    const next = () => {
      timer = setTimeout(
        () => {
          const action = pickIdleAction();
          if (action.reaction) react(action.reaction, action.durationMs);
          if (action.look) {
            setLook(action.look);
            lookTimer = setTimeout(() => setLook(null), action.durationMs);
          }
          next();
        },
        9000 + Math.random() * 7000,
      );
    };
    next();
    return () => {
      clearTimeout(timer);
      clearTimeout(lookTimer);
    };
  }, [busy, asleep, react]);

  // Start the inactivity clock and end the greeting reaction.
  useEffect(() => {
    lastPoke.current = Date.now();
    reactionTimer.current = setTimeout(() => setReaction(null), 2200);
    return () => clearTimeout(reactionTimer.current);
  }, []);

  return useMemo(
    () => ({ reaction, look, asleep, react, poke, sleep }),
    [reaction, look, asleep, react, poke, sleep],
  );
}
