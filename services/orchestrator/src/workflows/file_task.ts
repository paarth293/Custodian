import { log } from '@temporalio/workflow';

/** shadow -> diff -> approve -> real -> verify. Placeholder. */
export async function fileTaskWorkflow(plan: unknown): Promise<void> {
  log.info('FileTaskWorkflow started', { plan });
}
