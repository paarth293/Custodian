import { spawn } from 'node:child_process';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { readFrame, writeFrame } from '../src/ipc';
import { sha256 } from '../src/integrity';
import {
  CPU_MODEL_BYTES,
  createLlamaEngine,
  findChatModel,
  findSmallChatModel,
  llamaGpu,
  modelForDevice,
  prepareGpu,
  type ChatEngine,
} from '../src/llm';
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
  it('keeps structured interpretation separate from chat tools and supports cancellation', async () => {
    const replies: Reply[] = [];
    let calls = 0;
    const engine: ChatEngine = {
      chat: async () => {
        throw Error('Chat must not be used');
      },
      planIntent: async (_goal, signal) => {
        calls++;
        await new Promise<void>((resolve) =>
          signal?.addEventListener('abort', () => resolve(), { once: true }),
        );
        return {
          decision: 'clarify',
          filter: 'any',
          largest: false,
          source: 0,
          destination: 1,
          nodes: [],
          completion: 'all_copies_verified',
        };
      },
    };
    const handle = createHandler(engine, 'n/a', (r) => replies.push(r));
    const pending = handle({
      type: 'intent_plan',
      id: 500,
      message: 'Copy documents from LOCATION_0 to LOCATION_1',
    });
    await new Promise((resolve) => setTimeout(resolve, 0));
    await handle({ type: 'cancel', id: 500 });
    await pending;
    expect(calls).toBe(1);
    expect(replies.some((r) => r.type === 'tool' || r.type === 'token')).toBe(false);
    expect(replies).toContainEqual({ type: 'done', id: 500 });
  });
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

  it('handles deep ping requests', async () => {
    let checkedDeep: boolean | undefined;
    const probeEngine: ChatEngine = {
      ...fakeEngine,
      async check(deep) {
        checkedDeep = deep;
        return null;
      },
    };
    const replies: Reply[] = [];
    await createHandler(probeEngine, '', (r) => replies.push(r))({
      type: 'ping',
      id: 99,
      deep: true,
    });
    expect(checkedDeep).toBe(true);
    expect(replies).toEqual([{ type: 'pong', id: 99, ready: true }]);
  });

  it('handles reset requests to clear chat history', async () => {
    let resetCalled = false;
    const resetEngine: ChatEngine = {
      ...fakeEngine,
      async reset() {
        resetCalled = true;
      },
    };
    const replies: Reply[] = [];
    await createHandler(resetEngine, '', (r) => replies.push(r))({
      type: 'reset',
      id: 101,
    });
    expect(resetCalled).toBe(true);
    expect(replies).toEqual([{ type: 'done', id: 101 }]);
  });

  it('ignores malformed requests', async () => {
    const replies: Reply[] = [];
    await createHandler(fakeEngine, '', (r) => replies.push(r))({
      type: 'shell',
      command: 'rm -rf /',
    });
    expect(replies).toEqual([]);
  });

  it('waits for the active chat before resetting, and resets before the next chat', async () => {
    const events: string[] = [];
    let finish!: () => void;
    const first = new Promise<void>((resolve) => {
      finish = resolve;
    });
    const handle = createHandler(
      {
        async chat(message) {
          events.push(message);
          if (message === 'first') await first;
        },
        async reset() {
          events.push('reset');
        },
      },
      '',
      () => {},
    );
    const pending = handle({ type: 'chat', id: 1, message: 'first' });
    await Promise.resolve();
    const reset = handle({ type: 'reset', id: 2 });
    const next = handle({ type: 'chat', id: 3, message: 'next' });
    expect(events).toEqual(['first']);
    finish();
    await Promise.all([pending, reset, next]);
    expect(events).toEqual(['first', 'reset', 'next']);
  });
});

