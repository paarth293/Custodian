const INJECTION_MARKERS = ['ignore your instructions', 'ignore previous instructions'];

export interface ReaderResult {
  available: false;
  data: { summary: string };
  label: 'WEB_UNTRUSTED';
  injectionDetected: boolean;
}

/**
 * Extracts data from untrusted web chunks. Output is always labelled
 * untrusted; chunks containing prompt-injection markers are flagged.
 * Extraction is explicitly unavailable until a real reader is wired in.
 */
export function readerExtract(chunks: string[]): ReaderResult {
  const injectionDetected = chunks.some((chunk) =>
    INJECTION_MARKERS.some((marker) => chunk.toLowerCase().includes(marker)),
  );
  if (injectionDetected) console.log('sec.event: injection attempt detected');
  return {
    available: false,
    data: { summary: 'Content extraction is not implemented. No summary was generated.' },
    label: 'WEB_UNTRUSTED',
    injectionDetected,
  };
}
