import { log } from '@temporalio/workflow';

/** Waits up to 72h for a helper's co-signature. Placeholder: approves immediately. */
export async function coSignWait(planId: string): Promise<boolean> {
  log.info('Waiting for helper co-sign', { planId });
  return true;
}
