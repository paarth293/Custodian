/**
 * Builds a soft, furry outline: an ellipse whose radius is modulated by
 * small bumps, smoothed with Catmull-Rom curves. The result is deterministic
 * for a given seed, so Cubby looks the same on every render.
 */
export interface FluffOptions {
  cx: number;
  cy: number;
  rx: number;
  ry: number;
  /** Number of fur tufts around the outline. */
  tufts: number;
  /** Tuft height as a fraction of the radius. */
  depth: number;
  seed: number;
  /** Extra width added to the lower half (cheeks), as a fraction of rx. */
  cheeks?: number;
}

function mulberry32(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const round = (n: number) => Math.round(n * 100) / 100;

export function fluffPath(o: FluffOptions): string {
  const random = mulberry32(o.seed);
  const points: Array<[number, number]> = [];
  const samples = o.tufts * 2;

  for (let i = 0; i < samples; i++) {
    const angle = (i / samples) * Math.PI * 2;
    // Alternate tuft tips and valleys, with a little randomness.
    const tip = i % 2 === 0;
    const bump = tip ? o.depth * (0.6 + random() * 0.6) : -o.depth * 0.25;
    const lower = Math.max(0, Math.sin(angle));
    const cheek = 1 + (o.cheeks ?? 0) * lower * lower;
    const x = o.cx + Math.cos(angle) * o.rx * cheek * (1 + bump);
    const y = o.cy + Math.sin(angle) * o.ry * (1 + bump);
    points.push([x, y]);
  }

  // Closed Catmull-Rom spline converted to cubic Béziers.
  const n = points.length;
  const at = (i: number) => points[((i % n) + n) % n] as [number, number];
  let d = `M${round(at(0)[0])} ${round(at(0)[1])}`;
  for (let i = 0; i < n; i++) {
    const p0 = at(i - 1);
    const p1 = at(i);
    const p2 = at(i + 1);
    const p3 = at(i + 2);
    const c1x = p1[0] + (p2[0] - p0[0]) / 6;
    const c1y = p1[1] + (p2[1] - p0[1]) / 6;
    const c2x = p2[0] - (p3[0] - p1[0]) / 6;
    const c2y = p2[1] - (p3[1] - p1[1]) / 6;
    d += ` C${round(c1x)} ${round(c1y)} ${round(c2x)} ${round(c2y)} ${round(p2[0])} ${round(p2[1])}`;
  }
  return `${d} Z`;
}
