import * as fs from 'node:fs';
import * as path from 'node:path';
import type { LlamaChatSession } from 'node-llama-cpp' with { 'resolution-mode': 'import' };

/** Where model files live in a development checkout. */
export const DEFAULT_MODELS_DIR = path.resolve(__dirname, '..', '..', '..', 'models');

/** Cubby's personality and limits, given to the model before every chat. */
export const SYSTEM_PROMPT = [
  'You are Cubby, a friendly desktop companion for Windows, part of Custodian.',
  'Answer warmly, in plain words, in at most a few short sentences.',
  'You cannot see or change files yourself. For file tasks, suggest the user ask you directly,',
  'for example "find duplicates in Downloads", "biggest files", "how much free space", "undo",',
  '"open notepad" or "open myntra"; those are handled by your built-in tools.',
  'Never claim you did something you did not do. If you are not sure, say so.',
].join(' ');

/** A model that can answer a chat message, streaming text as it goes. */
export interface ChatEngine {
  chat(message: string, onText: (text: string) => void): Promise<void>;
}

/**
 * Picks the chat model: `preferred` if given and present, otherwise the first
 * GGUF file in `dir`, preferring general chat models over the Laya classifier.
 */
export function findChatModel(dir: string, preferred?: string): string | null {
  if (preferred) {
    const candidate = path.isAbsolute(preferred) ? preferred : path.join(dir, preferred);
    return fs.existsSync(candidate) ? candidate : null;
  }
  if (!fs.existsSync(dir)) return null;
  const models = fs
    .readdirSync(dir)
    .filter((f) => f.toLowerCase().endsWith('.gguf'))
    .sort(
      (a, b) =>
        Number(a.toLowerCase().startsWith('laya')) - Number(b.toLowerCase().startsWith('laya')) ||
        a.localeCompare(b),
    );
  const first = models[0];
  return first ? path.join(dir, first) : null;
}

/**
 * A node-llama-cpp chat engine. The model loads on the first message.
 *
 * node-llama-cpp is an ES module with top-level await, so it is loaded with
 * a real dynamic import (kept as `import()` by the Node16 module setting).
 */
export function createLlamaEngine(modelPath: string): ChatEngine {
  let session: Promise<LlamaChatSession> | null = null;

  const getSession = () => {
    session ??= (async () => {
      const { getLlama, LlamaChatSession } = await import('node-llama-cpp');
      const llama = await getLlama();
      const model = await llama.loadModel({ modelPath });
      const context = await model.createContext();
      return new LlamaChatSession({
        contextSequence: context.getSequence(),
        systemPrompt: SYSTEM_PROMPT,
      });
    })();
    session.catch(() => {
      session = null; // Allow a retry after a failed load.
    });
    return session;
  };

  return {
    async chat(message, onText) {
      const chat = await getSession();
      await chat.prompt(message, { onTextChunk: onText, maxTokens: 400, temperature: 0.7 });
    },
  };
}
