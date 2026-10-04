import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import CubbyCharacter from '../src/cubby/CubbyCharacter';
import { fluffPath } from '../src/cubby/fluff';
import { canBlink, pickIdleAction, resolveMood } from '../src/cubby/mood';
import type { Mood } from '../src/cubby/types';
import { formFor } from '../src/cubby/useMode';

const base = { held: false, activity: null, reaction: null, asleep: false } as const;

describe('resolveMood', () => {
  it('is idle by default', () => {
    expect(resolveMood(base)).toBe('idle');
  });

  it('prioritises being held, then work, then reactions, then sleep', () => {
    expect(
      resolveMood({ ...base, held: true, activity: 'working', reaction: 'happy', asleep: true }),
    ).toBe('held');
    expect(resolveMood({ ...base, activity: 'thinking', reaction: 'happy' })).toBe('thinking');
    expect(resolveMood({ ...base, activity: 'working', asleep: true })).toBe('working');
    expect(resolveMood({ ...base, activity: 'needs-you' })).toBe('curious');
    expect(resolveMood({ ...base, reaction: 'excited', asleep: true })).toBe('excited');
    expect(resolveMood({ ...base, asleep: true })).toBe('sleeping');
  });
});

describe('canBlink', () => {
  it('blinks only with open eyes', () => {
    expect(canBlink('idle')).toBe(true);
    expect(canBlink('sleeping')).toBe(false);
    expect(canBlink('happy')).toBe(false);
  });
});

describe('pickIdleAction', () => {
  it('maps random draws to bounded actions', () => {
    const draws = [0.1, 0.3, 0.5, 0.7, 0.95];
    for (const r of draws) {
      const action = pickIdleAction(() => r);
      expect(action.durationMs).toBeGreaterThan(0);
      expect(action.durationMs).toBeLessThanOrEqual(2500);
    }
    expect(pickIdleAction(() => 0.5).reaction).toBe('curious');
  });
});

describe('formFor', () => {
  it('is amber when online or previewing, blue otherwise', () => {
    expect(formFor({ online: false, demo: false })).toBe('home');
    expect(formFor({ online: true, demo: false })).toBe('special');
    expect(formFor({ online: false, demo: true })).toBe('special');
  });
});

describe('fluffPath', () => {
  const options = { cx: 100, cy: 100, rx: 50, ry: 40, tufts: 12, depth: 0.05, seed: 1 };

  it('is deterministic and closed', () => {
    const a = fluffPath(options);
    expect(fluffPath(options)).toBe(a);
    expect(a.startsWith('M')).toBe(true);
    expect(a.endsWith('Z')).toBe(true);
  });

  it('changes with the seed', () => {
    expect(fluffPath({ ...options, seed: 2 })).not.toBe(fluffPath(options));
  });
});

describe('CubbyCharacter', () => {
  const render = (mood: Mood, form: 'home' | 'special' = 'home') =>
    renderToStaticMarkup(<CubbyCharacter mood={mood} form={form} />);

  it('exposes mood and form for styling and an accessible label', () => {
    const html = render('working', 'special');
    expect(html).toContain('data-mood="working"');
    expect(html).toContain('data-form="special"');
    expect(html).toContain('aria-label="Cubby is working"');
  });

  it('draws open eyes when idle and closed arcs when sleeping', () => {
    expect(render('idle').match(/class="cubby-eye"/g)).toHaveLength(2);
    expect(render('sleeping')).not.toContain('class="cubby-eye"');
    expect(render('sleeping').match(/cubby-eye-arc/g)).toHaveLength(2);
  });

  it('winks with exactly one open eye', () => {
    const html = render('wink');
    expect(html.match(/class="cubby-eye"/g)).toHaveLength(1);
    expect(html.match(/cubby-eye-arc/g)).toHaveLength(1);
  });

  it('uses unique gradient ids per instance', () => {
    const html = renderToStaticMarkup(
      <>
        <CubbyCharacter mood="idle" form="home" />
        <CubbyCharacter mood="idle" form="home" />
      </>,
    );
    const ids = [...html.matchAll(/id="([^"]+)-fur"/g)].map((m) => m[1]);
    expect(new Set(ids).size).toBe(2);
  });
});
