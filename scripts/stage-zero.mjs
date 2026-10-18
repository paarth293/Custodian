// Read-only application audit. Writes only documentation evidence when requested.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const read = (name) =>
  JSON.parse(fs.readFileSync(path.join(root, name), 'utf8').replace(/^\uFEFF/, ''));
const sha = (value) => crypto.createHash('sha256').update(value).digest('hex');
const git = (...args) =>
  execFileSync('git', args, { cwd: root, encoding: 'utf8', maxBuffer: 32 * 1024 * 1024 });
const fail = (message) => {
  throw new Error(message);
};
const write = (name, value) =>
  fs.writeFileSync(path.join(root, name), `${JSON.stringify(value, null, 2)}\n`);
const register = read('docs/CAPABILITY-REGISTER.json');
const fixtures = read('docs/STAGE-0-ACCEPTANCE-FIXTURES.json');
const lockPath = 'docs/EVIDENCE/stage-0/acceptance-lock.json';
const rows = register.requirements;
const ids = new Set();
const sourcePaths = new Set();
const safeFile = (name) => {
  if (
    typeof name !== 'string' ||
    name.includes('\\') ||
    name.split('/').includes('..') ||
    path.isAbsolute(name)
  )
    fail(`Unsafe repository path: ${name}`);
  const resolved = path.join(root, name);
  if (!fs.existsSync(resolved) || !fs.lstatSync(resolved).isFile())
    fail(`Missing regular file: ${name}`);
  return resolved;
};
for (const [key, route] of Object.entries(register.routes)) {
  if (!route.component || !route.trace || !route.sources.length) fail(`Incomplete route ${key}`);
  route.sources.forEach((name) => {
    safeFile(name);
    sourcePaths.add(name);
  });
}
for (const row of rows) {
  if (!/^[A-Z]\d{2}$/.test(row.id) || ids.has(row.id))
    fail(`Invalid/duplicate requirement ${row.id}`);
  ids.add(row.id);
  if (
    !register.statuses[row.status] ||
    !register.routes[row.route] ||
    !row.domain ||
    !row.capability
  )
    fail(`Invalid requirement ${row.id}`);
  if (
    !(Number.isInteger(row.stage) && row.stage >= 1 && row.stage <= 12) &&
    !(row.stage === null && row.status === 'deferred')
  )
    fail(`Missing gate ${row.id}`);
  if (
    !row.acceptance?.task ||
    !row.acceptance?.expected ||
    !Array.isArray(row.dependencies) ||
    !Array.isArray(row.evidence)
  )
    fail(`Incomplete acceptance ${row.id}`);
  if (row.status === 'verified_subset' && !row.evidence.length)
    fail(`Verified claim without evidence: ${row.id}`);
  for (const key of row.evidence) {
    if (!register.evidence[key]) fail(`Unknown evidence ${key}`);
    safeFile(register.evidence[key]);
  }
}
const visited = new Set();
const active = new Set();
const visit = (id) => {
  if (!ids.has(id)) fail(`Unknown dependency ${id}`);
  if (active.has(id)) fail(`Dependency cycle at ${id}`);
  if (visited.has(id)) return;
  active.add(id);
  rows.find((row) => row.id === id).dependencies.forEach(visit);
  active.delete(id);
  visited.add(id);
};
rows.forEach((row) => visit(row.id));
for (const domain of [
  'desktop',
  'files',
  'content',
  'planning',
  'settings',
  'development',
  'routines',
  'interaction',
  'security',
  'deployment',
  'online',
  'platform',
]) {
  if (!rows.some((row) => row.domain === domain)) fail(`Untracked original domain ${domain}`);
}
const caseIds = new Set();
for (const test of fixtures.cases) {
  if (caseIds.has(test.id) || !test.input || !Object.keys(test.expected ?? {}).length)
    fail(`Invalid fixture case ${test.id}`);
  caseIds.add(test.id);
  test.requirements.forEach((id) => {
    if (!ids.has(id)) fail(`Unknown fixture requirement ${id}`);
  });
}
const acceptance = {
  requirements: rows.map(({ id, acceptance }) => ({ id, acceptance })),
  fixtures,
};
const acceptanceHash = sha(JSON.stringify(acceptance));
if (process.argv.includes('--freeze')) {
  if (fs.existsSync(path.join(root, lockPath)))
    fail('Acceptance lock already exists; do not silently replace frozen expectations.');
  write(lockPath, {
    schemaVersion: 1,
    frozenAtUtc: new Date().toISOString(),
    sha256: acceptanceHash,
    requirementTasks: rows.length,
    fixtureCases: fixtures.cases.length,
  });
}
if (!fs.existsSync(path.join(root, lockPath)) || read(lockPath).sha256 !== acceptanceHash)
  fail('Frozen acceptance differs or is missing. Review and version changes explicitly.');

