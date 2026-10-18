import { describe, expect, it, vi } from 'vitest';
import { createHandler, type Reply } from '../src/server';
import {
  parseWorkflowProposal,
  workflowPrompt,
  workflowSchema,
  workflowSelection,
  proposeWorkflow,
} from '../src/workflow-plan';
import { createLlamaEngine, type ChatEngine } from '../src/llm';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

describe('isolated workflow planning', () => {
  it('answers quick configuration checks without waiting for native runtime initialization', async () => {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), 'custodian-health-'));
    try {
      const model = path.join(root, 'header-only.gguf');
      fs.writeFileSync(model, 'GGUF');
      const engine = createLlamaEngine({ path: model, sha256: '0'.repeat(64) });
      let settled = false;
      let problem: string | null | undefined;
      const checked = engine.check!(false).then((value) => {
        settled = true;
        problem = value;
      });
      await Promise.resolve();
      expect(settled).toBe(true);
      expect(problem).toBeNull();
      await checked;
      // This is deliberately not an inference-ready model. Its pin/header
      // remains insufficient for getSession, which still verifies full bytes.
    } finally {
      fs.rmSync(root, { recursive: true, force: true });
    }
  });
  it('repairs one rejected proposal using compiler feedback', async () => {
    const log = vi.spyOn(console, 'error').mockImplementation(() => {});
    try {
      const generate = vi
        .fn()
        .mockResolvedValueOnce(
          JSON.stringify({
            steps: ['discover', 'select_largest', 'copy', 'verify', 'select_largest'],
          }),
        )
        .mockResolvedValueOnce(
          JSON.stringify({
            steps: ['discover', 'filter_documents', 'select_largest', 'copy', 'verify'],
          }),
        );
      const result = await proposeWorkflow(
        'Copy largest document',
        { filter: 'documents', largest: true },
        generate,
      );
      expect(result.attempts).toBe(2);
      expect(generate).toHaveBeenCalledTimes(2);
      expect(generate.mock.calls[1]?.[0]).toContain('Missing capabilities: filter_documents');
    } finally {
      log.mockRestore();
    }
  });
  it('fails after two rejected proposals without a third attempt', async () => {
    const log = vi.spyOn(console, 'error').mockImplementation(() => {});
    try {
      const generate = vi.fn().mockResolvedValue('not JSON');
      await expect(
        proposeWorkflow('Collect documents', { filter: 'documents', largest: false }, generate),
      ).rejects.toThrow('after two model attempts');
      expect(generate).toHaveBeenCalledTimes(2);
    } finally {
      log.mockRestore();
    }
  });
  it('does not generate a plan after cancellation', async () => {
    const stop = new AbortController();
    stop.abort();
    const generate = vi.fn();
    await expect(
      proposeWorkflow(
        'Collect documents',
        { filter: 'documents', largest: false },
        generate,
        stop.signal,
      ),
    ).rejects.toThrow('stopped');
    expect(generate).not.toHaveBeenCalled();
  });
  it('routes a proposal without invoking normal chat or desktop tools', async () => {
    const replies: Reply[] = [];
    const engine: ChatEngine = {
      async chat() {
        throw new Error('Normal chat must not run');
      },
      async planWorkflow(goal, selection) {
        expect(goal).toBe('Collect documents from SOURCE to DESTINATION');
        expect(selection).toEqual({ filter: 'documents', largest: false });
        return { steps: ['discover', 'filter_documents', 'select_all', 'copy', 'verify'] };
      },
    };
    await createHandler(engine, 'unavailable', (r) => replies.push(r))({
      type: 'workflow_plan',
      id: 1,
      message: 'Collect documents from SOURCE to DESTINATION',
      selection: { filter: 'documents', largest: false },
    });
    expect(replies.map((r) => r.type)).toEqual(['token', 'done']);
    expect(replies[0]).toMatchObject({
      id: 1,
      text: JSON.stringify({
        steps: ['discover', 'filter_documents', 'select_all', 'copy', 'verify'],
      }),
    });
  });
  it('does not fall back to normal chat when planning is unavailable', async () => {
    const replies: Reply[] = [];
    await createHandler(
      {
        async chat() {
          throw new Error('must not run');
        },
      },
      'unavailable',
      (r) => replies.push(r),
    )({ type: 'workflow_plan', id: 1, message: 'Collect documents' });
    expect(replies).toEqual([{ type: 'error', id: 1, error: 'Workflow planning is unavailable' }]);
  });
  it('suppresses a late proposal after cancellation', async () => {
    let release: (() => void) | undefined;
    const replies: Reply[] = [];
    const handle = createHandler(
      {
        async chat() {},
        async planWorkflow() {
          await new Promise<void>((resolve) => {
            release = resolve;
          });
          return { steps: ['discover', 'select_largest', 'copy', 'verify'] };
        },
      },
      'unavailable',
      (r) => replies.push(r),
    );
    const running = handle({
      type: 'workflow_plan',
      id: 1,
      message: 'Copy largest file',
      selection: { filter: 'any', largest: true },
    });
    await Promise.resolve();
    await handle({ type: 'cancel', id: 1 });
    release?.();
    await running;
    expect(replies).toEqual([{ type: 'done', id: 1 }]);
  });
  it('rejects extra authority, paths, code and oversized proposals', () => {
    for (const text of [
      '{"steps":["discover","select_all","copy","verify"],"grant":true}',
      '{"steps":["discover","select_all","shell","verify"]}',
      '{"steps":["discover","select_all","copy","verify"],"destination":"C:\\\\Windows"}',
      'not JSON',
      '{"steps":[]}',
    ]) {
      expect(() => parseWorkflowProposal(text)).toThrow();
    }
    expect(() => workflowPrompt('x'.repeat(2401))).toThrow();
  });
  it('constrains vocabulary to the native selection instead of reinterpreting it', () => {
    const schema = workflowSchema({ filter: 'images', largest: false });
    expect(schema.properties.steps.items.enum).toEqual([
      'discover',
      'filter_images',
      'select_all',
      'copy',
      'verify',
    ]);
    for (const value of [
      { filter: ['any'], largest: false },
      { filter: 'images', largest: 'true' },
      { filter: 'any', largest: false, grant: true },
    ])
      expect(() => workflowSelection(value)).toThrow();
  });
});
