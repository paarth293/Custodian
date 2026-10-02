export async function sendIpcFrame(role: string, payload: any): Promise<boolean> {
  console.log("Sending IPC frame to ${role}:", payload);
  return true;
}
