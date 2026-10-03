import { proxyActivities } from '@temporalio/workflow';
export async function fileTaskWorkflow(plan: any): Promise<void> {
    console.log("FileTaskWorkflow running: shadow -> diff -> approve -> real -> verify");
}
