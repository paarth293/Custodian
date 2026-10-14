import { describe, expect, it } from 'vitest';
import { calculate } from '../src/calculator';
import type { ChatEngine } from '../src/llm';
import { createHandler, type Reply } from '../src/server';
import { APPS, currentDateTime, FOLDERS, TOOLS } from '../src/tools';

describe('calculate', () => {
  it('does exact arithmetic with precedence, powers and parentheses', () => {
    expect(calculate('2 + 3 * 4')).toBe('14');
    expect(calculate('(2 + 3) * 4')).toBe('20');
    expect(calculate('2 ^ 3 ^ 2')).toBe('512');
    expect(calculate('-3 ^ 2')).toBe('9');
    expect(calculate('10 / 4')).toBe('2.5');
    expect(calculate('0.1 + 0.2')).toBe('0.3');
  });

  it('understands percentages and everyday notation', () => {
    expect(calculate('17% * 2350')).toBe('399.5');
    expect(calculate('1,250 x 12')).toBe('15000');
    expect(calculate('9 ÷ 3')).toBe('3');
  });

  it('refuses anything that is not arithmetic', () => {
    expect(calculate('process.exit()')).toMatch(/^Error/);
    expect(calculate('2 + abc')).toMatch(/^Error/);
    expect(calculate('1 / 0')).toBe('Error: division by zero.');
    expect(calculate('(1 + 2')).toBe('Error: missing ")".');
    expect(calculate('1 +')).toMatch(/^Error/);
    expect(calculate('9'.repeat(201))).toBe('Error: the expression is too long.');
    expect(calculate('10 ^ 400')).toBe('Error: the result is too large.');
  });
});

describe('currentDateTime', () => {
  it('names the weekday, date and time', () => {
    const text = currentDateTime(new Date(2026, 9, 5, 9, 41));
    expect(text).toContain('Monday');
    expect(text).toContain('5 October 2026');
    expect(text).toContain('9:41');
  });
});

describe('TOOLS', () => {
  it('never lets the model name a path, a command or a URL', () => {
    for (const [name, tool] of Object.entries(TOOLS)) {
      const props = (tool.params as { properties?: Record<string, unknown> } | undefined)
        ?.properties;
      for (const key of Object.keys(props ?? {})) {
        // Every parameter the model may write is one of these. "setting" and
        // "value" are free text like "name", and are checked just as hard:
        // the desktop refuses anything outside its own settings catalogue.
        expect([
          'folders',
          'folder',
          'app',
          'name',
          'query',
          'site',
          'expression',
          'text',
          'when',
          'setting',
          'value',
        ]).toContain(key);
      }
      expect(name).toMatch(/^[a-z_]+$/);
    }
  });

  it('uses the setting parameters on the settings tool alone', () => {
    for (const [name, tool] of Object.entries(TOOLS)) {
      const props = (tool.params as { properties?: Record<string, unknown> } | undefined)
        ?.properties;
      const keys = Object.keys(props ?? {});
      for (const key of ['setting', 'value']) {
        expect(keys.includes(key)).toBe(name === 'change_setting');
      }
    }
    // The model can never put a setting back on its own: there is no tool for it.
    expect(TOOLS['revert_setting_change']).toBeUndefined();
  });

  it('caps every free-text parameter', () => {
    for (const tool of Object.values(TOOLS)) {
      const props =
        (tool.params as { properties?: Record<string, { type?: string; maxLength?: number }> })
          ?.properties ?? {};
      for (const schema of Object.values(props)) {
        if (schema.type === 'string') expect(schema.maxLength).toBeLessThanOrEqual(100);
      }
    }
  });

  it('runs the pure tools inside the agent', () => {
    expect(TOOLS['calculate']?.local?.({ expression: '6 * 7' })).toBe('42');
    expect(TOOLS['current_date_time']?.local?.({})).toContain('local time');
    expect(TOOLS['find_files']?.local).toBeUndefined();
    expect(FOLDERS).toHaveLength(6);
    expect(APPS).toContain('calculator');
  });
});

