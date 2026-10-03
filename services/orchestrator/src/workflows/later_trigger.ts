import { log } from '@temporalio/workflow';

/** Defers a task until its conditions hold. Placeholder. */
export async function laterTrigger(conditions: unknown): Promise<void> {
  log.info('LaterTrigger waiting for condition', { conditions });
}
