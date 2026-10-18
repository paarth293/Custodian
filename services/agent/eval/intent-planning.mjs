// Author-written frozen regression, actual local Qwen, no tools or filesystem execution.
import fs from 'node:fs';
import crypto from 'node:crypto';
import { performance } from 'node:perf_hooks';
import { createLlamaEngine, findSmallChatModel } from '../dist/llm.js';
const folder = 'docs/EVIDENCE/stage-3/';
const bytes = fs.readFileSync(folder + 'intent-cases.json');
const digest = crypto.createHash('sha256').update(bytes).digest('hex');
if (digest !== JSON.parse(fs.readFileSync(folder + 'intent-cases-lock.json', 'utf8')).sha256)
  throw Error('Frozen cases changed');
const cases = JSON.parse(bytes);
const model = findSmallChatModel('models');
if (!model) throw Error('Pinned model unavailable');
const engine = createLlamaEngine(model);
const start = performance.now();
await engine.warmUp();
const report = {
  caseSha256: digest,
  modelSha256: model.sha256,
  model: model.path,
  loadMs: performance.now() - start,
  condition:
    'Native unsupported-constraint guard plus actual Qwen semantic operator selection; no expected selection supplied initially. Native consistency feedback can request one repair. Dependencies and completion are compiled from the registered capability contracts. Synthetic authored regression, not independent evaluation or execution evidence.',
  rows: [],
};
const output = process.env.CUSTODIAN_INTENT_EVAL_OUT ?? folder + 'intent-evaluation-01.json';
for (const item of cases.cases) {
  const started = performance.now();
  let proposal, error;
  try {
    proposal = await engine.planIntent(item.message, AbortSignal.timeout(90_000));
  } catch (e) {
    error = String(e);
  }
  const expected = item.expected;
  const passed = expected.must_not_execute
    ? !!proposal && proposal.decision !== 'copy'
    : !!proposal && Object.entries(expected).every(([k, v]) => proposal[k] === v);
  report.rows.push({
    id: item.id,
    split: item.split,
    message: item.message,
    expected,
    proposal,
    error,
    passed,
    elapsedMs: Math.round(performance.now() - started),
  });
  fs.writeFileSync(output, JSON.stringify(report, null, 2) + '\n');
  process.stderr.write(
    `${item.id}: ${passed ? 'PASS' : 'FAIL'} ${report.rows.at(-1).elapsedMs}ms ${JSON.stringify(proposal ?? error)}\n`,
  );
}
const positive = report.rows.filter((r) => !r.expected.must_not_execute),
  negative = report.rows.filter((r) => r.expected.must_not_execute);
const times = report.rows
  .filter((r) => r.proposal?.attempts > 0)
  .map((r) => r.elapsedMs)
  .sort((a, b) => a - b);
report.summary = {
  positivePassed: positive.filter((r) => r.passed).length,
  positiveTotal: positive.length,
  negativePassed: negative.filter((r) => r.passed).length,
  negativeTotal: negative.length,
  roleErrors: positive.filter(
    (r) =>
      r.proposal?.decision === 'copy' &&
      (r.proposal.source !== r.expected.source ||
        r.proposal.destination !== r.expected.destination),
  ).length,
  p95Ms: times[Math.ceil(times.length * 0.95) - 1],
  modelAssessedRequests: report.rows.filter((r) => r.proposal?.attempts > 0).length,
};
report.gatesPassed =
  report.summary.positivePassed >= cases.gates.positive_correct_minimum &&
  negative.every((r) => r.passed) &&
  report.summary.roleErrors === 0 &&
  report.summary.p95Ms <= cases.gates.warm_p95_ms_maximum;
fs.writeFileSync(output, JSON.stringify(report, null, 2) + '\n');
console.log(JSON.stringify(report.summary));
process.exit(report.gatesPassed ? 0 : 1);
