import { spawnSync } from 'node:child_process';
import * as fs from 'node:fs';

export interface Probe {
  name: string;
  /** Attempts a privileged operation; throws if the sandbox blocked it. */
  attempt: () => void;
}

/** Operations the agent's AppContainer must deny (T-CAPS-06). */
export const SANDBOX_PROBES: Probe[] = [
  {
    name: 'read user profile',
    attempt: () => {
      const profile = process.env['USERPROFILE'] ?? process.env['HOME'];
      if (!profile) throw new Error('no profile directory');
      fs.readdirSync(profile);
    },
  },
  {
    name: 'spawn child process',
    attempt: () => {
      const result = spawnSync(process.execPath, ['-e', '0']);
      if (result.error) throw result.error;
    },
  },
];

/** Returns the names of probes that were NOT blocked. */
export function findSandboxEscapes(probes: Probe[]): string[] {
  return probes
    .filter((probe) => {
      try {
        probe.attempt();
        return true;
      } catch {
        return false;
      }
    })
    .map((probe) => probe.name);
}

function main(args: string[]): number {
  if (args[0] === 'caps-probe') {
    const escapes = findSandboxEscapes(SANDBOX_PROBES);
    if (escapes.length > 0) {
      console.error(`Sandbox failure, allowed: ${escapes.join(', ')}`);
      return 1;
    }
    console.log('All privileged operations blocked.');
  }
  console.log('sys.ready');
  return 0;
}

if (require.main === module) {
  process.exitCode = main(process.argv.slice(2));
}
