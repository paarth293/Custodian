import * as fs from 'node:fs';
import * as path from 'node:path';

export const FIXTURE_MARKER = '.custodian-fixture-root';

/** A directory proven to be a generated fixture tree. */
export class FixtureRoot {
  constructor(readonly root: string) {
    if (!fs.existsSync(path.join(root, FIXTURE_MARKER))) {
      throw new Error(`SECURITY: test attempted to use a non-fixture path: ${root}`);
    }
  }
}
