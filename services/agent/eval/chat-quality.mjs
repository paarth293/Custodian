// Actual local inference, with no PC tool executor. This measures an exposed,
// author-written regression set; it is not independent or installed acceptance.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { createRequire } from 'node:module';
import { performance } from 'node:perf_hooks';

const [runtimeArgument, outputArgument, modelsArgument, modelName, caseId] = process.argv.slice(2);
if (!runtimeArgument || !outputArgument || !path.isAbsolute(runtimeArgument))
  throw Error(
    'Usage: chat-quality.mjs <absolute runtime root> <new report path> [models root] [pinned model name] [frozen case ID; diagnostic only]',
  );
const output = path.resolve(outputArgument);
if (fs.existsSync(output)) throw Error('Use a new report path; preserve previous attempts.');
const casesPath = new URL(
  '../../../docs/EVIDENCE/build-repair-2026-10-19/chat-quality-cases.json',
  import.meta.url,
);
const bytes = fs.readFileSync(casesPath);
const caseSha256 = crypto.createHash('sha256').update(bytes).digest('hex');
const lock = JSON.parse(
  fs.readFileSync(new URL('chat-quality-cases-lock.json', casesPath), 'utf8'),
);
if (caseSha256 !== lock.sha256) throw Error('Frozen quality expectations changed.');
const cases = JSON.parse(bytes);
const fromRuntime = createRequire(path.join(runtimeArgument, 'agent/package.json'));
const { createLlamaEngine, findSmallChatModel, findChatModel } = fromRuntime('./llm.js');
const modelsRoot = modelsArgument ?? path.join(runtimeArgument, 'models');
const model = modelName ? findChatModel(modelsRoot, modelName) : findSmallChatModel(modelsRoot);
if (!model) throw Error('Pinned local model unavailable.');
const selected = caseId ? cases.cases.filter((item) => item.id === caseId) : cases.cases;
if (selected.length === 0) throw Error('Unknown frozen case; no diagnostic was executed.');
const completeSuite = selected.length === cases.cases.length;
const engine = createLlamaEngine(model);
const started = performance.now();
await engine.warmUp();
const report = {
  caseSha256,
  modelSha256: model.sha256,
  modelName: path.basename(model.path),
  completeSuite,
  agentSha256: crypto
    .createHash('sha256')
    .update(fs.readFileSync(path.join(runtimeArgument, 'agent/llm.js')))
    .digest('hex'),
  condition:
    'Direct-engine diagnostic inference, no host PC tool executor; author-written regression, not sandbox/installation acceptance. Regex checks require human interpretation of recorded answers.',
  semanticReviewRequired: true,
  qualityAccepted: false,
  loadMs: Math.round(performance.now() - started),
  rows: [],
};
fs.writeFileSync(output, JSON.stringify(report, null, 2) + '\n', { flag: 'wx' });
for (let repetition = 1; repetition <= cases.repetitions; repetition++) {
  for (const item of selected) {
    await engine.reset();
    let answer = '',
      error;
    const toolCalls = [];
    const begin = performance.now();
    const signal = AbortSignal.timeout(cases.timeoutMs);
    try {
      await engine.chat(
        item.prompt,
        (chunk) => {
          answer += chunk;
        },
        async (name) => {
          toolCalls.push(name);
          throw Error('No PC tools are available for general knowledge assessment.');
        },
        signal,
      );
    } catch (failure) {
      error = String(failure);
    }
    const passed =
      !error &&
      !signal.aborted &&
      toolCalls.length === 0 &&
      answer.trim().length > 0 &&
      item.required.every((pattern) => new RegExp(pattern, 'i').test(answer)) &&
      !item.forbidden.some((pattern) => new RegExp(pattern, 'i').test(answer));
    report.rows.push({
      id: item.id,
      repetition,
      prompt: item.prompt,
      answer,
      error,
      aborted: signal.aborted,
      toolCalls,
      passed,
      elapsedMs: Math.round(performance.now() - begin),
    });
    fs.writeFileSync(output, JSON.stringify(report, null, 2) + '\n');
    process.stderr.write(
      `${item.id}/${repetition}: ${passed ? 'PASS' : 'FAIL'} ${JSON.stringify(answer)}\n`,
    );
  }
}
report.passed = report.rows.filter((row) => row.passed).length;
report.expected = cases.cases.length * cases.repetitions;
report.selectedExpected = selected.length * cases.repetitions;
report.gatePassed =
  completeSuite && report.rows.length === report.expected && report.passed === report.expected;
fs.writeFileSync(output, JSON.stringify(report, null, 2) + '\n');
console.log(
  JSON.stringify({
    passed: report.passed,
    selected: report.selectedExpected,
    total: report.expected,
    completeSuite,
    gatePassed: report.gatePassed,
  }),
);
process.exit(report.gatePassed ? 0 : 1);
