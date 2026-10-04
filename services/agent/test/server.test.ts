import { spawn } from 'node:child_process';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';
import { describe, expect, it } from 'vitest';
import { readFrame, writeFrame } from '../src/ipc';
import { findChatModel, type ChatEngine } from '../src/llm';
import { createHandler, FrameReader, type Reply } from '../src/server';

const key = Buffer.from('test-key');

describe('FrameReader', () => {
  it('reassembles frames split across chunks and drops forged ones', () => {
    const reader = new FrameReader(key);
    const good = writeFrame({ type: 'chat', id: 1, message: 'hi' }, key);
    const forged = writeFrame({ type: 'chat', id: 2, message: 'evil' }, Buffer.from('attacker'));
    const stream = Buffer.concat([good, forged, good]);

    const frames = [...reader.push(stream.subarray(0, 10)), ...reader.push(stream.subarray(10))];
    expect(frames).toEqual([
      { type: 'chat', id: 1, message: 'hi' },
      { type: 'chat', id: 1, message: 'hi' },
    ]);
  });

  it('refuses a desynchronised stream', () => {
    const reader = new FrameReader(key);
    expect(() => reader.push(Buffer.alloc(64, 0x41))).toThrow('desynchronised');
  });
});

describe('createHandler', () => {
  const fakeEngine: ChatEngine = {
    async chat(message, onText) {
      onText('You said: ');
      onText(message);
    },
  };

  it('streams tokens then done, one request at a time', async () => {
    const replies: Reply[] = [];
    const handle = createHandler(fakeEngine, 'n/a', (r) => replies.push(r));
    void handle({ type: 'chat', id: 1, message: 'a' });
    await handle({ type: 'chat', id: 2, message: 'b' });
    expect(replies).toEqual([
      { type: 'token', id: 1, text: 'You said: ' },
      { type: 'token', id: 1, text: 'a' },
      { type: 'done', id: 1 },
      { type: 'token', id: 2, text: 'You said: ' },
      { type: 'token', id: 2, text: 'b' },
      { type: 'done', id: 2 },
    ]);
  });

  it('explains when no model is installed', async () => {
    const replies: Reply[] = [];
    await createHandler(null, 'no chat model found', (r) => replies.push(r))({
      type: 'chat',
      id: 7,
      message: 'hi',
    });
    expect(replies).toEqual([{ type: 'error', id: 7, error: 'no chat model found' }]);
  });

  it('reports engine failures as errors', async () => {
    const broken: ChatEngine = { chat: () => Promise.reject(new Error('out of memory')) };
    const replies: Reply[] = [];
    await createHandler(broken, '', (r) => replies.push(r))({ type: 'chat', id: 3, message: 'x' });
    expect(replies).toEqual([{ type: 'error', id: 3, error: 'out of memory' }]);
  });

  it('ignores malformed requests', async () => {
    const replies: Reply[] = [];
    await createHandler(fakeEngine, '', (r) => replies.push(r))({
      type: 'shell',
      command: 'rm -rf /',
    });
    expect(replies).toEqual([]);
  });
});

describe('findChatModel', () => {
  it('prefers chat models over the Laya classifier', () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'models-'));
    fs.writeFileSync(path.join(dir, 'laya.Q4_K_M.gguf'), '');
    expect(findChatModel(dir)).toBe(path.join(dir, 'laya.Q4_K_M.gguf'));
    fs.writeFileSync(path.join(dir, 'gemma-2b-it.Q4_K_M.gguf'), '');
    expect(findChatModel(dir)).toBe(path.join(dir, 'gemma-2b-it.Q4_K_M.gguf'));
    expect(findChatModel(dir, 'missing.gguf')).toBeNull();
    fs.rmSync(dir, { recursive: true, force: true });
  });
});

describe('agent process (end to end)', () => {
  it('answers authenticated requests over stdio', async () => {
    const entry = path.resolve(__dirname, '..', 'dist', 'server.js');
    expect(fs.existsSync(entry), 'build the agent first (npm run build)').toBe(true);
    const emptyModels = fs.mkdtempSync(path.join(os.tmpdir(), 'no-models-'));
    const child = spawn(process.execPath, [entry], {
      env: { ...process.env, CUSTODIAN_IPC_KEY: 'test-key', CUSTODIAN_MODELS_DIR: emptyModels },
      stdio: ['pipe', 'pipe', 'ignore'],
    });

    const reply = await new Promise<unknown>((resolve, reject) => {
      let buffer = Buffer.alloc(0);
      child.stdout.on('data', (chunk: Buffer) => {
        buffer = Buffer.concat([buffer, chunk]);
        try {
          resolve(readFrame(buffer, key));
        } catch {
          // Wait for the rest of the frame.
        }
      });
      child.on('error', reject);
      child.stdin.write(writeFrame({ type: 'chat', id: 1, message: 'hello' }, key));
    });
    child.kill();
    fs.rmSync(emptyModels, { recursive: true, force: true });

    expect(reply).toMatchObject({ type: 'error', id: 1 });
    expect((reply as { error: string }).error).toContain('no chat model found');
  });
});
