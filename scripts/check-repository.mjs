// Dependency-free source-boundary checks: usable before npm ci in a clean clone.
import { execFileSync, spawnSync } from 'node:child_process';
import { existsSync, lstatSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const directories = new Set([
  '.github',
  '.vscode',
  'apps',
  'crates',
  'docs',
  'fixture',
  'models',
  'proto',
  'scripts',
  'services',
  'tests',
  'training',
  'video',
]);
const rootFiles = new Set([
  '.dockerignore',
  '.editorconfig',
  '.gitattributes',
  '.gitignore',
  '.nvmrc',
  '.prettierignore',
  '.prettierrc',
  '.python-version',
  'AGENTS.md',
  'Cargo.lock',
  'Cargo.toml',
  'CONTRIBUTING.md',
  'deny.toml',
  'Dockerfile',
  'eslint.config.mjs',
  'HONESTY.md',
  'LICENSE',
  'Makefile',
  'package-lock.json',
  'package.json',
  'pyproject.toml',
  'README.md',
  'requirements-dev.txt',
  'rust-toolchain.toml',
  'SECURITY.md',
  'tsconfig.base.json',
  'tsconfig.json',
  '.env.example',
]);
const documentationDirectories = new Set([
  'design',
  'plans',
  'reference',
  'verification',
  'EVIDENCE',
]);

function git(args) {
  return execFileSync('git', args, { cwd: root, encoding: 'utf8', maxBuffer: 16 << 20 });
}
function ignored(paths) {
  const result = spawnSync('git', ['check-ignore', '--no-index', '-z', '--stdin'], {
    cwd: root,
    encoding: 'utf8',
    input: paths.join('\0') + '\0',
  });
  if (result.error) throw result.error;
  if (result.status !== 0 && result.status !== 1) throw Error(result.stderr);
  return new Set(result.stdout.split('\0').filter(Boolean));
}

const files = [
  ...new Set(
    git(['ls-files', '--cached', '--others', '--exclude-standard', '-z'])
      .split('\0')
      .filter(Boolean),
  ),
].filter((name) => existsSync(path.join(root, name)));
const errors = [];
const accidentallyTracked = git(['ls-files', '--cached', '--ignored', '--exclude-standard', '-z'])
  .split('\0')
  .filter(Boolean);
for (const name of accidentallyTracked) {
  errors.push(`${name}: generated/local path is still tracked`);
}
for (const name of files) {
  const stat = lstatSync(path.join(root, name));
  if (stat.size > 5 * 1024 * 1024)
    errors.push(`${name}: exceeds the 5 MiB source-file budget; use a release/artifact store`);
  if (name.includes('/') ? !directories.has(name.split('/')[0]) : !rootFiles.has(name)) {
    errors.push(`${name}: place it in its owning source directory or a local artifact directory`);
  }
  if (name.startsWith('docs/')) {
    const parts = name.split('/');
    if (
      (parts.length === 2 && name !== 'docs/README.md') ||
      (parts.length > 2 && !documentationDirectories.has(parts[1]))
    ) {
      errors.push(`${name}: use a documented category; keep the docs root limited to its index`);
    }
    if (/\.html?$/i.test(name))
      errors.push(`${name}: generated documentation exports belong in local artifacts`);
  }
}

const ignoredExamples = [
  'target/debug/app.exe',
  'node_modules/package/index.js',
  'services/agent/dist/index.js',
  'services/mlhost/.venv/pyvenv.cfg',
  'services/mlhost/__pycache__/host.pyc',
  'models/download.gguf',
  'fixture/generated/document.txt',
  'apps/desktop/src-tauri/runtime-stage/node.exe',
  '.freebuff/project-id',
  '.local/design-references/reference.pdf',
  'deliverables/film.mp4',
  'artifacts/evidence/run.txt',
  'video/exports/film.mp4',
  '.env',
  '.env.production',
];
const sourceExamples = [
  'models/.gitkeep',
  'fixture/.gitkeep',
  '.env.example',
  'services/editor-adapter/package.json',
  'apps/desktop/src-tauri/src/lib.rs',
  'docs/reference/STAGE-0-ACCEPTANCE-FIXTURES.json',
  'docs/EVIDENCE/stage-3/intent-cases.json',
  'video/README.md',
  'package-lock.json',
  'Cargo.lock',
];
const ignoredSet = ignored([...ignoredExamples, ...sourceExamples]);
for (const name of ignoredExamples)
  if (!ignoredSet.has(name)) errors.push(`${name}: local artifact is not ignored`);
for (const name of sourceExamples)
  if (ignoredSet.has(name)) errors.push(`${name}: required source is incorrectly ignored`);

// Keep navigation and the canonical evidence aliases usable after documentation moves.
const documentationIndex = readFileSync(path.join(root, 'docs/README.md'), 'utf8');
for (const [, target] of documentationIndex.matchAll(/\[[^\]\n]*\]\(([^)]+)\)/g)) {
  if (/^[a-z][\w+.-]*:/i.test(target) || target.startsWith('#')) continue;
  const destination = path.resolve(root, 'docs', target.split('#')[0]);
  if (!existsSync(destination)) errors.push(`docs/README.md: missing link target ${target}`);
}
const register = JSON.parse(
  readFileSync(path.join(root, 'docs/reference/CAPABILITY-REGISTER.json'), 'utf8'),
);
for (const [alias, name] of Object.entries(register.evidence)) {
  const destination = path.resolve(root, name);
  if (!destination.startsWith(root + path.sep) || !existsSync(destination))
    errors.push(`Capability evidence ${alias}: missing or unsafe target ${name}`);
}

const manifest = JSON.parse(readFileSync(path.join(root, 'package.json'), 'utf8'));
const dockerfile = readFileSync(path.join(root, 'Dockerfile'), 'utf8');
for (const workspace of manifest.workspaces) {
  const name = `${workspace}/package.json`;
  if (!existsSync(path.join(root, name))) errors.push(`${name}: missing workspace manifest`);
  if (!dockerfile.includes(`COPY ${name} ${name}`))
    errors.push(`${name}: missing from Docker dependency-install stage`);
}
if (errors.length) {
  for (const error of errors) console.error(`[repo] ${error}`);
  process.exitCode = 1;
} else {
  console.log(
    `[repo] ${files.length} source files checked; source boundaries, ignore rules and workspace packaging pass.`,
  );
}
