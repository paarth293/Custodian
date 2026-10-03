export function createReceipt(action: string, metadata: unknown) {
    return "Receipt: " + action + " - " + JSON.stringify(metadata).substring(0, 256);
}
