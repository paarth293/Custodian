import { getLlama, LlamaChatSession } from 'node-llama-cpp';
import * as path from 'node:path';

let sessionPromise: Promise<LlamaChatSession> | null = null;

/**
 * Initializes and returns a chat session connected to the local GGUF model.
 */
export async function getChatSession(): Promise<LlamaChatSession> {
  if (!sessionPromise) {
    sessionPromise = (async () => {
      const llama = await getLlama();
      const modelPath = path.join(__dirname, '..', '..', '..', 'models', 'laya.Q4_K_M.gguf');

      const model = await llama.loadModel({ modelPath });
      const context = await model.createContext();
      const sequence = context.getSequence();

      return new LlamaChatSession({
        contextSequence: sequence,
        systemPrompt:
          'You are Cubby, a helpful and friendly AI desktop companion for Windows. You answer questions directly and concisely.',
      });
    })();
  }
  return sessionPromise;
}

/**
 * Sends a message to the local model and returns the response.
 */
export async function chatWithModel(
  message: string,
  onToken?: (token: string) => void,
): Promise<string> {
  const session = await getChatSession();

  // onTextChunk streams already-decoded text, so no manual token decoding.
  return onToken ? session.prompt(message, { onTextChunk: onToken }) : session.prompt(message);
}
