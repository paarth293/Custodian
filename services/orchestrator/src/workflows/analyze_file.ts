import { proxyActivities } from '@temporalio/workflow';
import type * as activities from '../activities';

const { sendIpcFrame } = proxyActivities<typeof activities>({
  startToCloseTimeout: '1 minute',
});

export async function AnalyzeFile(path: string, hash: string): Promise<void> {
  await sendIpcFrame('MlHost', {
    action: 'extract_embeddings',
    path,
    hash,
  });
}
