import {
  DEFAULT_MODELS_DIR,
  createLlamaEngine,
  findChatModel,
  findSmallChatModel,
  type ChatEngine,
} from './llm';
import { MAX_FRAME_SIZE, readFrame, writeFrame } from './ipc';
import type { ToolRunner } from './tools';
import { workflowSelection, type WorkflowSelection } from './workflow-plan';

const HEADER_SIZE = 4 + 4 + 32;

/** A message to answer, from the desktop app. */
export interface ChatRequest {
  type: 'chat';
  id: number;
  message: string;
}
export interface WorkflowRequest {
  type: 'workflow_plan';
  id: number;
  message: string;
  selection: WorkflowSelection;
}
export interface IntentRequest {
  type: 'intent_plan';
  id: number;
  message: string;
}

/** The desktop app's answer to a `tool` frame. */
export interface ToolResult {
  type: 'tool_result';
  id: number;
  call: number;
  result: string;
}

/** The person pressed Stop: end the answer to chat `id` now. */
export interface CancelRequest {
  type: 'cancel';
  id: number;
}

/** "Can you work?": answered at once with a `pong`, before any chat. */
export interface PingRequest {
  type: 'ping';
  id: number;
  deep?: boolean;
}

/** Reset the chat history for a fresh conversation. */
export interface ResetRequest {
  type: 'reset';
  id: number;
}

/** Frames the agent sends back. A `tool` frame asks the desktop to run a tool. */
export type Reply =
  | { type: 'token'; id: number; text: string }
  | { type: 'done'; id: number }
  | { type: 'error'; id: number; error: string }
  | { type: 'tool'; id: number; call: number; name: string; args: Record<string, unknown> }
  /** A quick pong checks configuration; `deep: true` checks initialized inference. */
  | { type: 'pong'; id: number; ready: boolean; error?: string };

/** How long to wait for the desktop to run one tool (a full scan can take minutes). */
export const TOOL_TIMEOUT_MS = 10 * 60 * 1000;

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
function isWorkflowRequest(value: unknown): value is WorkflowRequest | IntentRequest {
  const v = value as Partial<WorkflowRequest> | null;
  return (
    !!v &&
    (v.type === 'workflow_plan' || (v as { type?: string }).type === 'intent_plan') &&
    typeof v.id === 'number' &&
    typeof v.message === 'string' &&
    v.message.length <= 2400
  );
}

function isToolResult(value: unknown): value is ToolResult {
  const v = value as Partial<ToolResult> | null;
  return (
    !!v &&
    v.type === 'tool_result' &&
    typeof v.id === 'number' &&
    typeof v.call === 'number' &&
    typeof v.result === 'string'
  );
}

function isCancelRequest(value: unknown): value is CancelRequest {
  const v = value as Partial<CancelRequest> | null;
  return !!v && v.type === 'cancel' && typeof v.id === 'number';
}

function isPingRequest(value: unknown): value is PingRequest {
  const v = value as Partial<PingRequest> | null;
  return !!v && v.type === 'ping' && typeof v.id === 'number';
}

function isResetRequest(value: unknown): value is ResetRequest {
  const v = value as Partial<ResetRequest> | null;
  return !!v && v.type === 'reset' && typeof v.id === 'number';
}

/**
 * Answers chat requests one at a time with `engine`, or explains why no
 * model is available when `engine` is null. Tools the model calls are sent to
 * the desktop as `tool` frames; their `tool_result` frames are handled at
 * once, not queued, because the chat that asked for them is still running.
 * A `cancel` frame stops that chat's answer (or skips it if it has not
 * started), and the chat still ends with `done`.
 */
