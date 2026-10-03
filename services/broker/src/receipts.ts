export function createReceipt(action: string, metadata: any) {
    return Receipt:  + action +  -  + JSON.stringify(metadata).substring(0, 256);
}
