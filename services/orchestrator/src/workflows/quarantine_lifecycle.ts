import { log } from '@temporalio/workflow';

/** Purges a quarantined item after its 7-day retention. Placeholder. */
export async function quarantineLifecycle(id: string): Promise<void> {
  log.info('QuarantineLifecycle started', { id });
}
