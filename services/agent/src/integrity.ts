import { createHash } from 'node:crypto';
import * as fs from 'node:fs';

/**
 * Manifest mapping an artefact name to its expected SHA-256 digest. A chat
 * model marked `default` is used unless another is asked for by name.
 */
export type Manifest = Record<string, { sha256: string; default?: boolean } | undefined>;

export function sha256(data: Buffer | string): string {
  return createHash('sha256').update(data).digest('hex');
}

/** SHA-256 of a file, streamed so multi-gigabyte models are never held in memory. */
export async function sha256File(file: string): Promise<string> {
  const hash = createHash('sha256');
  // 4 MiB reads hash about twice as fast as the 64 KiB default.
  for await (const chunk of fs.createReadStream(file, { highWaterMark: 4 << 20 })) {
    hash.update(chunk as Buffer);
  }
  return hash.digest('hex');
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
