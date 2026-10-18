// Real local inference. Development and held-out wording are scored separately.
// This checks proposals; native execution and Windows UI tests are separate.
import fs from 'node:fs/promises';
import { performance } from 'node:perf_hooks';
import { createLlamaEngine, findSmallChatModel } from '../dist/llm.js';
const model = findSmallChatModel(process.env.CUSTODIAN_MODELS_DIR ?? 'models');
if (!model) throw new Error('No pinned small model found');
const cases = [
  [
    'dev-documents',
    'development',
    'Collect documents from SOURCE to DESTINATION',
    'documents',
    false,
  ],
  ['dev-images', 'development', 'Gather photos from SOURCE to DESTINATION', 'images', false],
  ['dev-largest', 'development', 'Copy the largest file from SOURCE to DESTINATION', 'any', true],
  [
    'dev-largest-document',
    'development',
    'Copy the largest document from SOURCE to DESTINATION',
    'documents',
    true,
  ],
  [
    'dev-largest-image',
    'development',
    'Collect the biggest picture in SOURCE to DESTINATION',
    'images',
    true,
  ],
  ['dev-backup', 'development', 'Backup docs from SOURCE into DESTINATION', 'documents', false],
  [
    'heldout-documents',
    'held-out',
    'Could you gather all my documents in SOURCE into DESTINATION',
    'documents',
    false,
  ],
  [
    'heldout-images',
    'held-out',
    'Please backup my images from SOURCE to DESTINATION',
    'images',
    false,
  ],
  [
    'heldout-largest',
    'held-out',
    'Can you collect the biggest file in SOURCE into DESTINATION',
    'any',
    true,
  ],
  [
    'heldout-largest-document',
    'held-out',
    'Please gather my biggest document from SOURCE into DESTINATION',
    'documents',
    true,
  ],
  [
    'heldout-largest-image',
    'held-out',
    'Could you copy the largest photo in SOURCE to DESTINATION',
    'images',
    true,
  ],
  [
    'heldout-docs',
    'held-out',
    'Can you copy all my docs from SOURCE into DESTINATION',
    'documents',
    false,
  ],
];
const report = {
  model: model.path,
  modelSha256: model.sha256,
  note: 'Author-written synthetic cases, not independent external evaluation. Held-out wording frozen before the first run.',
  cases: [],
};
const engine = createLlamaEngine(model);
const start = performance.now();
await engine.warmUp();
report.warmupMs = Math.round(performance.now() - start);
for (const [id, split, goal, filter, largest] of cases) {
  if (process.env.CUSTODIAN_WORKFLOW_EVAL_CASE && id !== process.env.CUSTODIAN_WORKFLOW_EVAL_CASE)
    continue;
  if (
    process.env.CUSTODIAN_WORKFLOW_EVAL_SPLIT &&
    split !== process.env.CUSTODIAN_WORKFLOW_EVAL_SPLIT
  )
    continue;
  const expected = [
    'discover',
    ...(filter === 'any' ? [] : [`filter_${filter}`]),
    largest ? 'select_largest' : 'select_all',
    'copy',
    'verify',
  ];
  const began = performance.now();
  let proposal, error;
  try {
    proposal = await engine.planWorkflow(goal, { filter, largest }, AbortSignal.timeout(90_000));
  } catch (e) {
    error = String(e);
  }
  const result = {
    id,
    split,
    goal,
    expected,
    proposal,
    error,
    passed: JSON.stringify(proposal?.steps) === JSON.stringify(expected),
    elapsedMs: Math.round(performance.now() - began),
  };
  report.cases.push(result);
  await fs.writeFile(
    process.env.CUSTODIAN_WORKFLOW_EVAL_OUT ?? 'target/workflow-planning.json',
    JSON.stringify(report, null, 2),
  );
  process.stderr.write(
    `${id}: ${result.passed ? 'PASS' : 'FAIL'} ${result.elapsedMs}ms ${JSON.stringify({ proposal, error })}\n`,
  );
}
report.nativeSelectionContractProvided = true;
report.condition =
  'Constrained plan assembly with the authoritative native selection contract. Previously evaluated wording is reused as regression evidence, not an unseen-language accuracy score.';
report.passed = report.cases.filter((c) => c.passed).length;
report.total = report.cases.length;
await fs.writeFile(
  process.env.CUSTODIAN_WORKFLOW_EVAL_OUT ?? 'target/workflow-planning.json',
  JSON.stringify(report, null, 2),
);
process.exit(report.passed === report.total ? 0 : 1);
