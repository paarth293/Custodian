// Diagnostic only: real pinned CPU inference, no OS tools or desktop actions.
// Compare this normal-priority process with the native job/sandbox probes.
import fs from 'node:fs';
import { performance } from 'node:perf_hooks';
import { createLlamaEngine, findChatModel } from '../dist/llm.js';

const started = performance.now();
const model = findChatModel(
  process.env.CUSTODIAN_MODELS_DIR ?? 'models',
  'qwen2.5-1.5b-instruct-q4_k_m.gguf',
);
if (!model) throw new Error('Pinned CPU model not found');
process.env.CUSTODIAN_LLAMA_GPU = 'off';
const engine = createLlamaEngine(model);
const result = { model: model.path, sha256: model.sha256, toolCalls: [], reply: '', elapsedMs: 0 };
const stop = new AbortController();
const deadline = setTimeout(() => {
  stop.abort();
  result.elapsedMs = performance.now() - started;
  result.error = '120-second diagnostic deadline exceeded';
  console.log(JSON.stringify(result));
  process.exit(1);
}, 120_000);
try {
  await engine.chat(
    'Reply with one short sentence: what is a duplicate file?',
    (text) => {
      result.reply += text;
    },
    async (name) => {
      result.toolCalls.push(name);
      return 'No OS tools are available in this diagnostic.';
    },
    stop.signal,
  );
  result.elapsedMs = performance.now() - started;
  result.memory = process.memoryUsage();
  result.success = /identical|same (content|data)|exact copy/i.test(result.reply);
  console.log(JSON.stringify(result));
  if (process.env.CUSTODIAN_DIAGNOSTIC_REPORT)
    fs.writeFileSync(
      process.env.CUSTODIAN_DIAGNOSTIC_REPORT,
      `${JSON.stringify(result, null, 2)}\n`,
    );
  process.exitCode = result.success ? 0 : 1;
} finally {
  clearTimeout(deadline);
}
// Dispose the diagnostic worker even if the inference runtime retains handles.
process.exit(process.exitCode ?? 0);