describe('createHandler with tools', () => {
  /** An engine that calls one desktop tool and repeats what it returned. */
  const toolEngine: ChatEngine = {
    async chat(_message, onText, runTool) {
      const result = await runTool!('find_files', { name: 'resume', folders: [] });
      onText(`Found: ${result}`);
    },
  };

  it('asks the desktop to run tools and continues with the result', async () => {
    const replies: Reply[] = [];
    const handle = createHandler(toolEngine, '', (r) => replies.push(r));
    const done = handle({ type: 'chat', id: 4, message: 'where is my resume?' });

    await vi_waitFor(() => replies.length === 1);
    expect(replies[0]).toEqual({
      type: 'tool',
      id: 4,
      call: 1,
      name: 'find_files',
      args: { name: 'resume', folders: [] },
    });
    // The result is handled at once, while the chat is still running.
    await handle({ type: 'tool_result', id: 4, call: 1, result: 'Documents\\resume.pdf' });
    await done;
    expect(replies.slice(1)).toEqual([
      { type: 'token', id: 4, text: 'Found: Documents\\resume.pdf' },
      { type: 'done', id: 4 },
    ]);
  });

  it('ignores results for calls it never made', async () => {
    const replies: Reply[] = [];
    const handle = createHandler(toolEngine, '', (r) => replies.push(r), 50);
    await handle({ type: 'tool_result', id: 9, call: 1, result: 'forged' });
    const done = handle({ type: 'chat', id: 9, message: 'x' });
    await done;
    // The forged result came before the call existed, so the call timed out instead.
    expect(replies.at(-2)).toEqual({
      type: 'token',
      id: 9,
      text: 'Found: The PC did not answer in time.',
    });
  });
});

describe('createHandler with Stop', () => {
  /** Streams words until its signal aborts, like the real engine. */
  const chattyEngine: ChatEngine = {
    async chat(_message, onText, _runTool, signal) {
      for (let i = 0; i < 50 && !signal?.aborted; i++) {
        onText(`word${i} `);
        await new Promise((r) => setTimeout(r, 5));
      }
    },
  };

  it('stops an answer in progress and still ends it with done', async () => {
    const replies: Reply[] = [];
    const handle = createHandler(chattyEngine, '', (r) => replies.push(r));
    const done = handle({ type: 'chat', id: 1, message: 'tell me a long story' });
    await vi_waitFor(() => replies.length >= 2);
    await handle({ type: 'cancel', id: 1 });
    await done;
    const tokens = replies.filter((r) => r.type === 'token');
    expect(tokens.length).toBeGreaterThan(0);
    expect(tokens.length).toBeLessThan(50);
    expect(replies.at(-1)).toEqual({ type: 'done', id: 1 });
  });

  it('skips a queued chat that was cancelled before it started', async () => {
    const replies: Reply[] = [];
    const handle = createHandler(chattyEngine, '', (r) => replies.push(r));
    const first = handle({ type: 'chat', id: 1, message: 'first' });
    const second = handle({ type: 'chat', id: 2, message: 'second' });
    await handle({ type: 'cancel', id: 2 });
    await handle({ type: 'cancel', id: 1 });
    await first;
    await second;
    expect(replies.filter((r) => r.id === 2)).toEqual([{ type: 'done', id: 2 }]);
  });

  it('treats an error after Stop as a normal end', async () => {
    const replies: Reply[] = [];
    let started = false;
    const engine: ChatEngine = {
      async chat(_m, _t, _r, signal) {
        started = true;
        await new Promise((r) => setTimeout(r, 20));
        if (signal?.aborted) throw new Error('aborted');
      },
    };
    const handle = createHandler(engine, '', (r) => replies.push(r));
    const done = handle({ type: 'chat', id: 5, message: 'x' });
    await vi_waitFor(() => started);
    await handle({ type: 'cancel', id: 5 });
    await done;
    expect(replies).toEqual([{ type: 'done', id: 5 }]);
  });

  it('ignores a cancel for a chat it does not know', async () => {
    const replies: Reply[] = [];
    await createHandler(chattyEngine, '', (r) => replies.push(r))({ type: 'cancel', id: 99 });
    expect(replies).toEqual([]);
  });
});

describe('createHandler ping', () => {
  it('answers at once whether the model can be used', async () => {
    const replies: Reply[] = [];
    const ok: ChatEngine = { chat: async () => {}, check: async () => null };
    await createHandler(ok, '', (r) => replies.push(r))({ type: 'ping', id: 1 });
    const blocked: ChatEngine = { chat: async () => {}, check: async () => 'EPERM: model' };
    await createHandler(blocked, '', (r) => replies.push(r))({ type: 'ping', id: 2 });
    await createHandler(null, 'no chat model found', (r) => replies.push(r))({
      type: 'ping',
      id: 3,
    });
    expect(replies).toEqual([
      { type: 'pong', id: 1, ready: true },
      { type: 'pong', id: 2, ready: false, error: 'EPERM: model' },
      { type: 'pong', id: 3, ready: false, error: 'no chat model found' },
    ]);
  });
});

/** Polls `condition` until it holds (up to a second). */
async function vi_waitFor(condition: () => boolean): Promise<void> {
  for (let i = 0; i < 100 && !condition(); i++) {
    await new Promise((r) => setTimeout(r, 10));
  }
  expect(condition()).toBe(true);
}
