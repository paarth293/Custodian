/** Sends an IPC frame to another Custodian role. Placeholder: logs only. */
export async function sendIpcFrame(role: string, payload: unknown): Promise<boolean> {
  console.log(`Sending IPC frame to ${role}:`, payload);
  return true;
}
