import { log } from '@temporalio/workflow';

/** Runs an installer through the install lab. Placeholder. */
export async function installWorkflow(installer: string): Promise<void> {
  log.info('InstallWorkflow started', { installer });
}
