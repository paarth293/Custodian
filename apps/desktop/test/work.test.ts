import { expect, it } from 'vitest';
import { updateWork, type ConversationWork } from '../src/chat/work';
import type { Plan } from '../src/components/ApprovalView';

it('finishing one chat preserves the other scan and its progress', () => {
  let state: ConversationWork = { turns: {}, plans: [] };
  state = updateWork(state, { kind: 'start', id: 'scan' });
  state = updateWork(state, { kind: 'status', id: 'scan', text: 'Scanning 1,000 files' });
  state = updateWork(state, { kind: 'start', id: 'question' });
  state = updateWork(state, { kind: 'finish', id: 'question' });
  expect(state.turns).toEqual({ scan: 'Scanning 1,000 files' });
});

it('keeps independent approval plans and removes only the one decided', () => {
  let state: ConversationWork = { turns: { a: null, b: null }, plans: [] };
  const plan = (id: string): Plan => ({ id, operations: [], riskLevel: 'low' });
  state = updateWork(state, { kind: 'plan', id: 'a', plan: plan('first') });
  state = updateWork(state, { kind: 'plan', id: 'b', plan: plan('second') });
  state = updateWork(state, { kind: 'decided', planId: 'first' });
  expect(state.plans.map((p) => p.id)).toEqual(['second']);
});

it('ignores old progress and plans after a new conversation starts', () => {
  let state: ConversationWork = { turns: { old: null }, plans: [] };
  state = updateWork(state, { kind: 'reset' });
  expect(updateWork(state, { kind: 'status', id: 'old', text: 'stale' })).toBe(state);
  expect(
    updateWork(state, {
      kind: 'plan',
      id: 'old',
      plan: { id: 'stale', operations: [], riskLevel: 'low' },
    }),
  ).toBe(state);
});
