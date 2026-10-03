/**
 * Local LLM provider backed by node-llama-cpp.
 * Placeholder: echoes the prompt until the runtime is wired in.
 */
export class NodeLlamaCppProvider {
  constructor(readonly modelPath: string) {}

  async generateText(prompt: string): Promise<string> {
    return `Mock response for: ${prompt}`;
  }
}