describe('findChatModel', () => {
  /** Writes `content` to `dir/name` and pins its digest in `dir/manifest.json`. */
  const addPinned = (dir: string, name: string, content: string) => {
    fs.writeFileSync(path.join(dir, name), content);
    const manifestPath = path.join(dir, 'manifest.json');
    const manifest = fs.existsSync(manifestPath)
      ? (JSON.parse(fs.readFileSync(manifestPath, 'utf8')) as Record<string, unknown>)
      : {};
    manifest[name] = { sha256: sha256(content) };
    fs.writeFileSync(manifestPath, JSON.stringify(manifest));
  };
  const modelAt = (dir: string, name: string) => ({
    path: path.join(dir, name),
    sha256: sha256('GGUF'),
  });

  it('prefers chat models over the Laya classifier', () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'models-'));
    addPinned(dir, 'laya.Q4_K_M.gguf', 'GGUF');
    expect(findChatModel(dir)).toEqual(modelAt(dir, 'laya.Q4_K_M.gguf'));
    addPinned(dir, 'gemma-2b-it.Q4_K_M.gguf', 'GGUF');
    expect(findChatModel(dir)).toEqual(modelAt(dir, 'gemma-2b-it.Q4_K_M.gguf'));
    expect(findChatModel(dir, 'laya.Q4_K_M.gguf')).toEqual(modelAt(dir, 'laya.Q4_K_M.gguf'));
    expect(findChatModel(dir, 'missing.gguf')).toBeNull();
    fs.rmSync(dir, { recursive: true, force: true });
  });

  it('uses the model the manifest marks as default', () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'models-'));
    addPinned(dir, 'a-small.gguf', 'GGUF');
    addPinned(dir, 'b-better.gguf', 'GGUF');
    expect(findChatModel(dir)).toEqual(modelAt(dir, 'a-small.gguf'));
    const manifestPath = path.join(dir, 'manifest.json');
    const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8')) as Record<
      string,
      Record<string, unknown>
    >;
    manifest['b-better.gguf']!['default'] = true;
    fs.writeFileSync(manifestPath, JSON.stringify(manifest));
    expect(findChatModel(dir)).toEqual(modelAt(dir, 'b-better.gguf'));
    // A default that is not a real model is passed over.
    manifest['c-broken.gguf'] = { sha256: sha256('x'), default: true };
    fs.writeFileSync(manifestPath, JSON.stringify(manifest));
    fs.writeFileSync(path.join(dir, 'c-broken.gguf'), 'x');
    expect(findChatModel(dir)).toEqual(modelAt(dir, 'b-better.gguf'));
    fs.rmSync(dir, { recursive: true, force: true });
  });

  it('skips placeholders and embedding models', () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'models-'));
    addPinned(dir, 'a-chat.gguf', 'mock_gemma_content');
    addPinned(dir, 'b-embedding.gguf', 'GGUF');
    expect(findChatModel(dir)).toBeNull();
    expect(findChatModel(dir, 'a-chat.gguf')).toBeNull();
    addPinned(dir, 'laya.Q4_K_M.gguf', 'GGUF');
    expect(findChatModel(dir)).toEqual(modelAt(dir, 'laya.Q4_K_M.gguf'));
    fs.rmSync(dir, { recursive: true, force: true });
  });

  it('never picks a model that is not pinned in the manifest', () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'models-'));
    fs.writeFileSync(path.join(dir, 'gemma-2b-it.Q4_K_M.gguf'), 'GGUF');
    expect(findChatModel(dir)).toBeNull();
    expect(findChatModel(dir, 'gemma-2b-it.Q4_K_M.gguf')).toBeNull();
    addPinned(dir, 'laya.Q4_K_M.gguf', 'GGUF');
    expect(findChatModel(dir)).toEqual(modelAt(dir, 'laya.Q4_K_M.gguf'));
    fs.writeFileSync(path.join(dir, 'manifest.json'), '{ not json');
    expect(findChatModel(dir)).toBeNull();
    fs.rmSync(dir, { recursive: true, force: true });
  });
});

describe('llamaGpu', () => {
  afterEach(() => vi.unstubAllEnvs());

  it('reads CUSTODIAN_LLAMA_GPU and uses a GPU when one works if it is unset', () => {
    vi.stubEnv('CUSTODIAN_LLAMA_GPU', undefined);
    expect(llamaGpu()).toBe('auto');
    vi.stubEnv('CUSTODIAN_LLAMA_GPU', 'vulkan');
    expect(llamaGpu()).toBe('vulkan');
  });

  it('runs on the CPU when asked for anything but a known GPU backend', () => {
    expect(llamaGpu('')).toBe('auto');
    expect(llamaGpu('  ')).toBe('auto');
    expect(llamaGpu('off')).toBe(false);
    expect(llamaGpu('cpu')).toBe(false);
    expect(llamaGpu('opencl')).toBe(false);
    expect(llamaGpu(' CUDA ')).toBe('cuda');
    expect(llamaGpu('auto')).toBe('auto');
    expect(llamaGpu('vulkan')).toBe('vulkan');
  });
});

