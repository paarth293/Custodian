import * as fs from 'node:fs';
import * as path from 'node:path';
import { matchesManifest, readManifest } from './integrity';

export const DEFAULT_PROMPTS_DIR = path.resolve(__dirname, '..', 'prompts');

/** Loads a prompt only if its content matches the pinned manifest digest. */
export function loadPrompt(name: string, promptsDir = DEFAULT_PROMPTS_DIR): string {
  const manifest = readManifest(path.join(promptsDir, 'manifest.json'));
  if (manifest[name] === undefined) throw new Error(`sec.event: unknown prompt ${name}`);

  const content = fs.readFileSync(path.join(promptsDir, `${name}.md`), 'utf8');
  if (!matchesManifest(manifest, name, content)) {
    throw new Error(`sec.event: prompt ${name} failed verification`);
  }
  return content;
}
