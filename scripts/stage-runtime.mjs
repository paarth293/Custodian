import fs from 'node:fs/promises';
import path from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import { createReadStream } from 'node:fs';
import { execFileSync } from 'node:child_process';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
if (process.platform !== 'win32' || process.arch !== 'x64')
  throw new Error('This offline installer staging target requires Windows x64.');
const modules = path.join(root, 'node_modules');
const stage = path.join(root, 'apps/desktop/src-tauri/runtime-stage');
const require = createRequire(import.meta.url);
const npm =
  process.env.npm_execpath ??
  path.join(path.dirname(process.execPath), 'node_modules/npm/bin/npm-cli.js');
execFileSync(process.execPath, [npm, 'run', 'build', '-w', 'services/agent'], {
  cwd: root,
  stdio: 'inherit',
});

const inside = (parent, child) => {
  const relative = path.relative(parent, child);
  return !path.isAbsolute(relative) && relative !== '..' && !relative.startsWith(`..${path.sep}`);
};
// This is generated output only. Validate its exact resolved parent before replacing it.
const realRoot = await fs.realpath(root);
if (
  !inside(realRoot, await fs.realpath(path.dirname(stage))) ||
  path.basename(stage) !== 'runtime-stage'
)
  throw new Error('Unsafe runtime staging path');
try {
  if ((await fs.lstat(stage)).isSymbolicLink())
    throw new Error('Runtime staging must not be a link');
} catch (error) {
  if (error.code !== 'ENOENT') throw error;
}
await fs.rm(stage, { recursive: true, force: true });
await fs.mkdir(stage, { recursive: true });
await fs.writeFile(path.join(stage, '.gitkeep'), '');

const queue = [path.join(root, 'services/agent')];
const visited = new Set();
let packages = 0;
while (queue.length > 0) {
  const directory = queue.shift();
  if (visited.has(directory)) continue;
  visited.add(directory);
  const manifest = JSON.parse(await fs.readFile(path.join(directory, 'package.json'), 'utf8'));
  if (
    manifest.cpu?.includes(`!${process.arch}`) ||
    (manifest.cpu?.some((cpu) => !cpu.startsWith('!')) && !manifest.cpu.includes(process.arch))
  )
    continue;
  if (
    manifest.os?.includes(`!${process.platform}`) ||
    (manifest.os?.some((os) => !os.startsWith('!')) && !manifest.os.includes(process.platform))
  )
    continue;
  // The offline Windows runtime includes CPU and Vulkan. CUDA is an optional external backend.
  if (manifest.name.startsWith('@node-llama-cpp/') && manifest.name.includes('cuda')) continue;
  const from = createRequire(path.join(directory, 'package.json'));
  const required = new Set(Object.keys(manifest.dependencies ?? {}));
  const dependencies = {
    ...manifest.dependencies,
    ...manifest.optionalDependencies,
    ...manifest.peerDependencies,
  };
  for (const name of Object.keys(dependencies)) {
    const candidates = from.resolve.paths(name) ?? [];
    let found = false;
    for (const base of candidates) {
      const dependency = path.join(base, name);
      if (!inside(modules, dependency)) continue;
      try {
        await fs.access(path.join(dependency, 'package.json'));
        queue.push(dependency);
        found = true;
        break;
      } catch (error) {
        if (error.code !== 'ENOENT') throw error;
      }
    }
    if (!found && required.has(name)) throw new Error(`Missing runtime dependency ${name}`);
  }
  if (directory !== path.join(root, 'services/agent')) {
    const destination = path.join(stage, 'node_modules', path.relative(modules, directory));
    await fs.cp(directory, destination, {
      recursive: true,
      dereference: false,
      filter: (source) =>
        !path.relative(directory, source).split(path.sep).includes('node_modules'),
    });
    packages++;
  }
}
await fs.cp(path.join(root, 'services/agent/dist'), path.join(stage, 'agent'), { recursive: true });
await fs.copyFile(
  path.join(root, 'services/agent/package.json'),
  path.join(stage, 'agent/package.json'),
);
await fs.mkdir(path.join(stage, 'bin'), { recursive: true });
await fs.copyFile(process.execPath, path.join(stage, 'bin/node.exe'));
try {
  await fs.copyFile(
    path.join(path.dirname(process.execPath), 'LICENSE'),
    path.join(stage, 'bin/NODE-LICENSE'),
  );
} catch (error) {
  if (error.code !== 'ENOENT') throw error;
}

const { findSmallChatModel } = require('../services/agent/dist/llm.js');
const model = findSmallChatModel(path.join(root, 'models'));
if (!model)
  throw new Error(
    'A pinned CPU chat model is required for an offline installer. Fetch one into models/ first.',
  );
const hash = createHash('sha256');
for await (const chunk of createReadStream(model.path, { highWaterMark: 4 << 20 }))
  hash.update(chunk);
if (hash.digest('hex') !== model.sha256.toLowerCase())
  throw new Error('Bundled model failed its SHA-256 pin');
await fs.mkdir(path.join(stage, 'models'), { recursive: true });
await fs.copyFile(model.path, path.join(stage, 'models', path.basename(model.path)));
await fs.writeFile(
  path.join(stage, 'models/manifest.json'),
  JSON.stringify({ [path.basename(model.path)]: { sha256: model.sha256, default: true } }, null, 2),
);
await fs.writeFile(
  path.join(stage, 'runtime-manifest.json'),
  JSON.stringify(
    {
      node: process.version,
      platform: process.platform,
      architecture: process.arch,
      packages,
      model: path.basename(model.path),
      sha256: model.sha256,
    },
    null,
    2,
  ),
);
console.log(
  `Staged Node ${process.version}, ${packages} runtime packages, and ${path.basename(model.path)}.`,
);
