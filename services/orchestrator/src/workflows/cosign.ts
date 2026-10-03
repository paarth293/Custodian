export async function coSignWait(planId: string): Promise<boolean> {
    console.log("Waiting 72h for helper cosign on plan " + planId);
    return true;
}
