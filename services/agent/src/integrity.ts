import { createHash } from 'node:crypto';
import * as fs from 'node:fs';

/** Manifest mapping an artefact name to its expected SHA-256 digest. */
export type Manifest = Record<string, { sha256: string } | undefined>;

export function sha256(data: Buffer | string): string {
  return createHash('sha256').update(data).digest('hex');
}

export function readManifest(manifestPath: string): Manifest {
  return JSON.parse(fs.readFileSync(manifestPath, 'utf8')) as Manifest;
}

/** Returns true when `content` matches the digest pinned for `name`. */
export function matchesManifest(
  manifest: Manifest,
  name: string,
  content: Buffer | string,
): boolean {
  const expected = manifest[name]?.sha256;
  return expected !== undefined && sha256(content) === expected.toLowerCase();
}