const lines = [
  '# Capability register',
  '',
  'Generated from [CAPABILITY-REGISTER.json](CAPABILITY-REGISTER.json). That JSON is the canonical register.',
  '',
  'Statuses describe stated subsets. No row establishes whole-product acceptance. Component ownership is by module; no staffing is assumed.',
  '',
  '| ID | Domain / capability | Current status | Implementation owner | Gate | Dependencies |',
  '| --- | --- | --- | --- | --- | --- |',
  ...rows.map(
    (row) =>
      `| ${row.id} | ${row.domain}: ${row.capability} | ${row.status} | ${register.routes[row.route].component} | ${row.stage === null ? 'Deferred' : `Stage ${row.stage}`} | ${row.dependencies.join(', ') || 'None'} |`,
  ),
  '',
  '## Traced routes',
  '',
  ...Object.entries(register.routes).flatMap(([id, route]) => [
    `### ${id}: ${route.component}`,
    '',
    route.trace,
    '',
    ...route.sources.map((name) => `- [${name}](../${name})`),
    '',
  ]),
  '## Frozen acceptance tasks',
  '',
  `Each of the ${rows.length} requirements has a task and expected result in the canonical JSON.`,
  `The [fixture specification](STAGE-0-ACCEPTANCE-FIXTURES.json) contains ${fixtures.cases.length} representative authored cases with fixed source text and expected effects.`,
  'Their [acceptance lock](EVIDENCE/stage-0/acceptance-lock.json) detects expectation changes; none of these specifications is automatically labelled an executed result.',
  '',
];
const rendered = `${lines.join('\n')}\n`;
const renderedPath = path.join(root, 'docs/CAPABILITY-REGISTER.md');
if (process.argv.includes('--render')) fs.writeFileSync(renderedPath, rendered);
else if (!fs.existsSync(renderedPath) || fs.readFileSync(renderedPath, 'utf8') !== rendered)
  fail('Human-readable register is stale; regenerate with --render.');