export function createHandler(
  engine: ChatEngine | null,
  unavailable: string,
  send: (reply: Reply) => void,
  toolTimeoutMs = TOOL_TIMEOUT_MS,
) {
  let queue = Promise.resolve();
  const waiting = new Map<string, (result: string) => void>();
  const stops = new Map<number, AbortController>();

  return (request: unknown): Promise<void> => {
    if (isToolResult(request)) {
      const key = `${request.id}:${request.call}`;
      const resolve = waiting.get(key);
      waiting.delete(key);
      resolve?.(request.result);
      return Promise.resolve();
    }
    if (isCancelRequest(request)) {
      stops.get(request.id)?.abort();
      return Promise.resolve();
    }
    if (isPingRequest(request)) {
      const { id } = request;
      if (!engine) {
        send({ type: 'pong', id, ready: false, error: unavailable });
        return Promise.resolve();
      }
      return (engine.check?.(request.deep) ?? Promise.resolve(null)).then(
        (problem) =>
          send(
            problem
              ? { type: 'pong', id, ready: false, error: problem }
              : { type: 'pong', id, ready: true },
          ),
        (err: unknown) =>
          send({
            type: 'pong',
            id,
            ready: false,
            error: err instanceof Error ? err.message : String(err),
          }),
      );
    }
    if (isResetRequest(request)) {
      const { id } = request;
      if (!engine) {
        send({ type: 'done', id });
        return Promise.resolve();
      }
      queue = queue.then(async () => {
        try {
          await engine.reset?.();
          send({ type: 'done', id });
        } catch (err) {
          send({ type: 'error', id, error: err instanceof Error ? err.message : String(err) });
        }
      });
      return queue;
    }
    if (!isChatRequest(request) && !isWorkflowRequest(request)) return queue;
    const planning = request.type === 'workflow_plan' || request.type === 'intent_plan';
    const { id, message } = request;
    const stop = new AbortController();
    stops.set(id, stop);
    let calls = 0;
    const runTool: ToolRunner = (name, args) =>
      new Promise((resolve) => {
        if (stop.signal.aborted) {
          resolve('The person stopped this request. Do not continue.');
          return;
        }
        const call = ++calls;
        const key = `${id}:${call}`;
        const finish = (result: string) => {
          clearTimeout(timer);
          waiting.delete(key);
          stop.signal.removeEventListener('abort', onAbort);
          resolve(result);
        };
        const onAbort = () => finish('The person stopped this request. Do not continue.');
        const timer = setTimeout(() => finish('The PC did not answer in time.'), toolTimeoutMs);
        waiting.set(key, finish);
        stop.signal.addEventListener('abort', onAbort, { once: true });
        send({ type: 'tool', id, call, name, args });
      });

    queue = queue.then(async () => {
      try {
        if (stop.signal.aborted) {
          send({ type: 'done', id });
          return;
        }
        if (!engine) {
          send({ type: 'error', id, error: unavailable });
          return;
        }
        if (planning) {
          if (request.type === 'intent_plan') {
            if (!engine.planIntent) throw new Error('Intent planning unavailable');
            const proposal = await engine.planIntent(message, stop.signal);
            if (!stop.signal.aborted) send({ type: 'token', id, text: JSON.stringify(proposal) });
          } else {
            if (!engine.planWorkflow) throw new Error('Workflow planning is unavailable');
            const selection = workflowSelection((request as WorkflowRequest).selection);
            const proposal = await engine.planWorkflow(message, selection, stop.signal);
            if (!stop.signal.aborted) send({ type: 'token', id, text: JSON.stringify(proposal) });
          }
        } else
          await engine.chat(
            message,
            (text) => {
              if (!stop.signal.aborted) send({ type: 'token', id, text });
            },
            runTool,
            stop.signal,
          );
        send({ type: 'done', id });
      } catch (err) {
        if (stop.signal.aborted) send({ type: 'done', id });
        else send({ type: 'error', id, error: err instanceof Error ? err.message : String(err) });
      } finally {
        for (const [key, finish] of waiting) {
          if (key.startsWith(`${id}:`)) finish('This request has ended. Do not continue.');
        }
        stops.delete(id);
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
  const named = process.env['CUSTODIAN_CHAT_MODEL'];
  const model = findChatModel(modelsDir, named);
  // A model asked for by name is always used; otherwise, without a GPU, the
  // smallest chat model stands in for one too big to answer quickly.
  const small = named ? null : findSmallChatModel(modelsDir);
  const engine = model ? createLlamaEngine(model, small) : null;
  const unavailable = `no chat model found in ${modelsDir}; add a pinned GGUF chat model with scripts/fetch-models.ps1`;
  console.error(model ? `[agent] chat model: ${model.path}` : `[agent] ${unavailable}`);
  // Load the model now, so it is ready by the time the first message arrives.
  // A failure here is reported again, to the person, on that message.
  engine?.warmUp?.().catch((err: unknown) => {
    console.error(`[agent] warm-up failed: ${err instanceof Error ? err.message : String(err)}`);
  });

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
