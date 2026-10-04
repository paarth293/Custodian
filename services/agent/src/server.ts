import { DEFAULT_MODELS_DIR, createLlamaEngine, findChatModel, type ChatEngine } from './llm';
import { MAX_FRAME_SIZE, readFrame, writeFrame } from './ipc';

const HEADER_SIZE = 4 + 4 + 32;

/** Requests the desktop app sends. */
export interface ChatRequest {
  type: 'chat';
  id: number;
  message: string;
}

/** Frames the agent sends back. */
export type Reply =
  | { type: 'token'; id: number; text: string }
  | { type: 'done'; id: number }
  | { type: 'error'; id: number; error: string };

/**
 * Splits a byte stream into authenticated frames. A frame whose signature
 * fails is dropped; a header that cannot be trusted ends the stream, because
 * the next frame boundary is unknown.
 */
export class FrameReader {
  private buffer = Buffer.alloc(0);

  constructor(private readonly key: Buffer) {}

  push(chunk: Buffer): unknown[] {
    this.buffer = Buffer.concat([this.buffer, chunk]);
    const frames: unknown[] = [];
    while (this.buffer.length >= HEADER_SIZE) {
      if (!this.buffer.subarray(0, 4).equals(Buffer.from('CUST')))
        throw new Error('stream desynchronised: bad magic');
      const length = this.buffer.readUInt32LE(4);
      if (length > MAX_FRAME_SIZE) throw new Error('stream desynchronised: frame too large');
      if (this.buffer.length < HEADER_SIZE + length) break;
      const frame = this.buffer.subarray(0, HEADER_SIZE + length);
      this.buffer = this.buffer.subarray(HEADER_SIZE + length);
      try {
        frames.push(readFrame(frame, this.key));
      } catch {
        // Unauthenticated frame: drop it silently.
      }
    }
    return frames;
  }
}

function isChatRequest(value: unknown): value is ChatRequest {
  const v = value as Partial<ChatRequest> | null;
  return !!v && v.type === 'chat' && typeof v.id === 'number' && typeof v.message === 'string';
}

/**
 * Answers chat requests one at a time with `engine`, or explains why no
 * model is available when `engine` is null.
 */
export function createHandler(
  engine: ChatEngine | null,
  unavailable: string,
  send: (reply: Reply) => void,
) {
  let queue = Promise.resolve();
  return (request: unknown): Promise<void> => {
    if (!isChatRequest(request)) return queue;
    const { id, message } = request;
    queue = queue.then(async () => {
      if (!engine) {
        send({ type: 'error', id, error: unavailable });
        return;
      }
      try {
        await engine.chat(message, (text) => send({ type: 'token', id, text }));
        send({ type: 'done', id });
      } catch (err) {
        send({ type: 'error', id, error: err instanceof Error ? err.message : String(err) });
      }
    });
    return queue;
  };
}

function main(): void {
  const keyText = process.env['CUSTODIAN_IPC_KEY'];
  if (!keyText) {
    console.error('[agent] CUSTODIAN_IPC_KEY is not set; refusing to start.');
    process.exit(2);
  }
  const key = Buffer.from(keyText, 'utf-8');
  const modelsDir = process.env['CUSTODIAN_MODELS_DIR'] ?? DEFAULT_MODELS_DIR;
  const modelPath = findChatModel(modelsDir, process.env['CUSTODIAN_CHAT_MODEL']);
  const engine = modelPath ? createLlamaEngine(modelPath) : null;
  const unavailable = `no chat model found in ${modelsDir}; add a GGUF chat model there`;
  console.error(modelPath ? `[agent] chat model: ${modelPath}` : `[agent] ${unavailable}`);

  const handle = createHandler(engine, unavailable, (reply) =>
    process.stdout.write(writeFrame(reply, key)),
  );
  const reader = new FrameReader(key);
  process.stdin.on('data', (chunk: Buffer) => {
    try {
      for (const request of reader.push(chunk)) void handle(request);
    } catch (err) {
      console.error(`[agent] ${err instanceof Error ? err.message : String(err)}`);
      process.exit(1);
    }
  });
  process.stdin.on('end', () => process.exit(0));
}

if (require.main === module) main();