const captureArgument = process.argv.find((arg) => arg.startsWith('--capture='));
if (process.argv.includes('--capture') || captureArgument) {
  const captureLabel = captureArgument ? captureArgument.slice('--capture='.length) : 'baseline';
  if (!/^[a-z0-9-]+$/.test(captureLabel)) fail('Invalid capture label');
  const output = `docs/EVIDENCE/stage-0/${captureLabel}.json`;
  if (fs.existsSync(path.join(root, output)))
    fail('Baseline already exists; use a new reviewed evidence version.');
  const listed = git('ls-files', '-z', '--cached', '--others', '--exclude-standard')
    .split('\0')
    .filter(Boolean);
  const selected = new Set(
    listed.filter((name) => {
      if (
        [
          'docs/EVIDENCE/stage-3/intent-cases.json',
          'docs/EVIDENCE/stage-3/intent-cases-lock.json',
          'docs/EVIDENCE/stage-3/additional-gates.json',
        ].includes(name)
      )
        return true;
      if (/^(apps|crates|services|scripts|proto|packages|tests)\//.test(name)) {
        return (
          !/(^|\/)(node_modules|dist|target|runtime-stage|gen|__pycache__|\.venv)(\/|$)/.test(
            name,
          ) &&
          !/\.(log|pyc|tsbuildinfo|gguf|exe|dll|pdb|lib)$/i.test(name) &&
          !/(^|\/)\.env(?:\.|$)/.test(name)
        );
      }
      return (
        !name.includes('/') &&
        /(^Cargo\.(toml|lock)$|^package(?:-lock)?\.json$|^tsconfig.*\.json$|^eslint\.config\.|^\.gitignore$|^rust-toolchain)/.test(
          name,
        )
      );
    }),
  );
  sourcePaths.forEach((name) => selected.add(name));
  for (const name of [
    'docs/CAPABILITY-REGISTER.json',
    'docs/STAGE-0-ACCEPTANCE-FIXTURES.json',
    'docs/OFFLINE-EXECUTION-PLAN-2026-10-17.md',
  ])
    selected.add(name);
  let totalBytes = 0;
  const snapshot = [...selected].sort().map((name) => {
    const absolute = safeFile(name);
    const size = fs.statSync(absolute).size;
    if (size > 8 * 1024 * 1024) fail(`Oversized source snapshot member: ${name}`);
    totalBytes += size;
    if (totalBytes > 64 * 1024 * 1024) fail('Source snapshot exceeds 64 MiB budget.');
    const data = fs.readFileSync(absolute);
    return { path: name, bytes: data.length, sha256: sha(data), base64: data.toString('base64') };
  });
  const snapshotName = `target/stage-0/${captureLabel}-source-snapshot.json`;
  fs.mkdirSync(path.join(root, 'target/stage-0'), { recursive: true });
  write(snapshotName, {
    version: 1,
    description:
      'Exact selected dirty source/config/assets and lockfiles, not a clean Git checkout or self-contained dependency/runtime bundle.',
    files: snapshot,
  });
  const hashFile = async (name) => {
    const digest = crypto.createHash('sha256');
    for await (const part of fs.createReadStream(name, { highWaterMark: 4 * 1024 * 1024 }))
      digest.update(part);
    return digest.digest('hex');
  };
  const artifactArgument = process.argv.find((arg) => arg.startsWith('--artifacts='));
  const artifactManifestPath = artifactArgument
    ? artifactArgument.slice('--artifacts='.length)
    : 'docs/EVIDENCE/workflows/artifacts.json';
  safeFile(artifactManifestPath);
  const artifacts = read(artifactManifestPath);
  const modulesArgument = process.argv.find((arg) => arg.startsWith('--agent-modules='));
  const modulesPath = modulesArgument
    ? modulesArgument.slice('--agent-modules='.length)
    : 'docs/EVIDENCE/workflows/agent-modules.json';
  safeFile(modulesPath);
  const modules = read(modulesPath);
  if (
    !Array.isArray(modules) ||
    !modules.length ||
    new Set(modules.map((m) => m.file)).size !== modules.length
  )
    fail('Invalid agent module inventory');
  const artifactChecks = [];
  for (const [name, expected] of [
    ['target/release/custodian-desktop.exe', artifacts.nativeSha256],
    ['target/release/bundle/nsis/Custodian_0.1.0_x64-setup.exe', artifacts.installerSha256],
  ]) {
    const actual = await hashFile(safeFile(name));
    if (actual !== expected.toLowerCase()) fail(`Prior accepted artifact mismatch: ${name}`);
    artifactChecks.push({
      path: name,
      bytes: fs.statSync(path.join(root, name)).size,
      sha256: actual,
    });
  }
  for (const module of modules) {
    for (const prefix of ['services/agent/dist', 'apps/desktop/src-tauri/runtime-stage/agent']) {
      const actual = await hashFile(safeFile(`${prefix}/${module.file}`));
      if (actual !== module.sha256.toLowerCase())
        fail(`Accepted agent mismatch: ${prefix}/${module.file}`);
    }
  }
  const runtimeManifest = read('apps/desktop/src-tauri/runtime-stage/runtime-manifest.json');
  const modelName = `apps/desktop/src-tauri/runtime-stage/models/${runtimeManifest.model}`;
  const actualModel = await hashFile(safeFile(modelName));
  if (actualModel !== runtimeManifest.sha256.toLowerCase()) fail('Pinned staged model mismatch');
  write(output, {
    schemaVersion: 1,
    capturedAtUtc: new Date().toISOString(),
    head: git('rev-parse', 'HEAD').trim(),
    gitStatus: git('status', '--short', '--untracked-files=normal'),
    dirtySource: true,
    sourceFiles: snapshot.map(({ base64: _base64, ...entry }) => entry),
    sourceBytes: totalBytes,
    sourceSnapshot: { path: snapshotName, sha256: await hashFile(path.join(root, snapshotName)) },
    acceptanceSha256: acceptanceHash,
    artifactManifestPath,
    artifacts: artifactChecks,
    agentModules: modules,
    agentModulesManifestPath: modulesPath,
    model: { path: modelName, sha256: actualModel },
    runtimeManifest,
    limitations: [
      'Snapshot excludes dependencies, models, generated outputs, training datasets and general docs.',
      'Restore into a new directory only; never overwrite the active checkout.',
      'Rebuild needs pinned dependency/model assets and reference toolchain; bit-for-bit compiler reproducibility is not certified.',
      'Artifact hash identity is not installation acceptance or code signing.',
    ],
  });
}
console.log(
  `PASS: ${rows.length} requirements, ${Object.keys(register.routes).length} source routes, ${fixtures.cases.length} frozen cases; dependencies, source/evidence links, acceptance lock and rendered register validated.`,
);
