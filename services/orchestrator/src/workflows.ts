import { proxyActivities } from '@temporalio/workflow';
import type * as activities from './activities';

const { sendIpcFrame } = proxyActivities<typeof activities>({
  startToCloseTimeout: '1 minute',
});

export interface IpcTaskInput {
  role: string;
  payload: unknown;
}

/**
 * Minimal durable workflow boundary. The activity remains fail-closed until
 * Supervisor-owned IPC is connected; this file ensures the Temporal worker
 * has a real, typed workflow module instead of a missing runtime import.
 */
export async function sendTaskFrame(input: IpcTaskInput): Promise<boolean> {
  if (!input || typeof input.role !== 'string' || input.role.length === 0) {
    throw new Error('E_INVALID: workflow role is required');
  }
  return sendIpcFrame(input.role, input.payload);
}
