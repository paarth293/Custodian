import * as fs from 'node:fs';
import * as path from 'node:path';
import { matchesManifest, readManifest } from './integrity';

export const DEFAULT_PROMPTS_DIR = path.resolve(__dirname, '..', 'prompts');

/**
 * Loads a prompt only if its content matches the pinned manifest digest.
 *
 * Line endings are normalised to LF before hashing, so the same prompt
 * verifies on a Windows (CRLF) and a Unix (LF) checkout. Pin hashes of the
 * LF-normalised text.
 */
export function loadPrompt(name: string, promptsDir = DEFAULT_PROMPTS_DIR): string {
  const manifest = readManifest(path.join(promptsDir, 'manifest.json'));
  if (manifest[name] === undefined) throw new Error(`sec.event: unknown prompt ${name}`);

  const content = fs
    .readFileSync(path.join(promptsDir, `${name}.md`), 'utf8')
    .replace(/\r\n/g, '\n');
  if (!matchesManifest(manifest, name, content)) {
    throw new Error(`sec.event: prompt ${name} failed verification`);
  }
  return content;
}