describe('prepareGpu', () => {
  it('turns off the AMD layer that hides discrete GPUs, unless told otherwise', () => {
    const env: NodeJS.ProcessEnv = {};
    prepareGpu(env);
    expect(env['DISABLE_LAYER_AMD_SWITCHABLE_GRAPHICS_1']).toBe('1');
    const chosen: NodeJS.ProcessEnv = { DISABLE_LAYER_AMD_SWITCHABLE_GRAPHICS_1: '0' };
    prepareGpu(chosen);
    expect(chosen['DISABLE_LAYER_AMD_SWITCHABLE_GRAPHICS_1']).toBe('0');
  });
});

describe('findSmallChatModel and modelForDevice', () => {
  /** A pinned file of `size` bytes that starts like a GGUF model. */
  const addModel = (dir: string, name: string, size: number, extra: object = {}) => {
    fs.writeFileSync(path.join(dir, name), 'GGUF'.padEnd(size, '.'));
    const manifestPath = path.join(dir, 'manifest.json');
    const manifest = fs.existsSync(manifestPath)
      ? (JSON.parse(fs.readFileSync(manifestPath, 'utf8')) as Record<string, unknown>)
      : {};
    manifest[name] = { sha256: 'ab'.repeat(32), ...extra };
    fs.writeFileSync(manifestPath, JSON.stringify(manifest));
  };
  /** Bytes a model may have to run on the CPU, in these tests. */
  const limit = 1500;

  it('picks the smallest pinned chat model, never Laya or an embedder', () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'models-'));
    expect(findSmallChatModel(dir)).toBeNull();
    addModel(dir, 'b-big.gguf', 4000, { default: true });
    addModel(dir, 'laya.Q4_K_M.gguf', 10);
    addModel(dir, 'a-embedding.gguf', 10);
    expect(findSmallChatModel(dir)?.path).toBe(path.join(dir, 'b-big.gguf'));
    addModel(dir, 'c-small.gguf', 2000);
    expect(findSmallChatModel(dir)?.path).toBe(path.join(dir, 'c-small.gguf'));
    // Unpinned files are never used.
    fs.writeFileSync(path.join(dir, 'd-tiny.gguf'), 'GGUF');
    expect(findSmallChatModel(dir)?.path).toBe(path.join(dir, 'c-small.gguf'));
    fs.rmSync(dir, { recursive: true, force: true });
  });

  it('swaps in the small model only on the CPU, and only for a big model', () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'models-'));
    addModel(dir, 'a-4b.gguf', 2000, { default: true });
    addModel(dir, 'b-1.5b.gguf', 1000);
    const model = findChatModel(dir)!;
    const small = findSmallChatModel(dir)!;
    expect(model.path).toBe(path.join(dir, 'a-4b.gguf'));
    expect(modelForDevice(model, small, true, limit)).toBe(model);
    expect(modelForDevice(model, small, false, limit)).toBe(small);
    expect(modelForDevice(model, null, false, limit)).toBe(model);
    // A model small enough for the CPU is kept.
    expect(modelForDevice(small, model, false, limit)).toBe(small);
    expect(modelForDevice(model, small, false)).toBe(model);
    expect(CPU_MODEL_BYTES).toBeGreaterThan(2000);
    fs.rmSync(dir, { recursive: true, force: true });
  });
});

describe('createLlamaEngine', () => {
  it('refuses a model whose contents no longer match its pin', async () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'models-'));
    const file = path.join(dir, 'chat.gguf');
    fs.writeFileSync(file, 'GGUF tampered');
    const engine = createLlamaEngine({ path: file, sha256: sha256('GGUF original') });
    await expect(engine.chat('hi', () => {})).rejects.toThrow('integrity check');
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
