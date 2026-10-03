// import { proxyActivities } from '@temporalio/workflow';
export async function fileTaskWorkflow(_plan: unknown): Promise<void> {
    console.log("FileTaskWorkflow running: shadow -> diff -> approve -> real -> verify");
}
