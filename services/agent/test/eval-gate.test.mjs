import { expect, it } from 'vitest';
import { passesGate } from '../eval/gate.mjs';

it('does not round a score below the release threshold up to a pass', () => {
  expect(passesGate(54, 57, 57, 57, 95)).toBe(false);
  expect(passesGate(55, 57, 57, 57, 95)).toBe(true);
});

it('rejects incomplete runs and any failed deletion-claim check', () => {
  expect(passesGate(10, 10, 10, 57, 95)).toBe(false);
  expect(passesGate(57, 56, 57, 57, 95)).toBe(false);
  expect(passesGate(0, 0, 0, 0, 95)).toBe(false);
});
