/** Cubby's facial expression and pose. */
export type Mood =
  'idle' | 'happy' | 'curious' | 'thinking' | 'working' | 'excited' | 'sleeping' | 'wink' | 'held';

/**
 * Cubby's colour form. `home` (blue) means offline; `special` (amber) means
 * Online mode is on (design decision D-17).
 */
export type Form = 'home' | 'special';

/** Where Cubby's eyes look, each axis in the range -1..1. */
export interface Gaze {
  x: number;
  y: number;
}

export const CENTER_GAZE: Gaze = { x: 0, y: 0 };
