const MAX_METADATA_CHARS = 256;

/** Human-readable receipt for an action, with metadata truncated. */
export function createReceipt(action: string, metadata: unknown): string {
  const json = JSON.stringify(metadata) ?? '';
  return `Receipt: ${action} - ${json.substring(0, MAX_METADATA_CHARS)}`;
}
