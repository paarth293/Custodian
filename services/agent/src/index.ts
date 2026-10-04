import { spawnSync } from 'node:child_process';
import * as fs from 'node:fs';
import * as net from 'node:net';

export { getChatSession, chatWithModel } from './llm';

export interface Probe {
  name: string;
  /** Attempts a privileged operation; throws or rejects if the sandbox blocked it. */
  attempt: () => void | Promise<void>;
}

/** Opens a listening TCP socket on loopback and closes it again. */
function openSocket(): Promise<void> {
  return new Promise((resolve, reject) => {
    const server = net.createServer();
    server.once('error', reject);
    server.listen(0, '127.0.0.1', () => server.close(() => resolve()));
  });
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
    name: 'open network socket',
    attempt: openSocket,
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
export async function findSandboxEscapes(probes: Probe[]): Promise<string[]> {
  const escapes: string[] = [];
  for (const probe of probes) {
    try {
      await probe.attempt();
      escapes.push(probe.name);
    } catch {
      // Blocked, as required.
    }
  }
  return escapes;
}

async function main(args: string[]): Promise<number> {
  if (args[0] === 'caps-probe') {
    const escapes = await findSandboxEscapes(SANDBOX_PROBES);
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
  main(process.argv.slice(2)).then((code) => {
    process.exitCode = code;
  });
}
