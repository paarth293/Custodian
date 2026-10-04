function escapeXml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

/**
 * Wraps untrusted text in a `<data>` element so the model cannot confuse it
 * with instructions. Text is escaped and truncated to `cap` characters.
 */
export function wrapData(text: string, source: string, label: string, cap: number): string {
  const clipped = text.length > cap ? `${text.substring(0, cap)}...` : text;
  return `<data source="${escapeXml(source)}" label="${escapeXml(label)}">${escapeXml(clipped)}</data>`;
}
