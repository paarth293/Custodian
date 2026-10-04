import { Worker } from '@temporalio/worker';
import * as activities from './activities';

export const TASK_QUEUE = 'custodian-agent';

async function run(): Promise<void> {
  const worker = await Worker.create({
    workflowsPath: require.resolve('./workflows'),
    activities,
    taskQueue: TASK_QUEUE,
  });
  await worker.run();
}

run().catch((err: unknown) => {
  console.error(err);
  process.exit(1);
});
