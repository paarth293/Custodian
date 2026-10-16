import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { InputData, JSONSchemaInput, quicktype } from 'quicktype-core';
import { JSONSchemaStore } from 'quicktype-core/dist/input/JSONSchemaStore.js';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const schemaRoot = path.join(root, 'proto');
const outAt = process.argv.indexOf('--out-root');
const outRoot = outAt < 0 ? root : path.resolve(process.argv[outAt + 1] ?? '');

/** Our checked-in schemas only. No schema generation request may fetch a URL. */
class LocalSchemas extends JSONSchemaStore {
  async fetch(address) {
    const url = new URL(address);
    const file =
      url.protocol === 'file:'
        ? fileURLToPath(url)
        : url.origin === 'https://custodian.internal'
          ? path.join(schemaRoot, decodeURIComponent(url.pathname))
          : null;
    if (!file || path.relative(schemaRoot, file).split(path.sep).includes('..')) {
      throw new Error(`Schema reference is outside the checked-in proto directory: ${address}`);
    }
    return JSON.parse(await readFile(file, 'utf8'));
  }
}

for (const [lang, destination, rendererOptions] of [
  [
    'rust',
    'crates/custodian-proto/src/lib.rs',
    { visibility: 'public', 'derive-debug': true, 'derive-clone': true, 'derive-partial-eq': true },
  ],
  ['ts', 'services/agent/src/proto.ts', { 'just-types': true }],
  ['python', 'services/mlhost/mlhost/proto.py', { 'python-version': '3.7' }],
]) {
  const input = new JSONSchemaInput(new LocalSchemas());
  await input.addSource({
    name: 'Transaction',
    uris: [pathToFileURL(path.join(schemaRoot, 'core/Transaction.json')).href],
  });
  const inputData = new InputData();
  inputData.addInput(input);
  const result = await quicktype({ lang, inputData, rendererOptions });
  const output = path.join(outRoot, destination);
  await mkdir(path.dirname(output), { recursive: true });
  await writeFile(output, result.lines.join('\n') + '\n');
  console.log(`Generated ${destination}`);
}
