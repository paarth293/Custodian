import { describe, it, expect } from 'vitest';
import { parseIntentPlan, proposeIntent, unsupportedIntent } from '../src/intent-plan';
const valid = () => ({
  decision: 'copy',
  filter: 'documents',
  largest: false,
  source: 0,
  destination: 1,
  nodes: ['discover', 'filter_documents', 'select_all', 'copy', 'verify'].map((op, i) => ({
    op,
    after: i - 1,
  })),
  completion: 'all_copies_verified',
});
describe('structured intent planning', () => {
  it('requires dependencies, registered effects and verification', () => {
    expect(parseIntentPlan(JSON.stringify(valid())).nodes).toHaveLength(5);
    const bad = valid();
    bad.nodes[3]!.op = 'delete';
    expect(() => parseIntentPlan(JSON.stringify(bad))).toThrow();
    const cycle = valid();
    cycle.nodes[0]!.after = 4;
    expect(() => parseIntentPlan(JSON.stringify(cycle))).toThrow();
    expect(() => parseIntentPlan(JSON.stringify({ ...valid(), destination: 0 }))).toThrow();
    expect(() => parseIntentPlan(JSON.stringify({ ...valid(), authority: 'allow-all' }))).toThrow();
  });
  it('blocks unregistered constraints and ambiguous references before inference', async () => {
    for (const text of [
      'Copy those',
      'Copy documents except invoices',
      'Copy docs and photos',
      'Copy and delete originals',
      'कॉपियां बनाओ',
    ])
      expect(unsupportedIntent(text)).toBe(true);
    let calls = 0;
    const p = await proposeIntent('Copy documents except invoices', async () => {
      calls++;
      return JSON.stringify(valid());
    });
    expect(p.decision).toBe('clarify');
    expect(calls).toBe(0);
  });
  it('repairs at most once while retaining the same goal and native data separation', async () => {
    const prompts: string[] = [];
    const p = await proposeIntent(
      'Copy documents from LOCATION_0 to LOCATION_1',
      async (prompt) => {
        prompts.push(prompt);
        return prompts.length === 1
          ? '{}'
          : JSON.stringify({ intent: 'documents', source: 0, destination: 1 });
      },
    );
    expect(p.attempts).toBe(2);
    expect(prompts[1]).toContain('Copy documents from LOCATION_0');
    expect(prompts[1]).toContain('untrusted_content');
    await expect(proposeIntent('Copy docs', async () => '{broken')).rejects.toThrow('two attempts');
  });
});
